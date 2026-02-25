use async_trait::async_trait;
use std::fmt;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Parameters needed to establish an SMTP connection.
/// Passed explicitly to every trait method so the trait stays stateless.
#[derive(Debug, Clone)]
pub struct SmtpCredentials {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub email: String,
    pub password: String,
}

/// A message ready to be sent via SMTP.
#[derive(Debug, Clone)]
pub struct SendableMessage {
    /// Sender email address.
    pub from: String,
    /// Primary recipients.
    pub to: Vec<String>,
    /// CC recipients.
    pub cc: Vec<String>,
    /// BCC recipients.
    pub bcc: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// Plain-text body.
    pub text_body: String,
    /// Optional HTML body.
    pub html_body: Option<String>,
    /// In-Reply-To header value for threading.
    pub in_reply_to: Option<String>,
    /// References header value for threading.
    pub references: Option<String>,
    /// File attachments.
    pub attachments: Vec<AttachmentData>,
}

/// A single file attachment to include in an outgoing message.
#[derive(Debug, Clone)]
pub struct AttachmentData {
    /// Filename as it should appear to the recipient.
    pub filename: String,
    /// MIME content type (e.g. "application/pdf").
    pub content_type: String,
    /// Raw file content.
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during SMTP operations.
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum SmtpError {
    /// Could not connect to the SMTP server.
    ConnectionFailed(String),
    /// The server rejected our credentials.
    AuthenticationFailed,
    /// The message could not be sent.
    SendFailed(String),
}

impl fmt::Display for SmtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmtpError::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            SmtpError::AuthenticationFailed => write!(f, "Authentication failed"),
            SmtpError::SendFailed(msg) => write!(f, "Send failed: {msg}"),
        }
    }
}

impl std::error::Error for SmtpError {}

// ---------------------------------------------------------------------------
// Trait definition
// ---------------------------------------------------------------------------

/// Abstraction over SMTP operations.
///
/// Every method receives explicit connection parameters so that the trait
/// remains stateless — no persistent connections are held.
///
/// The `Send + Sync` bounds allow implementations to be shared across
/// Tokio tasks and stored in `Arc`.
#[async_trait]
pub trait SmtpClient: Send + Sync {
    /// Send an email message. Returns the generated Message-ID on success.
    async fn send_message(
        &self,
        creds: &SmtpCredentials,
        message: &SendableMessage,
    ) -> Result<String, SmtpError>;
}

// ---------------------------------------------------------------------------
// Real implementation backed by lettre
// ---------------------------------------------------------------------------

/// Production SMTP client that uses `lettre`.
///
/// This is a stateless unit struct — every method creates a fresh connection,
/// performs the operation, and disconnects.
pub struct RealSmtpClient;

#[async_trait]
impl SmtpClient for RealSmtpClient {
    async fn send_message(
        &self,
        creds: &SmtpCredentials,
        message: &SendableMessage,
    ) -> Result<String, SmtpError> {
        use lettre::message::{
            header::ContentType, Attachment, Mailbox, MessageBuilder, MultiPart, SinglePart,
        };
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

        // Generate a unique Message-ID.
        let message_id = format!(
            "<{}.{}@{}>",
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            creds.host
        );

        // Build the email message.
        let from_mailbox: Mailbox = message
            .from
            .parse()
            .map_err(|e: lettre::address::AddressError| SmtpError::SendFailed(e.to_string()))?;

        let mut builder: MessageBuilder = lettre::Message::builder()
            .from(from_mailbox)
            .subject(&message.subject)
            .message_id(Some(message_id.clone()));

        // Add To recipients.
        for addr in &message.to {
            let mailbox: Mailbox = addr
                .parse()
                .map_err(|e: lettre::address::AddressError| SmtpError::SendFailed(e.to_string()))?;
            builder = builder.to(mailbox);
        }

        // Add CC recipients.
        for addr in &message.cc {
            let mailbox: Mailbox = addr
                .parse()
                .map_err(|e: lettre::address::AddressError| SmtpError::SendFailed(e.to_string()))?;
            builder = builder.cc(mailbox);
        }

        // Add BCC recipients.
        for addr in &message.bcc {
            let mailbox: Mailbox = addr
                .parse()
                .map_err(|e: lettre::address::AddressError| SmtpError::SendFailed(e.to_string()))?;
            builder = builder.bcc(mailbox);
        }

        // Add In-Reply-To header.
        if let Some(ref irt) = message.in_reply_to {
            builder = builder.in_reply_to(irt.clone());
        }

        // Add References header.
        if let Some(ref refs) = message.references {
            builder = builder.references(refs.clone());
        }

        // Build the body part(s).
        let body_part = if let Some(ref html) = message.html_body {
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .content_type(ContentType::TEXT_PLAIN)
                        .body(message.text_body.clone()),
                )
                .singlepart(
                    SinglePart::builder()
                        .content_type(ContentType::TEXT_HTML)
                        .body(html.clone()),
                )
        } else {
            MultiPart::alternative().singlepart(
                SinglePart::builder()
                    .content_type(ContentType::TEXT_PLAIN)
                    .body(message.text_body.clone()),
            )
        };

