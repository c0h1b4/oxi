use std::path::Path;
use std::sync::Arc;

use axum::extract::Path as AxumPath;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};

#[cfg(test)]
#[path = "drafts_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpsertDraftRequest {
    pub id: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub cc: String,
    #[serde(default)]
    pub bcc: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub text_body: String,
    pub html_body: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Option<String>,
}

#[derive(Debug, Serialize)]
struct DraftResponse {
    id: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct DraftDetail {
    id: String,
    to: String,
    cc: String,
    bcc: String,
    subject: String,
    text_body: String,
    html_body: Option<String>,
    in_reply_to: Option<String>,
    references: Option<String>,
    created_at: String,
    updated_at: String,
    attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Serialize)]
struct AttachmentInfo {
    id: String,
    filename: String,
    content_type: String,
    size: i64,
}

#[derive(Debug, Serialize)]
struct DraftListItem {
    id: String,
    imap_message_id: String,
    to: String,
    subject: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct DeleteResponse {
    status: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/drafts` — Create or update a draft.
pub async fn upsert_draft_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Json(req): Json<UpsertDraftRequest>,
) -> Result<Response, AppError> {
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    db::drafts::upsert_draft(
        &conn,
        &req.id,
        &req.to,
        &req.cc,
        &req.bcc,
        &req.subject,
        &req.text_body,
        req.html_body.as_deref(),
        req.in_reply_to.as_deref(),
        req.references.as_deref(),
    )
    .await
    .map_err(AppError::InternalError)?;

    // Read the old UIDs before APPEND, and remove them only after the new copy
    // exists. A failed append must never destroy the last server-side draft.
    if let Some((creds, folder)) = draft_mailbox(&config, &session).await? {
        let message_id = draft_message_id(&req.id);
        let sync_result = async {
            let old_uids = draft_uids(imap_client.as_ref(), &creds, &folder, &message_id).await?;
            let bytes = build_draft_rfc822(&req, &session.email)?;
            imap_client
                .append_message(&creds, &folder, &bytes, &["\\Draft", "\\Seen"])
                .await
                .map_err(|e| e.to_string())?;
            remove_draft_copies(
                imap_client.as_ref(),
                &creds,
                &folder,
                &old_uids,
                &config,
                &session,
            )
            .await
        }
        .await;
        if let Err(e) = sync_result {
            tracing::warn!(error = %e, "Failed to synchronize IMAP draft");
        }
    }

    Ok(Json(DraftResponse {
        id: req.id,
        status: "saved".to_string(),
    })
    .into_response())
}

/// Stable, header-safe identity shared by every saved revision of a draft.
fn draft_message_id(id: &str) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "<oxi-draft-{:x}@draft.oxi.local>",
        Sha256::digest(id.as_bytes())
    )
}

async fn draft_mailbox(
    config: &AppConfig,
    session: &SessionState,
) -> Result<Option<(ImapCredentials, String)>, AppError> {
    let Some(host) = config.imap_host.as_ref() else {
        return Ok(None);
    };
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;
    let folder = db::folders::find_folder_by_attribute(&conn, "\\Drafts")
        .await
        .map_err(AppError::InternalError)?
        .unwrap_or_else(|| "Drafts".to_string());
    Ok(Some((
        ImapCredentials {
            host: host.clone(),
            port: config.imap_port,
            tls: config.tls_enabled,
            email: session.email.clone(),
            password: session.password.clone(),
        },
        folder,
    )))
}

async fn draft_uids(
    client: &dyn ImapClient,
    creds: &ImapCredentials,
    folder: &str,
    message_id: &str,
) -> Result<Vec<u32>, String> {
    Ok(client
        .fetch_headers(creds, folder, "1:*")
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|header| header.message_id.as_deref() == Some(message_id))
        .map(|header| header.uid)
        .collect())
}

async fn remove_draft_copies(
    client: &dyn ImapClient,
    creds: &ImapCredentials,
    folder: &str,
    uids: &[u32],
    config: &AppConfig,
    session: &SessionState,
) -> Result<(), String> {
    for &uid in uids {
        client
            .expunge_message(creds, folder, uid)
            .await
            .map_err(|e| e.to_string())?;
        let conn = config
            .user_db(&session.user_hash)
            .await
            .map_err(|e| e.to_string())?;
        db::messages::remove_after_mutation(&conn, folder, uid).await?;
    }
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| e.to_string())?;
    db::folders::invalidate_folder_freshness(&conn, folder).await?;
    Ok(())
}

pub(super) async fn delete_imap_draft(
    client: &dyn ImapClient,
    config: &AppConfig,
    session: &SessionState,
    id: &str,
) -> Result<(), AppError> {
    if let Some((creds, folder)) = draft_mailbox(config, session).await? {
        let uids = draft_uids(client, &creds, &folder, &draft_message_id(id))
            .await
            .map_err(AppError::InternalError)?;
        remove_draft_copies(client, &creds, &folder, &uids, config, session)
            .await
            .map_err(AppError::InternalError)?;
    }
    Ok(())
}

/// Build a minimal RFC822 message from draft fields for IMAP APPEND.
fn build_draft_rfc822(req: &UpsertDraftRequest, from_email: &str) -> Result<Vec<u8>, String> {
    use lettre::message::{Mailbox, MultiPart, SinglePart, header::ContentType};

    let from_mailbox: Mailbox = from_email
        .parse()
        .map_err(|e: lettre::address::AddressError| e.to_string())?;

    let mut builder = lettre::Message::builder()
        .from(from_mailbox)
        .subject(&req.subject)
        .message_id(Some(draft_message_id(&req.id)));

    // Parse comma-separated address lists.
    for addr in req.to.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(mailbox) = addr.parse::<Mailbox>() {
            builder = builder.to(mailbox);
        }
    }
    for addr in req.cc.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(mailbox) = addr.parse::<Mailbox>() {
            builder = builder.cc(mailbox);
        }
    }
    if let Some(ref irt) = req.in_reply_to {
        builder = builder.in_reply_to(irt.clone());
    }
    if let Some(ref refs) = req.references {
        builder = builder.references(refs.clone());
    }

    let email = if let Some(ref html) = req.html_body {
        builder
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .content_type(ContentType::TEXT_PLAIN)
                            .body(req.text_body.clone()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .content_type(ContentType::TEXT_HTML)
                            .body(html.clone()),
                    ),
            )
            .map_err(|e| e.to_string())?
    } else {
        builder
            .body(req.text_body.clone())
            .map_err(|e| e.to_string())?
    };

    Ok(email.formatted())
}

/// `GET /api/drafts` — List all drafts.
pub async fn list_drafts_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let drafts = db::drafts::list_drafts(&conn)
        .await
        .map_err(AppError::InternalError)?;

    let items: Vec<DraftListItem> = drafts
        .into_iter()
        .map(|d| DraftListItem {
            imap_message_id: draft_message_id(&d.id),
            id: d.id,
            to: d.to_addresses,
            subject: d.subject,
            updated_at: d.updated_at,
        })
        .collect();

    Ok(Json(serde_json::json!({ "drafts": items })).into_response())
}

/// `GET /api/drafts/{id}` — Get a single draft with its attachments.
pub async fn get_draft_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, AppError> {
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let draft = db::drafts::get_draft(&conn, &id)
        .await
        .map_err(AppError::InternalError)?
        .ok_or_else(|| AppError::NotFound("Draft not found".to_string()))?;

    let attachments = db::drafts::get_draft_attachments(&conn, &id)
        .await
        .map_err(AppError::InternalError)?;

    let att_infos: Vec<AttachmentInfo> = attachments
        .into_iter()
        .map(|a| AttachmentInfo {
            id: a.id,
            filename: a.filename,
            content_type: a.content_type,
            size: a.size,
        })
        .collect();

    Ok(Json(DraftDetail {
        id: draft.id,
        to: draft.to_addresses,
        cc: draft.cc_addresses,
        bcc: draft.bcc_addresses,
        subject: draft.subject,
        text_body: draft.text_body,
        html_body: draft.html_body,
        in_reply_to: draft.in_reply_to,
        references: draft.references_header,
        created_at: draft.created_at,
        updated_at: draft.updated_at,
        attachments: att_infos,
    })
    .into_response())
}

/// `DELETE /api/drafts/{id}` — Delete a draft and its attachments.
pub async fn delete_draft_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    AxumPath(id): AxumPath<String>,
) -> Result<Response, AppError> {
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    delete_imap_draft(imap_client.as_ref(), &config, &session, &id).await?;

    let deleted = db::drafts::delete_draft(&conn, &id)
        .await
        .map_err(AppError::InternalError)?;

    if !deleted {
        return Err(AppError::NotFound("Draft not found".to_string()));
    }

    // Clean up attachment files from disk.
    let att_dir = Path::new(&config.data_dir)
        .join(&session.user_hash)
        .join("attachments")
        .join(&id);
    if att_dir.exists()
        && let Err(e) = tokio::fs::remove_dir_all(&att_dir).await
    {
        tracing::warn!(error = %e, path = %att_dir.display(), "Failed to clean up attachment directory");
    }

    Ok(Json(DeleteResponse {
        status: "deleted".to_string(),
    })
    .into_response())
}
