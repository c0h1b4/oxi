use super::*;
use std::sync::Mutex;

/// A mock IMAP client that returns pre-loaded data.
///
/// Uses interior mutability (`Mutex`) so it can be shared behind `&self`.
/// Build it up with the `.with_*()` builder methods, then pass it wherever
/// an `&dyn ImapClient` is needed.
pub struct MockImapClient {
    folders: Mutex<Vec<ImapFolder>>,
    headers: Mutex<Vec<ImapMessageHeader>>,
    bodies: Mutex<Vec<ImapMessageBody>>,
    folder_status: Mutex<Option<FolderStatus>>,
    folder_status_ext: Mutex<Option<FolderStatusExtended>>,
    should_fail: Mutex<Option<ImapError>>,
    pub appended: Mutex<Vec<(String, Vec<u8>)>>,
}

impl MockImapClient {
    /// Create a new empty mock.
    pub fn new() -> Self {
        Self {
            folders: Mutex::new(Vec::new()),
            headers: Mutex::new(Vec::new()),
            bodies: Mutex::new(Vec::new()),
            folder_status: Mutex::new(None),
            folder_status_ext: Mutex::new(None),
            should_fail: Mutex::new(None),
            appended: Mutex::new(Vec::new()),
        }
    }

    /// Pre-load folders that `list_folders` will return.
    pub fn with_folders(self, folders: Vec<ImapFolder>) -> Self {
        *self.folders.lock().unwrap() = folders;
        self
    }

    /// Pre-load message headers that `fetch_headers` will return.
    pub fn with_headers(self, headers: Vec<ImapMessageHeader>) -> Self {
        *self.headers.lock().unwrap() = headers;
        self
    }

    /// Pre-load a folder status that `folder_status` will return.
    #[allow(dead_code)]
    pub fn with_folder_status(self, status: FolderStatus) -> Self {
        *self.folder_status.lock().unwrap() = Some(status);
        self
    }

    /// Pre-load an extended folder status that `folder_status_extended` will return.
    #[allow(dead_code)]
    pub fn with_folder_status_extended(self, status: FolderStatusExtended) -> Self {
        *self.folder_status_ext.lock().unwrap() = Some(status);
        self
    }

    /// Pre-load message bodies that `fetch_body` will match against by UID.
    pub fn with_bodies(self, bodies: Vec<ImapMessageBody>) -> Self {
        *self.bodies.lock().unwrap() = bodies;
        self
    }

    /// Make every subsequent call return this error.
    pub fn with_error(self, error: ImapError) -> Self {
        *self.should_fail.lock().unwrap() = Some(error);
        self
    }
}

/// Helper to clone an `ImapError` for the mock (the real errors are not
/// `Clone`, so we reconstruct them).
fn clone_error(err: &ImapError) -> ImapError {
    match err {
        ImapError::ConnectionFailed(msg) => ImapError::ConnectionFailed(msg.clone()),
        ImapError::AuthenticationFailed => ImapError::AuthenticationFailed,
        ImapError::FolderNotFound(name) => ImapError::FolderNotFound(name.clone()),
        ImapError::MessageNotFound { uid, folder } => ImapError::MessageNotFound {
            uid: *uid,
            folder: folder.clone(),
        },
        ImapError::ProtocolError(msg) => ImapError::ProtocolError(msg.clone()),
    }
}

