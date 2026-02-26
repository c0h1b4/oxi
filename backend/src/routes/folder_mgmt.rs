use std::sync::Arc;

use axum::extract::Path;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct RenameFolderRequest {
    pub new_name: String,
}

#[derive(Deserialize)]
pub struct SubscribeRequest {
    pub subscribed: bool,
}

#[derive(Serialize)]
struct MessageResponse {
    status: &'static str,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Folder names that must not be renamed or deleted.
const SYSTEM_FOLDERS: &[&str] = &["INBOX", "Sent", "Drafts", "Trash", "Junk", "Spam"];

/// Returns `true` if the folder is a system folder (case-insensitive).
fn is_system_folder(name: &str) -> bool {
    SYSTEM_FOLDERS
        .iter()
        .any(|s| s.eq_ignore_ascii_case(name))
}

/// Validates a folder name. Returns `Ok(())` if valid, or an `AppError::BadRequest`.
fn validate_folder_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::BadRequest("Folder name cannot be empty".to_string()));
    }
    if name.len() > 255 {
        return Err(AppError::BadRequest(
            "Folder name must be 255 characters or fewer".to_string(),
        ));
    }
    if name.contains('{') || name.contains('}') || name.contains('*') || name.contains('%') {
        return Err(AppError::BadRequest(
            "Folder name contains invalid characters".to_string(),
        ));
    }
    Ok(())
}

/// Build IMAP credentials from the session and app configuration.
fn build_creds(
    session: &SessionState,
    config: &AppConfig,
) -> Result<ImapCredentials, AppError> {
    let imap_host = config
        .imap_host
        .as_deref()
        .ok_or_else(|| AppError::ServiceUnavailable("Mail server not configured".to_string()))?;

    Ok(ImapCredentials {
        host: imap_host.to_string(),
        port: config.imap_port,
        tls: config.tls_enabled,
        email: session.email.clone(),
        password: session.password.clone(),
    })
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/folders` -- create a new IMAP folder.
pub async fn create_folder(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Json(body): Json<CreateFolderRequest>,
) -> Result<Response, AppError> {
    validate_folder_name(&body.name)?;

    let creds = build_creds(&session, &config)?;

    // Create the folder on the IMAP server (also subscribes).
    imap_client
        .create_folder(&creds, &body.name)
        .await
        .map_err(|e| AppError::InternalError(format!("IMAP create_folder failed: {e}")))?;

    // Cache the new folder locally.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    db::folders::insert_folder_if_new(&conn, &body.name, None, "")
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(MessageResponse { status: "created" }).into_response())
}

/// `PATCH /api/folders/:name` -- rename an existing folder.
pub async fn rename_folder(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path(name): Path<String>,
    Json(body): Json<RenameFolderRequest>,
) -> Result<Response, AppError> {
    if is_system_folder(&name) {
        return Err(AppError::BadRequest(format!(
            "Cannot rename system folder '{name}'"
        )));
    }

    validate_folder_name(&body.new_name)?;

    let creds = build_creds(&session, &config)?;

    imap_client
        .rename_folder(&creds, &name, &body.new_name)
        .await
        .map_err(|e| AppError::InternalError(format!("IMAP rename_folder failed: {e}")))?;

    // Update the local cache.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    db::folders::rename_folder_in_cache(&conn, &name, &body.new_name)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(MessageResponse { status: "renamed" }).into_response())
}

/// `DELETE /api/folders/:name` -- delete a folder.
pub async fn delete_folder(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path(name): Path<String>,
) -> Result<Response, AppError> {
    if is_system_folder(&name) {
        return Err(AppError::BadRequest(format!(
            "Cannot delete system folder '{name}'"
        )));
    }

    let creds = build_creds(&session, &config)?;

    imap_client
        .delete_folder(&creds, &name)
        .await
        .map_err(|e| AppError::InternalError(format!("IMAP delete_folder failed: {e}")))?;

    // Remove from local cache.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    db::folders::delete_folder_and_messages(&conn, &name)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(MessageResponse { status: "deleted" }).into_response())
}

/// `PATCH /api/folders/:name/subscribe` -- toggle folder subscription.
pub async fn subscribe_folder(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path(name): Path<String>,
    Json(body): Json<SubscribeRequest>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    imap_client
        .subscribe_folder(&creds, &name, body.subscribed)
        .await
        .map_err(|e| AppError::InternalError(format!("IMAP subscribe_folder failed: {e}")))?;

    Ok(Json(MessageResponse {
        status: if body.subscribed {
            "subscribed"
        } else {
            "unsubscribed"
        },
    })
    .into_response())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_system_folder() {
        assert!(is_system_folder("INBOX"));
        assert!(is_system_folder("inbox"));
        assert!(is_system_folder("Sent"));
        assert!(is_system_folder("Drafts"));
        assert!(is_system_folder("Trash"));
        assert!(is_system_folder("Junk"));
        assert!(is_system_folder("Spam"));
        assert!(!is_system_folder("Archive"));
        assert!(!is_system_folder("MyFolder"));
    }

    #[test]
    fn test_validate_folder_name_ok() {
        assert!(validate_folder_name("MyFolder").is_ok());
        assert!(validate_folder_name("Projects/2024").is_ok());
    }

    #[test]
    fn test_validate_folder_name_empty() {
        assert!(validate_folder_name("").is_err());
        assert!(validate_folder_name("   ").is_err());
    }

    #[test]
    fn test_validate_folder_name_too_long() {
        let long = "a".repeat(256);
        assert!(validate_folder_name(&long).is_err());
    }

    #[test]
    fn test_validate_folder_name_invalid_chars() {
        assert!(validate_folder_name("test{folder").is_err());
        assert!(validate_folder_name("test}folder").is_err());
        assert!(validate_folder_name("test*folder").is_err());
        assert!(validate_folder_name("test%folder").is_err());
    }
}
