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
    Json(req): Json<UpsertDraftRequest>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
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
    .map_err(|e| AppError::InternalError(e))?;

    Ok(Json(DraftResponse {
        id: req.id,
        status: "saved".to_string(),
    })
    .into_response())
}

/// `GET /api/drafts` — List all drafts.
pub async fn list_drafts_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let drafts = db::drafts::list_drafts(&conn).map_err(|e| AppError::InternalError(e))?;

    let items: Vec<DraftListItem> = drafts
        .into_iter()
        .map(|d| DraftListItem {
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
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let draft = db::drafts::get_draft(&conn, &id)
        .map_err(|e| AppError::InternalError(e))?
        .ok_or_else(|| AppError::NotFound("Draft not found".to_string()))?;

    let attachments = db::drafts::get_draft_attachments(&conn, &id)
        .map_err(|e| AppError::InternalError(e))?;

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
    AxumPath(id): AxumPath<String>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let deleted =
        db::drafts::delete_draft(&conn, &id).map_err(|e| AppError::InternalError(e))?;

    if !deleted {
        return Err(AppError::NotFound("Draft not found".to_string()));
    }

    // Clean up attachment files from disk.
    let att_dir = Path::new(&config.data_dir)
        .join(&session.user_hash)
        .join("attachments")
        .join(&id);
    if att_dir.exists() {
        if let Err(e) = tokio::fs::remove_dir_all(&att_dir).await {
            tracing::warn!(error = %e, path = %att_dir.display(), "Failed to clean up attachment directory");
        }
    }

    Ok(Json(DeleteResponse {
        status: "deleted".to_string(),
    })
    .into_response())
}