#[async_trait]
impl ImapClient for MockImapClient {
    async fn list_folders(
        &self,
        _creds: &ImapCredentials,
    ) -> Result<Vec<ImapFolder>, ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(self.folders.lock().unwrap().clone())
    }

    async fn folder_status(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
    ) -> Result<FolderStatus, ImapError> {
        {
            let fail = self.should_fail.lock().unwrap();
            if let Some(ref err) = *fail {
                return Err(clone_error(err));
            }
        }
        {
            let status = self.folder_status.lock().unwrap();
            if let Some(ref s) = *status {
                return Ok(s.clone());
            }
        }
        // Derive from headers (separate lock scope).
        let headers = self.headers.lock().unwrap();
        let exists = headers.len() as u32;
        let uid_next = headers.iter().map(|h| h.uid).max().unwrap_or(0) + 1;
        Ok(FolderStatus {
            uid_validity: 1,
            exists,
            uid_next,
        })
    }

    async fn fetch_headers(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid_range: &str,
    ) -> Result<Vec<ImapMessageHeader>, ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(self.headers.lock().unwrap().clone())
    }

    async fn fetch_body(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        uid: u32,
    ) -> Result<ImapMessageBody, ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        let bodies = self.bodies.lock().unwrap();
        bodies
            .iter()
            .find(|b| b.uid == uid)
            .cloned()
            .ok_or_else(|| ImapError::MessageNotFound {
                uid,
                folder: _folder.to_string(),
            })
    }

    async fn add_flags(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
        _flags: &[&str],
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn remove_flags(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
        _flags: &[&str],
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn set_flags(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
        _flags: &[&str],
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn move_message(
        &self,
        _creds: &ImapCredentials,
        _from_folder: &str,
        _uid: u32,
        _to_folder: &str,
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn expunge_message(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn append_message(
        &self,
        _creds: &ImapCredentials,
        folder: &str,
        message_bytes: &[u8],
        _flags: &[&str],
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        self.appended
            .lock()
            .unwrap()
            .push((folder.to_string(), message_bytes.to_vec()));
        Ok(())
    }

    async fn create_folder(
        &self,
        _creds: &ImapCredentials,
        _folder_name: &str,
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn rename_folder(
        &self,
        _creds: &ImapCredentials,
        _from: &str,
        _to: &str,
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn delete_folder(
        &self,
        _creds: &ImapCredentials,
        _folder_name: &str,
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn subscribe_folder(
        &self,
        _creds: &ImapCredentials,
        _folder_name: &str,
        _subscribe: bool,
    ) -> Result<(), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        Ok(())
    }

    async fn fetch_uids_and_flags(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
    ) -> Result<Vec<(u32, Vec<String>)>, ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        let headers = self.headers.lock().unwrap();
        Ok(headers.iter().map(|h| (h.uid, h.flags.clone())).collect())
    }

    async fn folder_status_extended(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
    ) -> Result<FolderStatusExtended, ImapError> {
        {
            let fail = self.should_fail.lock().unwrap();
            if let Some(ref err) = *fail {
                return Err(clone_error(err));
            }
        }
        {
            let status = self.folder_status_ext.lock().unwrap();
            if let Some(ref s) = *status {
                return Ok(s.clone());
            }
        }
        // Derive from headers.
        let headers = self.headers.lock().unwrap();
        let exists = headers.len() as u32;
        let uid_next = headers.iter().map(|h| h.uid).max().unwrap_or(0) + 1;
        Ok(FolderStatusExtended {
            uid_validity: 1,
            exists,
            uid_next,
            unseen: 0,
            highest_modseq: 0,
        })
    }

    async fn fetch_changed_flags(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _since_modseq: u64,
    ) -> Result<(Vec<(u32, Vec<String>)>, u64), ImapError> {
        if let Some(ref err) = *self.should_fail.lock().unwrap() {
            return Err(clone_error(err));
        }
        // In mock, return all headers as "changed" with modseq 0.
        let headers = self.headers.lock().unwrap();
        let items: Vec<(u32, Vec<String>)> = headers.iter().map(|h| (h.uid, h.flags.clone())).collect();
        Ok((items, 0))
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Convenience helper to build dummy credentials for tests.
    fn test_creds() -> ImapCredentials {
        ImapCredentials {
            host: "imap.example.com".to_string(),
            port: 993,
            tls: true,
            email: "user@example.com".to_string(),
            password: "hunter2".to_string(),
        }
    }

    #[tokio::test]
    async fn mock_list_folders_returns_preloaded_data() {
        let mock = MockImapClient::new().with_folders(vec![
            ImapFolder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                attributes: vec!["\\HasNoChildren".to_string()],
            },
            ImapFolder {
                name: "Sent".to_string(),
                delimiter: Some("/".to_string()),
                attributes: vec![],
            },
        ]);

        let folders = mock.list_folders(&test_creds()).await.unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, "INBOX");
        assert_eq!(folders[1].name, "Sent");
    }

    #[tokio::test]
    async fn mock_fetch_headers_returns_preloaded_data() {
        let mock = MockImapClient::new().with_headers(vec![ImapMessageHeader {
            uid: 42,
            subject: Some("Hello".to_string()),
            from: vec![EmailAddress {
                name: Some("Alice".to_string()),
                address: "alice@example.com".to_string(),
            }],
            to: vec![EmailAddress {
                name: None,
                address: "bob@example.com".to_string(),
            }],
            date: Some("Mon, 1 Jan 2024 00:00:00 +0000".to_string()),
            flags: vec!["\\Seen".to_string()],
            has_attachments: false,
            size: 1024,
            message_id: None,
            in_reply_to: None,
            references: None,
            cc: vec![],
        }]);

        let headers = mock
            .fetch_headers(&test_creds(), "INBOX", "1:*")
            .await
            .unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].uid, 42);
        assert_eq!(headers[0].subject.as_deref(), Some("Hello"));
    }

    #[tokio::test]
    async fn mock_fetch_body_returns_matching_uid() {
        let mock = MockImapClient::new().with_bodies(vec![
            ImapMessageBody {
                uid: 1,
                text_plain: Some("First message".to_string()),
                text_html: None,
                attachments: vec![],
                raw_headers: String::new(),
            },
            ImapMessageBody {
                uid: 2,
                text_plain: None,
                text_html: Some("<p>Second</p>".to_string()),
                attachments: vec![ImapAttachment {
                    filename: Some("doc.pdf".to_string()),
                    content_type: "application/pdf".to_string(),
                    size: 1024,
                    data: vec![0u8; 1024],
                    content_id: None,
                }],
                raw_headers: String::new(),
            },
        ]);

        let body = mock.fetch_body(&test_creds(), "INBOX", 2).await.unwrap();
        assert_eq!(body.uid, 2);
        assert!(body.text_html.is_some());
        assert_eq!(body.attachments.len(), 1);
        assert_eq!(body.attachments[0].filename.as_deref(), Some("doc.pdf"));
    }

    #[tokio::test]
    async fn mock_fetch_body_returns_not_found_for_missing_uid() {
        let mock = MockImapClient::new().with_bodies(vec![ImapMessageBody {
            uid: 1,
            text_plain: Some("only message".to_string()),
            text_html: None,
            attachments: vec![],
            raw_headers: String::new(),
        }]);

        let err = mock
            .fetch_body(&test_creds(), "INBOX", 999)
            .await
            .unwrap_err();
        match err {
            ImapError::MessageNotFound { uid, folder } => {
                assert_eq!(uid, 999);
                assert_eq!(folder, "INBOX");
            }
            other => panic!("Expected MessageNotFound, got: {other}"),
        }
    }

    #[tokio::test]
    async fn mock_with_error_overrides_all_methods() {
        let mock = MockImapClient::new()
            .with_folders(vec![ImapFolder {
                name: "INBOX".to_string(),
                delimiter: None,
                attributes: vec![],
            }])
            .with_error(ImapError::AuthenticationFailed);

        let err = mock.list_folders(&test_creds()).await.unwrap_err();
        assert!(matches!(err, ImapError::AuthenticationFailed));

        let err = mock
            .fetch_headers(&test_creds(), "INBOX", "1:*")
            .await
            .unwrap_err();
        assert!(matches!(err, ImapError::AuthenticationFailed));

        let err = mock
            .set_flags(&test_creds(), "INBOX", 1, &["\\Seen"])
            .await
            .unwrap_err();
        assert!(matches!(err, ImapError::AuthenticationFailed));

        let err = mock
            .move_message(&test_creds(), "INBOX", 1, "Trash")
            .await
            .unwrap_err();
        assert!(matches!(err, ImapError::AuthenticationFailed));

        let err = mock
            .expunge_message(&test_creds(), "INBOX", 1)
            .await
            .unwrap_err();
        assert!(matches!(err, ImapError::AuthenticationFailed));
    }

    #[tokio::test]
    async fn mock_set_flags_succeeds_without_error() {
        let mock = MockImapClient::new();
        let result = mock
            .set_flags(&test_creds(), "INBOX", 1, &["\\Seen", "\\Flagged"])
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_move_message_succeeds_without_error() {
        let mock = MockImapClient::new();
        let result = mock
            .move_message(&test_creds(), "INBOX", 1, "Archive")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_expunge_message_succeeds_without_error() {
        let mock = MockImapClient::new();
        let result = mock.expunge_message(&test_creds(), "Trash", 5).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn real_imap_client_connection_fails_with_bad_host() {
        let client = RealImapClient;
        let creds = test_creds();

        let err = client.list_folders(&creds).await.unwrap_err();
        // With a fake host the connection should fail.
        assert!(
            matches!(err, ImapError::ConnectionFailed(_)),
            "Expected ConnectionFailed, got: {err}"
        );
    }

    #[tokio::test]
    async fn imap_error_display_formats_correctly() {
        let cases: Vec<(ImapError, &str)> = vec![
            (
                ImapError::ConnectionFailed("timeout".to_string()),
                "Connection failed: timeout",
            ),
            (ImapError::AuthenticationFailed, "Authentication failed"),
            (
                ImapError::FolderNotFound("Drafts".to_string()),
                "Folder not found: Drafts",
            ),
            (
                ImapError::MessageNotFound {
                    uid: 7,
                    folder: "INBOX".to_string(),
                },
                "Message UID 7 not found in folder INBOX",
            ),
            (
                ImapError::ProtocolError("unexpected EOF".to_string()),
                "Protocol error: unexpected EOF",
            ),
        ];

        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[tokio::test]
    async fn email_address_serializes_and_deserializes() {
        let addr = EmailAddress {
            name: Some("Test User".to_string()),
            address: "test@example.com".to_string(),
        };

        let json = serde_json::to_string(&addr).unwrap();
        let deserialized: EmailAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(addr, deserialized);
    }

    #[tokio::test]
    async fn mock_append_message_succeeds() {
        let mock = MockImapClient::new();
        let result = mock
            .append_message(&test_creds(), "Sent", b"From: test\r\n\r\nBody", &["\\Seen"])
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_append_captures_data() {
        let mock = MockImapClient::new();
        mock.append_message(&test_creds(), "Sent", b"test message", &["\\Seen"])
            .await
            .unwrap();
        let appended = mock.appended.lock().unwrap();
        assert_eq!(appended.len(), 1);
        assert_eq!(appended[0].0, "Sent");
        assert_eq!(appended[0].1, b"test message");
    }

    #[tokio::test]
    async fn mock_empty_returns_empty_collections() {
        let mock = MockImapClient::new();
        let creds = test_creds();

        let folders = mock.list_folders(&creds).await.unwrap();
        assert!(folders.is_empty());

        let headers = mock.fetch_headers(&creds, "INBOX", "1:*").await.unwrap();
        assert!(headers.is_empty());
    }

    #[tokio::test]
    async fn mock_create_folder_succeeds() {
        let mock = MockImapClient::new();
        let result = mock.create_folder(&test_creds(), "NewFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_rename_folder_succeeds() {
        let mock = MockImapClient::new();
        let result = mock
            .rename_folder(&test_creds(), "OldName", "NewName")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_delete_folder_succeeds() {
        let mock = MockImapClient::new();
        let result = mock.delete_folder(&test_creds(), "OldFolder").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_subscribe_folder_succeeds() {
        let mock = MockImapClient::new();
        let result = mock
            .subscribe_folder(&test_creds(), "SomeFolder", true)
            .await;
        assert!(result.is_ok());

        let result = mock
            .subscribe_folder(&test_creds(), "SomeFolder", false)
            .await;
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------
    // Integration tests (run manually against a real IMAP server)
    // -------------------------------------------------------------------
    //
    //   cargo test --manifest-path backend/Cargo.toml real_imap -- --ignored
    //
    // Required env vars:
    //   TEST_IMAP_HOST     (e.g. "imap.gmail.com")
    //   TEST_IMAP_PORT     (e.g. "993")
    //   TEST_IMAP_EMAIL    (e.g. "you@gmail.com")
    //   TEST_IMAP_PASSWORD (e.g. "app-password")
    //   TEST_IMAP_TLS      (e.g. "true")

    fn real_creds() -> Option<ImapCredentials> {
        let host = std::env::var("TEST_IMAP_HOST").ok()?;
        let port: u16 = std::env::var("TEST_IMAP_PORT")
            .ok()?
            .parse()
            .ok()?;
        let email = std::env::var("TEST_IMAP_EMAIL").ok()?;
        let password = std::env::var("TEST_IMAP_PASSWORD").ok()?;
        let tls = std::env::var("TEST_IMAP_TLS")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true);
        Some(ImapCredentials {
            host,
            port,
            tls,
            email,
            password,
        })
    }

    #[tokio::test]
    #[ignore] // Run manually: cargo test real_imap_list_folders -- --ignored
    async fn real_imap_list_folders() {
        let creds = real_creds().expect("TEST_IMAP_* env vars required");
        let client = RealImapClient;
        let folders = client.list_folders(&creds).await.unwrap();
        assert!(!folders.is_empty(), "expected at least one folder");
        let names: Vec<_> = folders.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.iter().any(|n| n.eq_ignore_ascii_case("INBOX")),
            "expected INBOX in folder list, got: {names:?}"
        );
    }

    #[tokio::test]
    #[ignore] // Run manually: cargo test real_imap_fetch_headers -- --ignored
    async fn real_imap_fetch_headers() {
        let creds = real_creds().expect("TEST_IMAP_* env vars required");
        let client = RealImapClient;
        let headers = client
            .fetch_headers(&creds, "INBOX", "1:5")
            .await
            .unwrap();
        // The mailbox might be empty, so we just check it doesn't error.
        for h in &headers {
            assert!(h.uid > 0);
        }
    }

    #[tokio::test]
    #[ignore] // Run manually: cargo test real_imap_fetch_body -- --ignored
    async fn real_imap_fetch_body() {
        let creds = real_creds().expect("TEST_IMAP_* env vars required");
        let client = RealImapClient;

        // First fetch headers to find a UID.
        let headers = client
            .fetch_headers(&creds, "INBOX", "1:1")
            .await
            .unwrap();
        if let Some(h) = headers.first() {
            let body = client.fetch_body(&creds, "INBOX", h.uid).await.unwrap();
            assert_eq!(body.uid, h.uid);
        }
    }
}
