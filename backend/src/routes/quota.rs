use std::sync::Arc;
use std::time::Duration;

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
/// the cached message sizes (an estimate during sync). An empty cache uses IMAP.
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
            limit_bytes: if q.limit_bytes > 0 {
                Some(q.limit_bytes)
            } else {
                None
            },
        },
        None => {
            let database = config
                .user_db(&session.user_hash)
                .await
                .map_err(AppError::InternalError)?;
            if let Some(bytes) = db::messages::cached_storage_bytes(&database)
                .await
                .map_err(AppError::InternalError)?
            {
                return Ok(Json(QuotaResponse {
                    usage_bytes: Some(bytes),
                    limit_bytes: None,
                })
                .into_response());
            }
            let folders: Vec<String> = db::folders::get_all_folders(&database)
                .await
                .map_err(AppError::InternalError)?
                .into_iter()
                .map(|folder| folder.name)
                .collect();

            // Fetch sizes with a 90s overall timeout.
            let total = tokio::time::timeout(
                Duration::from_secs(90),
                crate::imap::pool::background(async {
                    let mut sum: u64 = 0;
                    for folder in &folders {
                        sum += imap_client.fetch_folder_size(&creds, folder).await?;
                    }
                    Ok::<_, crate::imap::error::ImapError>(sum)
                }),
            )
            .await;
            let total = match total {
                Ok(Ok(total)) => Some(total),
                other => {
                    tracing::warn!(error = ?other, "Quota folder-size fallback failed");
                    None
                }
            };

            QuotaResponse {
                usage_bytes: total,
                limit_bytes: None,
            }
        }
    };

    Ok(Json(resp).into_response())
}
