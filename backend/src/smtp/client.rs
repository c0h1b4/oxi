//! SMTP client abstraction and real implementation.

use async_trait::async_trait;

// Import and re-export types for backward compatibility
pub use crate::smtp::error::SmtpError;
pub use crate::smtp::types::{AttachmentData, SendableMessage, SmtpCredentials};

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

        // Separate inline images (those with content_id referenced in HTML)
        // from regular file attachments.
        let html_body = message.html_body.as_deref().unwrap_or("");
        let (inline_atts, file_atts): (Vec<_>, Vec<_>) =
            message.attachments.iter().partition(|att| {
                att.content_id
                    .as_ref()
                    .is_some_and(|cid| html_body.contains(&format!("cid:{cid}")))
            });

        // Build the body part(s).
        let body_part = if let Some(ref html) = message.html_body {
            if inline_atts.is_empty() {
                // Simple alternative: text + HTML.
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
                // Alternative with related HTML part containing inline images.
                let mut related = MultiPart::related().singlepart(
                    SinglePart::builder()
                        .content_type(ContentType::TEXT_HTML)
                        .body(html.clone()),
                );
                for att in &inline_atts {
                    let ct: ContentType = att
                        .content_type
                        .parse()
                        .unwrap_or(ContentType::TEXT_PLAIN);
                    let cid = att.content_id.as_deref().unwrap_or("unknown");
                    let inline_part =
                        Attachment::new_inline(cid.to_string()).body(att.data.clone(), ct);
                    related = related.singlepart(inline_part);
                }

                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .content_type(ContentType::TEXT_PLAIN)
                            .body(message.text_body.clone()),
                    )
                    .multipart(related)
            }
        } else {
            MultiPart::alternative().singlepart(
                SinglePart::builder()
                    .content_type(ContentType::TEXT_PLAIN)
                    .body(message.text_body.clone()),
            )
        };

        // Build the final email — wrap in mixed multipart if there are file attachments.
        let email = if file_atts.is_empty() {
            builder
                .multipart(body_part)
                .map_err(|e| SmtpError::SendFailed(e.to_string()))?
        } else {
            let mut mixed = MultiPart::mixed().multipart(body_part);
            for att in &file_atts {
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
#[path = "mock.rs"]
pub mod mock;
