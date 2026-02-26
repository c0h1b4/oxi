use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Serialize;

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};

/// Response envelope for `GET /api/folders`.
#[derive(Serialize)]
struct FoldersResponse {
    folders: Vec<FolderEntry>,
}

/// A single folder in the response.
#[derive(Serialize)]
struct FolderEntry {
    name: String,
    delimiter: Option<String>,
    attributes: Vec<String>,
    is_subscribed: bool,
    total_count: u32,
    unread_count: u32,
}

/// How many seconds the folder list cache is considered fresh.
const FOLDER_LIST_TTL_SECS: u32 = 30;

/// `GET /api/folders`
///
/// Lists all IMAP folders for the authenticated user, syncing the result
/// into the per-user SQLite cache.  If the cache was refreshed within
/// `FOLDER_LIST_TTL_SECS` seconds, IMAP is skipped entirely.
pub async fn list_folders(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
) -> Result<Response, AppError> {
    // Open the per-user database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // If the folder cache was updated recently, skip the IMAP round-trip.
    let cache_fresh = db::folders::is_folder_cache_fresh(&conn, FOLDER_LIST_TTL_SECS)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if !cache_fresh {
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

        // Fetch folders from IMAP server.
        let imap_folders = imap_client
            .list_folders(&creds)
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

        // Sync each folder into SQLite cache.
        // Use INSERT OR IGNORE to create new folders without triggering CASCADE on
        // existing ones, then UPDATE the metadata fields separately.
        for folder in &imap_folders {
            let flags_csv = folder.attributes.join(",");
            db::folders::insert_folder_if_new(
                &conn,
                &folder.name,
                folder.delimiter.as_deref(),
                &flags_csv,
            )
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        }

        // Remove stale folders that no longer exist on the server.
        let current_names: Vec<String> = imap_folders.iter().map(|f| f.name.clone()).collect();
        db::folders::remove_stale_folders(&conn, &current_names)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        // Touch updated_at on all folders so the cache TTL resets.
        db::folders::touch_all_folders(&conn)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    }

    // Refresh unread counts from cached messages — but skip folders whose
    // messages cache has been invalidated (messages_updated_at IS NULL).
    // Those folders have a manually adjusted unread_count (via adjust_unread_count)
    // that should be preserved until the folder's messages are resynced from IMAP.
    let all_folders = db::folders::get_all_folders(&conn)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    for f in &all_folders {
        let invalidated = db::folders::is_folder_messages_invalidated(&conn, &f.name)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        if !invalidated {
            db::folders::refresh_unread_count(&conn, &f.name)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        }
    }

    // Read back from cache to get the refreshed counts.
    let cached = db::folders::get_all_folders(&conn)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Build response from cached data (includes any stored counts).
    let folders: Vec<FolderEntry> = cached
        .into_iter()
        .map(|f| {
            let attributes: Vec<String> = if f.flags.is_empty() {
                vec![]
            } else {
                f.flags.split(',').map(|s| s.to_string()).collect()
            };
            FolderEntry {
                name: f.name,
                delimiter: f.delimiter,
                attributes,
                is_subscribed: f.is_subscribed,
                total_count: f.total_count,
                unread_count: f.unread_count,
            }
        })
        .collect();

    Ok(Json(FoldersResponse { folders }).into_response())
}
