use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Represents an IMAP folder (mailbox).
#[derive(Debug, Clone, Serialize)]
pub struct ImapFolder {
    /// Folder name as returned by the IMAP server (e.g. "INBOX", "Sent").
    pub name: String,
    /// Delimiter used by the server (e.g. "/" or ".").
    pub delimiter: Option<String>,
    /// IMAP attributes for this folder (e.g. `\Noselect`, `\HasChildren`).
    pub attributes: Vec<String>,
}

/// A parsed email address with optional display name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmailAddress {
    /// Display name, if present (e.g. "Alice Smith").
    pub name: Option<String>,
    /// The actual email address (e.g. "alice@example.com").
    pub address: String,
}

/// A lightweight summary of an email message (envelope data).
#[derive(Debug, Clone, Serialize)]
pub struct ImapMessageHeader {
    /// IMAP UID of the message within its folder.
    pub uid: u32,
    /// Subject line.
    pub subject: Option<String>,
    /// Sender(s) of the message.
    pub from: Vec<EmailAddress>,
    /// Recipient(s) of the message.
    pub to: Vec<EmailAddress>,
    /// Date header value (raw string from the server).
    pub date: Option<String>,
    /// IMAP flags currently set on this message (e.g. `\Seen`, `\Flagged`).
    pub flags: Vec<String>,
}

/// The full body of an email message, including attachments.
#[derive(Debug, Clone, Serialize)]
pub struct ImapMessageBody {
    /// IMAP UID of the message within its folder.
    pub uid: u32,
    /// Plain-text body part, if available.
    pub text_plain: Option<String>,
    /// HTML body part, if available.
    pub text_html: Option<String>,
    /// List of attachments found in the message.
    pub attachments: Vec<ImapAttachment>,
}

/// Metadata about a single attachment in an email message.
#[derive(Debug, Clone, Serialize)]
pub struct ImapAttachment {
    /// Filename of the attachment, if provided by the sender.
    pub filename: Option<String>,
    /// MIME content type (e.g. "application/pdf").
    pub content_type: String,
    /// Size in bytes.
    pub size: usize,
    /// Raw attachment content.
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during IMAP operations.
#[derive(Debug)]
pub enum ImapError {
    /// Could not connect to the IMAP server.
    ConnectionFailed(String),
    /// The server rejected our credentials.
    AuthenticationFailed,
    /// The requested folder does not exist.
    FolderNotFound(String),
    /// The requested message UID was not found in the given folder.
    MessageNotFound { uid: u32, folder: String },
    /// A low-level IMAP protocol error.
    ProtocolError(String),
}

impl fmt::Display for ImapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImapError::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            ImapError::AuthenticationFailed => write!(f, "Authentication failed"),
            ImapError::FolderNotFound(name) => write!(f, "Folder not found: {name}"),
            ImapError::MessageNotFound { uid, folder } => {
                write!(f, "Message UID {uid} not found in folder {folder}")
            }
            ImapError::ProtocolError(msg) => write!(f, "Protocol error: {msg}"),
        }
    }
}

impl std::error::Error for ImapError {}

// ---------------------------------------------------------------------------
// Connection parameters (passed explicitly to every method)
// ---------------------------------------------------------------------------

/// Parameters needed to establish an IMAP connection.
/// Passed explicitly to every trait method so the trait stays stateless.
#[derive(Debug, Clone)]
pub struct ImapCredentials {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub email: String,
    pub password: String,
}

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// Abstraction over IMAP operations.
///
/// Every method receives explicit connection parameters so that the trait
/// remains stateless — no persistent connections are held.
///
/// The `Send + Sync` bounds allow implementations to be shared across
/// Tokio tasks and stored in `Arc`.
#[async_trait]
pub trait ImapClient: Send + Sync {
    /// List all folders (mailboxes) on the server.
    async fn list_folders(&self, creds: &ImapCredentials) -> Result<Vec<ImapFolder>, ImapError>;

    /// Fetch message headers (envelopes) for a range of UIDs in a folder.
    async fn fetch_headers(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid_range: &str,
    ) -> Result<Vec<ImapMessageHeader>, ImapError>;

    /// Fetch the full body of a single message by UID.
    async fn fetch_body(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
    ) -> Result<ImapMessageBody, ImapError>;

    /// Set (replace) the flags on a message.
    async fn set_flags(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
        flags: &[&str],
    ) -> Result<(), ImapError>;

    /// Move a message from one folder to another.
    async fn move_message(
        &self,
        creds: &ImapCredentials,
        from_folder: &str,
        uid: u32,
        to_folder: &str,
    ) -> Result<(), ImapError>;

    /// Permanently remove a message that has the `\Deleted` flag.
    async fn expunge_message(
        &self,
        creds: &ImapCredentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), ImapError>;
}

// ---------------------------------------------------------------------------
// Placeholder real implementation
// ---------------------------------------------------------------------------

/// Placeholder implementation that always returns
/// `ImapError::ConnectionFailed`. The real `async-imap`-backed
/// implementation will replace the method bodies in a later task.
pub struct RealImapClient;

#[async_trait]
impl ImapClient for RealImapClient {
    async fn list_folders(&self, _creds: &ImapCredentials) -> Result<Vec<ImapFolder>, ImapError> {
        Err(ImapError::ConnectionFailed(
            "Not yet implemented".to_string(),
        ))
    }

    async fn fetch_headers(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid_range: &str,
    ) -> Result<Vec<ImapMessageHeader>, ImapError> {
        Err(ImapError::ConnectionFailed(
            "Not yet implemented".to_string(),
        ))
    }

    async fn fetch_body(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
    ) -> Result<ImapMessageBody, ImapError> {
        Err(ImapError::ConnectionFailed(
            "Not yet implemented".to_string(),
        ))
    }

    async fn set_flags(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
        _flags: &[&str],
    ) -> Result<(), ImapError> {
        Err(ImapError::ConnectionFailed(
            "Not yet implemented".to_string(),
        ))
    }

    async fn move_message(
        &self,
        _creds: &ImapCredentials,
        _from_folder: &str,
        _uid: u32,
        _to_folder: &str,
    ) -> Result<(), ImapError> {
        Err(ImapError::ConnectionFailed(
            "Not yet implemented".to_string(),
        ))
    }

    async fn expunge_message(
        &self,
        _creds: &ImapCredentials,
        _folder: &str,
        _uid: u32,
    ) -> Result<(), ImapError> {
        Err(ImapError::ConnectionFailed(
            "Not yet implemented".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Mock implementation (test-only)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod mock {
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
        should_fail: Mutex<Option<ImapError>>,
    }

    impl MockImapClient {
        /// Create a new empty mock.
        pub fn new() -> Self {
            Self {
                folders: Mutex::new(Vec::new()),
                headers: Mutex::new(Vec::new()),
                bodies: Mutex::new(Vec::new()),
                should_fail: Mutex::new(None),
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
                    }],
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
        async fn real_imap_client_returns_not_implemented() {
            let client = RealImapClient;
            let creds = test_creds();

            let err = client.list_folders(&creds).await.unwrap_err();
            match err {
                ImapError::ConnectionFailed(msg) => {
                    assert_eq!(msg, "Not yet implemented");
                }
                other => panic!("Expected ConnectionFailed, got: {other}"),
            }
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
        async fn mock_empty_returns_empty_collections() {
            let mock = MockImapClient::new();
            let creds = test_creds();

            let folders = mock.list_folders(&creds).await.unwrap();
            assert!(folders.is_empty());

            let headers = mock.fetch_headers(&creds, "INBOX", "1:*").await.unwrap();
            assert!(headers.is_empty());
        }
    }
}
