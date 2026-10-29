use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// Preserve the existing email-only key and on-disk directory names.
pub fn hash_email(email: &str) -> String {
    Sha256::digest(email.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Filesystem provisioning is separate from relational backend initialization.
pub async fn provision_user_data(data_dir: &str, user_hash: &str) -> Result<PathBuf, String> {
    let user_dir = PathBuf::from(data_dir).join(user_hash);
    tokio::fs::create_dir_all(user_dir.join("tantivy"))
        .await
        .map_err(|e| format!("Failed to create user storage: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&user_dir, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(|e| format!("Failed to secure user storage: {e}"))?;
    }
    Ok(user_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn email_hash_is_stable_and_distinct() {
        let a = hash_email("alice@example.com");
        assert_eq!(a, hash_email("alice@example.com"));
        assert_ne!(a, hash_email("bob@example.com"));
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[tokio::test]
    async fn filesystem_provisioning_is_idempotent_and_does_not_create_sqlite() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().to_str().unwrap();
        let first = provision_user_data(path, "alice").await.unwrap();
        assert_eq!(first, provision_user_data(path, "alice").await.unwrap());
        assert!(first.join("tantivy").is_dir());
        assert!(!first.join("db.sqlite").exists());
    }
}
