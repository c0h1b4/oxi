use std::path::Path;
use std::sync::Arc;

use axum::extract::Path as AxumPath;
use axum::extract::Multipart;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;

/// Maximum attachment file size: 25 MB.
const MAX_ATTACHMENT_SIZE: usize = 25 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct UploadResponse {
    id: String,
    filename: String,
    content_type: String,
    size: i64,
}

#[derive(Debug, Serialize)]
struct DeleteResponse {
    status: String,
}

/// Handler for `POST /api/drafts/{draft_id}/attachments`.
///
/// Accepts a multipart upload containing one or more files. Each file is
/// saved to disk and recorded in the `draft_attachments` table.
pub async fn upload_attachment(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    AxumPath(draft_id): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    // Ensure draft exists (create it if not — auto-creates on first attachment upload).
    let existing = db::drafts::get_draft(&conn, &draft_id)
        .map_err(AppError::InternalError)?;
    if existing.is_none() {
        db::drafts::upsert_draft(&conn, &draft_id, "", "", "", "", "", None, None, None)
            .map_err(AppError::InternalError)?;
    }

    // Build the attachment storage directory.
    let att_dir = Path::new(&config.data_dir)
        .join(&session.user_hash)
        .join("attachments")
        .join(&draft_id);
    tokio::fs::create_dir_all(&att_dir)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to create attachment dir: {e}")))?;

    let mut uploaded = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid multipart data: {e}")))?
    {
        let filename = field
            .file_name()
            .unwrap_or("unnamed")
            .to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("Failed to read field data: {e}")))?;

        if data.len() > MAX_ATTACHMENT_SIZE {
            return Err(AppError::BadRequest(format!(
                "File '{}' exceeds maximum size of 25 MB",
                filename
            )));
        }

        let att_id = uuid::Uuid::new_v4().to_string();
        let file_path = att_dir.join(&att_id);

        tokio::fs::write(&file_path, &data)
            .await
            .map_err(|e| AppError::InternalError(format!("Failed to write attachment: {e}")))?;

        let size = data.len() as i64;

        db::drafts::add_draft_attachment(
            &conn,
            &att_id,
            &draft_id,
            &filename,
            &content_type,
            size,
            file_path.to_str().unwrap_or(""),
        )
        .map_err(AppError::InternalError)?;

        uploaded.push(UploadResponse {
            id: att_id,
            filename,
            content_type,
            size,
        });
    }

    if uploaded.is_empty() {
        return Err(AppError::BadRequest("No files uploaded".to_string()));
    }

    Ok(Json(serde_json::json!({
        "attachments": uploaded,
    }))
    .into_response())
}

/// Handler for `GET /api/drafts/{draft_id}/attachments/{attachment_id}/content`.
///
/// Serves the raw file content of a draft attachment for inline preview.
pub async fn get_attachment_content(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    AxumPath((draft_id, attachment_id)): AxumPath<(String, String)>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    let attachments = db::drafts::get_draft_attachments(&conn, &draft_id)
        .map_err(AppError::InternalError)?;

    let attachment = attachments
        .iter()
        .find(|a| a.id == attachment_id)
        .ok_or_else(|| AppError::NotFound("Attachment not found".to_string()))?;

    let data = tokio::fs::read(&attachment.file_path)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to read attachment file: {e}")))?;

    Ok(Response::builder()
        .header("content-type", &attachment.content_type)
        .header("cache-control", "private, max-age=3600")
        .body(axum::body::Body::from(data))
        .unwrap())
}

/// Handler for `DELETE /api/drafts/{draft_id}/attachments/{attachment_id}`.
///
/// Removes an attachment from the database and deletes the file from disk.
pub async fn delete_attachment(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    AxumPath((draft_id, attachment_id)): AxumPath<(String, String)>,
) -> Result<Response, AppError> {
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Failed to open database: {e}")))?;

    // Get the attachment record so we can find the file path.
    let attachments = db::drafts::get_draft_attachments(&conn, &draft_id)
        .map_err(AppError::InternalError)?;

    let attachment = attachments
        .iter()
        .find(|a| a.id == attachment_id)
        .ok_or_else(|| AppError::NotFound("Attachment not found".to_string()))?;

    // Delete file from disk (best-effort).
    let file_path = attachment.file_path.clone();
    if let Err(e) = tokio::fs::remove_file(&file_path).await {
        tracing::warn!(error = %e, path = %file_path, "Failed to delete attachment file from disk");
    }

    // Delete from DB.
    db::drafts::delete_draft_attachment(&conn, &attachment_id)
        .map_err(AppError::InternalError)?;

    Ok(Json(DeleteResponse {
        status: "deleted".to_string(),
    })
    .into_response())
}