        // Build the final email — wrap in mixed multipart if there are attachments.
        let email = if message.attachments.is_empty() {
            builder
                .multipart(body_part)
                .map_err(|e| SmtpError::SendFailed(e.to_string()))?
        } else {
            let mut mixed = MultiPart::mixed().multipart(body_part);
            for att in &message.attachments {
                let ct: ContentType = att
                    .content_type
                    .parse()
                    .unwrap_or(ContentType::TEXT_PLAIN);
                let attachment =
                    Attachment::new(att.filename.clone()).body(att.data.clone(), ct);
                mixed = mixed.singlepart(attachment);
            }
            builder
                .multipart(mixed)
                .map_err(|e| SmtpError::SendFailed(e.to_string()))?
        };

        // Build the SMTP transport.
        let smtp_creds =
            Credentials::new(creds.email.clone(), creds.password.clone());

        let transport: AsyncSmtpTransport<Tokio1Executor> = if creds.tls && creds.port == 587 {
            // STARTTLS on port 587.
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&creds.host)
                .map_err(|e| SmtpError::ConnectionFailed(e.to_string()))?
                .port(creds.port)
                .credentials(smtp_creds)
                .build()
        } else if creds.tls {
            // Implicit TLS (typically port 465).
            AsyncSmtpTransport::<Tokio1Executor>::relay(&creds.host)
                .map_err(|e| SmtpError::ConnectionFailed(e.to_string()))?
                .port(creds.port)
                .credentials(smtp_creds)
                .build()
        } else {
            // No TLS — dangerous / plaintext.
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&creds.host)
                .port(creds.port)
                .credentials(smtp_creds)
                .build()
        };

        // Send the message.
        transport.send(email).await.map_err(|e| {
            let msg = e.to_string();
            if msg.to_lowercase().contains("authentication")
                || msg.to_lowercase().contains("credentials")
                || msg.to_lowercase().contains("auth")
            {
                SmtpError::AuthenticationFailed
            } else if msg.to_lowercase().contains("connect")
                || msg.to_lowercase().contains("dns")
                || msg.to_lowercase().contains("resolve")
                || msg.to_lowercase().contains("timeout")
                || msg.to_lowercase().contains("tls")
            {
                SmtpError::ConnectionFailed(msg)
            } else {
                SmtpError::SendFailed(msg)
            }
        })?;

        Ok(message_id)
    }
}

