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

/// `GET /api/folders`
///
/// Lists all IMAP folders for the authenticated user, syncing the result
/// into the per-user SQLite cache.
pub async fn list_folders(
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

    // Fetch folders from IMAP server.
    let imap_folders = imap_client
        .list_folders(&creds)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

    // Open the per-user database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Upsert each folder into SQLite cache.
    for folder in &imap_folders {
        let flags_csv = folder.attributes.join(",");
        db::folders::upsert_folder(
            &conn,
            &folder.name,
            folder.delimiter.as_deref(),
            None,            // parent — not available from IMAP list
            &flags_csv,
            true,            // is_subscribed — assume all listed folders are subscribed
            0,               // total_count — not yet available from IMAP
            0,               // unread_count — not yet available from IMAP
            0,               // uid_validity — not yet available
            0,               // highest_modseq — not yet available
        )
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    }

    // Remove stale folders that no longer exist on the server.
    let current_names: Vec<String> = imap_folders.iter().map(|f| f.name.clone()).collect();
    db::folders::remove_stale_folders(&conn, &current_names)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Read back from cache to get any stored counts.
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
