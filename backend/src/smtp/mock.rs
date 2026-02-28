//! Mock SMTP client for testing.

use super::{AttachmentData, SendableMessage, SmtpClient, SmtpCredentials, SmtpError};
use async_trait::async_trait;
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

impl Default for MockSmtpClient {
    fn default() -> Self {
        Self::new()
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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
                content_id: None,
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
        let mock = MockSmtpClient::new().with_error(SmtpError::AuthenticationFailed);

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
        use crate::smtp::client::RealSmtpClient;

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