// ---------------------------------------------------------------------------
// Mock implementation (test-only)
// ---------------------------------------------------------------------------

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::Mutex;

    /// A mock SMTP client that records sent messages.
    ///
    /// Uses interior mutability (`Mutex`) so it can be shared behind `&self`.
    /// Build it up with the `.with_*()` builder methods, then pass it wherever
    /// an `&dyn SmtpClient` is needed.
    pub struct MockSmtpClient {
        should_fail: Mutex<Option<SmtpError>>,
        sent_messages: Mutex<Vec<SendableMessage>>,
    }

    impl MockSmtpClient {
        /// Create a new empty mock.
        pub fn new() -> Self {
            Self {
                should_fail: Mutex::new(None),
                sent_messages: Mutex::new(Vec::new()),
            }
        }

        /// Make every subsequent call return this error.
        pub fn with_error(self, error: SmtpError) -> Self {
            *self.should_fail.lock().unwrap() = Some(error);
            self
        }

        /// Return the number of messages sent through this mock.
        pub fn sent_count(&self) -> usize {
            self.sent_messages.lock().unwrap().len()
        }

        /// Return a clone of the most recently sent message, if any.
        pub fn last_sent(&self) -> Option<SendableMessage> {
            self.sent_messages.lock().unwrap().last().cloned()
        }
    }

    /// Helper to clone an `SmtpError` for the mock (the real errors are not
    /// `Clone`, so we reconstruct them).
    fn clone_error(err: &SmtpError) -> SmtpError {
        match err {
            SmtpError::ConnectionFailed(msg) => SmtpError::ConnectionFailed(msg.clone()),
            SmtpError::AuthenticationFailed => SmtpError::AuthenticationFailed,
            SmtpError::SendFailed(msg) => SmtpError::SendFailed(msg.clone()),
        }
    }

    #[async_trait]
    impl SmtpClient for MockSmtpClient {
        async fn send_message(
            &self,
            _creds: &SmtpCredentials,
            message: &SendableMessage,
        ) -> Result<String, SmtpError> {
            if let Some(ref err) = *self.should_fail.lock().unwrap() {
                return Err(clone_error(err));
            }
            self.sent_messages.lock().unwrap().push(message.clone());
            Ok(format!("<mock-{}>", uuid::Uuid::new_v4()))
        }
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Convenience helper to build dummy credentials for tests.
        fn test_creds() -> SmtpCredentials {
            SmtpCredentials {
                host: "smtp.example.com".to_string(),
                port: 587,
                tls: true,
                email: "user@example.com".to_string(),
                password: "hunter2".to_string(),
            }
        }

        /// Build a minimal test message.
        fn test_message() -> SendableMessage {
            SendableMessage {
                from: "user@example.com".to_string(),
                to: vec!["recipient@example.com".to_string()],
                cc: vec![],
                bcc: vec![],
                subject: "Test subject".to_string(),
                text_body: "Hello, world!".to_string(),
                html_body: None,
                in_reply_to: None,
                references: None,
                attachments: vec![],
            }
        }

        #[tokio::test]
        async fn mock_send_succeeds_without_error() {
            let mock = MockSmtpClient::new();
            let result = mock.send_message(&test_creds(), &test_message()).await;
            assert!(result.is_ok());
            let message_id = result.unwrap();
            assert!(!message_id.is_empty());
        }

        #[tokio::test]
        async fn mock_send_captures_message() {
            let mock = MockSmtpClient::new();
            let msg = SendableMessage {
                from: "sender@example.com".to_string(),
                to: vec!["alice@example.com".to_string()],
                cc: vec!["bob@example.com".to_string()],
                bcc: vec![],
                subject: "Important".to_string(),
                text_body: "Please read.".to_string(),
                html_body: Some("<p>Please read.</p>".to_string()),
                in_reply_to: Some("<original@example.com>".to_string()),
                references: Some("<original@example.com>".to_string()),
                attachments: vec![AttachmentData {
                    filename: "notes.txt".to_string(),
                    content_type: "text/plain".to_string(),
                    data: b"some content".to_vec(),
                }],
            };

            let result = mock.send_message(&test_creds(), &msg).await;
            assert!(result.is_ok());
            assert_eq!(mock.sent_count(), 1);

            let captured = mock.last_sent().unwrap();
            assert_eq!(captured.from, "sender@example.com");
            assert_eq!(captured.to, vec!["alice@example.com"]);
            assert_eq!(captured.cc, vec!["bob@example.com"]);
            assert_eq!(captured.subject, "Important");
            assert_eq!(captured.text_body, "Please read.");
            assert_eq!(
                captured.html_body.as_deref(),
                Some("<p>Please read.</p>")
            );
            assert_eq!(
                captured.in_reply_to.as_deref(),
                Some("<original@example.com>")
            );
            assert_eq!(
                captured.references.as_deref(),
                Some("<original@example.com>")
            );
            assert_eq!(captured.attachments.len(), 1);
            assert_eq!(captured.attachments[0].filename, "notes.txt");
        }

        #[tokio::test]
        async fn mock_with_error_returns_error() {
            let mock = MockSmtpClient::new()
                .with_error(SmtpError::AuthenticationFailed);

            let err = mock
                .send_message(&test_creds(), &test_message())
                .await
                .unwrap_err();
            assert!(matches!(err, SmtpError::AuthenticationFailed));

            // Ensure no messages were recorded.
            assert_eq!(mock.sent_count(), 0);
        }

        #[tokio::test]
        async fn real_smtp_connection_fails_with_bad_host() {
            let client = RealSmtpClient;
            let creds = SmtpCredentials {
                host: "invalid.host.test".to_string(),
                port: 587,
                tls: true,
                email: "user@invalid.host.test".to_string(),
                password: "password".to_string(),
            };
            let msg = test_message();

            let err = client.send_message(&creds, &msg).await.unwrap_err();
            // With a fake host the connection should fail.
            assert!(
                matches!(err, SmtpError::ConnectionFailed(_) | SmtpError::SendFailed(_)),
                "Expected ConnectionFailed or SendFailed, got: {err}"
            );
        }

        #[tokio::test]
        async fn smtp_error_display_formats_correctly() {
            let cases: Vec<(SmtpError, &str)> = vec![
                (
                    SmtpError::ConnectionFailed("timeout".to_string()),
                    "Connection failed: timeout",
                ),
                (SmtpError::AuthenticationFailed, "Authentication failed"),
                (
                    SmtpError::SendFailed("rejected by server".to_string()),
                    "Send failed: rejected by server",
                ),
            ];

            for (err, expected) in cases {
                assert_eq!(err.to_string(), expected);
            }
        }
    }
}
