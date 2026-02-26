use std::sync::Arc;

use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::search::engine::{SearchEngine, SearchQuery};

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/search`.
#[derive(Deserialize)]
pub struct SearchParams {
    /// The search text (required, must not be empty).
    pub q: Option<String>,
    /// Optional folder filter.
    pub folder: Option<String>,
    /// Optional from address filter.
    pub from: Option<String>,
    /// Optional to address filter.
    pub to: Option<String>,
    /// Optional date range start (Unix epoch seconds).
    pub date_from: Option<i64>,
    /// Optional date range end (Unix epoch seconds).
    pub date_to: Option<i64>,
    /// Optional attachment filter.
    pub has_attachment: Option<bool>,
    /// Maximum number of results (default 50).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination (default 0).
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    50
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response envelope for `GET /api/search`.
#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total_count: usize,
    pub query: String,
}

/// A single search result item enriched with message metadata from SQLite.
#[derive(Serialize)]
pub struct SearchResultItem {
    pub uid: u32,
    pub folder: String,
    pub score: f32,
    pub subject: String,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: String,
    pub date: String,
    pub flags: String,
    pub has_attachments: bool,
    pub snippet: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// `GET /api/search?q=text&folder=INBOX&from=alice&date_from=...&date_to=...&has_attachment=true&limit=50&offset=0`
///
/// Searches the user's Tantivy index and resolves matching UIDs from the
/// SQLite message cache to return enriched results.
pub async fn search_messages(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(search_engine): Extension<Arc<SearchEngine>>,
    Query(params): Query<SearchParams>,
) -> Result<Response, AppError> {
    // Validate that `q` is provided and non-empty.
    let query_text = params
        .q
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    if query_text.is_empty() {
        return Err(AppError::BadRequest(
            "Query parameter 'q' is required and must not be empty".to_string(),
        ));
    }

    // Open the user's search index.
    let user_index = search_engine
        .open_user_index(&session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Search engine error: {e}")))?;

    // Build SearchQuery from params.
    let search_query = SearchQuery {
        text: query_text.clone(),
        folder: params.folder,
        from: params.from,
        to: params.to,
        date_from: params.date_from,
        date_to: params.date_to,
        has_attachment: params.has_attachment,
        limit: params.limit,
        offset: params.offset,
    };

    // Execute search.
    let (search_results, total_count) = user_index
        .search(&search_query)
        .map_err(|e| AppError::InternalError(format!("Search error: {e}")))?;

    // Open the user's SQLite database to resolve message metadata.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Resolve each search result UID into a full SearchResultItem.
    let mut results = Vec::with_capacity(search_results.len());
    for sr in &search_results {
        let item = if let Ok(Some(msg)) =
            db::messages::get_single_message(&conn, &sr.folder, sr.uid)
        {
            SearchResultItem {
                uid: msg.uid,
                folder: msg.folder,
                score: sr.score,
                subject: msg.subject,
                from_address: msg.from_address,
                from_name: msg.from_name,
                to_addresses: msg.to_addresses,
                date: msg.date,
                flags: msg.flags,
                has_attachments: msg.has_attachments,
                snippet: if sr.snippet.is_empty() {
                    msg.snippet
                } else {
                    sr.snippet.clone()
                },
            }
        } else {
            // Message not in SQLite cache; return basic info from the index.
            SearchResultItem {
                uid: sr.uid,
                folder: sr.folder.clone(),
                score: sr.score,
                subject: String::new(),
                from_address: String::new(),
                from_name: String::new(),
                to_addresses: String::new(),
                date: String::new(),
                flags: String::new(),
                has_attachments: false,
                snippet: sr.snippet.clone(),
            }
        };
        results.push(item);
    }

    Ok(Json(SearchResponse {
        results,
        total_count,
        query: query_text,
    })
    .into_response())
}
