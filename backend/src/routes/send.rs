use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};
use crate::smtp::client::{SendableMessage, SmtpClient, SmtpCredentials};

#[derive(Debug, Deserialize)]
pub struct SendRequest {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    pub text_body: Option<String>,
    pub html_body: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendResponse {
    pub status: String,
    pub message_id: String,
}

/// Handler for `POST /api/messages/send`.
///
/// Validates the request, sends the message via SMTP, and appends a copy
/// to the IMAP "Sent" folder (best-effort).
pub async fn send_message_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(smtp_client): Extension<Arc<dyn SmtpClient>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Json(req): Json<SendRequest>,
) -> Result<Response, AppError> {
    // Validate: at least one recipient.
    if req.to.is_empty() && req.cc.is_empty() && req.bcc.is_empty() {
        return Err(AppError::BadRequest(
            "At least one recipient is required".to_string(),
        ));
    }

    // Validate: subject or body must be non-empty.
    let has_subject = !req.subject.trim().is_empty();
    let has_text = req.text_body.as_deref().map_or(false, |t| !t.trim().is_empty());
    let has_html = req.html_body.as_deref().map_or(false, |h| !h.trim().is_empty());
    if !has_subject && !has_text && !has_html {
        return Err(AppError::BadRequest(
            "Subject or body is required".to_string(),
        ));
    }

    // Check that SMTP is configured.
    let smtp_host = config
        .smtp_host
        .as_deref()
        .ok_or_else(|| AppError::ServiceUnavailable("SMTP server not configured".to_string()))?;

    // Build SMTP credentials from config + session.
    let smtp_creds = SmtpCredentials {
        host: smtp_host.to_string(),
        port: config.smtp_port,
        tls: config.tls_enabled,
        email: session.email.clone(),
        password: session.password.clone(),
    };

    // Build the sendable message.
    let message = SendableMessage {
        from: session.email.clone(),
        to: req.to,
        cc: req.cc,
        bcc: req.bcc,
        subject: req.subject,
        text_body: req.text_body.unwrap_or_default(),
        html_body: req.html_body,
        in_reply_to: req.in_reply_to,
        references: req.references,
        attachments: vec![], // Phase 4 will add attachment support
    };

    // Send via SMTP.
    let message_id = smtp_client
        .send_message(&smtp_creds, &message)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("Failed to send email: {e}")))?;

    // Best-effort: append a copy to the "Sent" folder via IMAP.
    // Don't fail the send if this fails.
    if let Some(imap_host) = config.imap_host.as_deref() {
        let imap_creds = ImapCredentials {
            host: imap_host.to_string(),
            port: config.imap_port,
            tls: config.tls_enabled,
            email: session.email.clone(),
            password: session.password.clone(),
        };

        // Build RFC822 bytes for IMAP APPEND using lettre's Message builder.
        if let Ok(rfc822_bytes) = build_rfc822_bytes(&message, &message_id) {
            if let Err(e) = imap_client
                .append_message(&imap_creds, "Sent", &rfc822_bytes, &["\\Seen"])
                .await
            {
                tracing::warn!(error = %e, "Failed to append sent message to IMAP Sent folder");
            }
        }
    }

    Ok(Json(SendResponse {
        status: "sent".to_string(),
        message_id,
    })
    .into_response())
}

/// Build RFC822 bytes from a SendableMessage for IMAP APPEND.
fn build_rfc822_bytes(message: &SendableMessage, message_id: &str) -> Result<Vec<u8>, String> {
    use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};

    let from_mailbox: Mailbox = message
        .from
        .parse()
        .map_err(|e: lettre::address::AddressError| e.to_string())?;

    let mut builder = lettre::Message::builder()
        .from(from_mailbox)
        .subject(&message.subject)
        .message_id(Some(message_id.to_string()));

    for addr in &message.to {
        let mailbox: Mailbox = addr.parse().map_err(|e: lettre::address::AddressError| e.to_string())?;
        builder = builder.to(mailbox);
    }
    for addr in &message.cc {
        let mailbox: Mailbox = addr.parse().map_err(|e: lettre::address::AddressError| e.to_string())?;
        builder = builder.cc(mailbox);
    }
    if let Some(ref irt) = message.in_reply_to {
        builder = builder.in_reply_to(irt.clone());
    }
    if let Some(ref refs) = message.references {
        builder = builder.references(refs.clone());
    }

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

    let email = builder
        .multipart(body_part)
        .map_err(|e| e.to_string())?;

    Ok(email.formatted())
}
