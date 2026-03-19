use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};

#[derive(Serialize)]
struct QuotaResponse {
    /// Storage used in bytes, or null if unavailable.
    usage_bytes: Option<u64>,
    /// Storage limit in bytes, or null if unlimited/unavailable.
    limit_bytes: Option<u64>,
}

/// `GET /api/quota`
///
/// Returns the mailbox storage quota for the authenticated user.
/// Tries IMAP GETQUOTAROOT first; if no quota is configured, falls back to
/// summing RFC822.SIZE across all folders via IMAP FETCH.
pub async fn get_quota(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
) -> Result<Response, AppError> {
    let imap_host = config
        .imap_host
        .as_deref()
        .ok_or_else(|| AppError::ServiceUnavailable("Mail server not configured".to_string()))?;

    let creds = ImapCredentials {
        host: imap_host.to_string(),
        port: config.imap_port,
        tls: config.tls_enabled,
        email: session.email.clone(),
        password: session.password.clone(),
    };

    // Try IMAP GETQUOTAROOT first.
    let quota = imap_client.get_quota(&creds).await.unwrap_or(None);

    let resp = match quota {
        Some(q) => QuotaResponse {
            usage_bytes: Some(q.usage_bytes),
            limit_bytes: if q.limit_bytes > 0 { Some(q.limit_bytes) } else { None },
        },
        None => {
            // No IMAP QUOTA — sum RFC822.SIZE across all folders.
            let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
            let folders: Vec<String> = db::folders::get_all_folders(&conn)
                .unwrap_or_default()
                .into_iter()
                .map(|f| f.name)
                .collect();

            let mut total: u64 = 0;
            for folder in &folders {
                if let Ok(size) = imap_client.fetch_folder_size(&creds, folder).await {
                    total += size;
                }
            }

            QuotaResponse {
                usage_bytes: if total > 0 { Some(total) } else { None },
                limit_bytes: None,
            }
        }
    };

    Ok(Json(resp).into_response())
}
