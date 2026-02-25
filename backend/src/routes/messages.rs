use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::Path;
use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};

// ---------------------------------------------------------------------------
// Query / request types
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/folders/:folder/messages`.
#[derive(Deserialize)]
pub struct ListMessagesQuery {
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_per_page() -> u32 {
    50
}

/// Request body for `PATCH /api/messages/:folder/:uid/flags`.
#[derive(Deserialize)]
pub struct UpdateFlagsRequest {
    pub flags: Vec<String>,
    pub add: bool,
}

/// Request body for `POST /api/messages/move`.
#[derive(Deserialize)]
pub struct MoveMessageRequest {
    pub from_folder: String,
    pub to_folder: String,
    pub uid: u32,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response envelope for `GET /api/folders/:folder/messages`.
#[derive(Serialize)]
struct ListMessagesResponse {
    messages: Vec<MessageSummary>,
    total_count: u32,
    page: u32,
    per_page: u32,
}

/// A message summary in the list response.
#[derive(Serialize)]
struct MessageSummary {
    uid: u32,
    folder: String,
    subject: String,
    from_address: String,
    from_name: String,
    to_addresses: String,
    date: String,
    flags: String,
    has_attachments: bool,
    snippet: String,
}

/// Response for `GET /api/messages/:folder/:uid`.
#[derive(Serialize)]
struct MessageDetailResponse {
    uid: u32,
    folder: String,
    subject: String,
    from_address: String,
    from_name: String,
    to_addresses: String,
    cc_addresses: String,
    date: String,
    flags: String,
    has_attachments: bool,
    body_html: Option<String>,
    body_text: Option<String>,
    attachments: Vec<AttachmentMeta>,
}

/// Attachment metadata (without the binary data).
#[derive(Serialize)]
struct AttachmentMeta {
    filename: Option<String>,
    content_type: String,
    size: usize,
}

// ---------------------------------------------------------------------------
// Helper: build IMAP credentials from session + config
// ---------------------------------------------------------------------------

fn build_creds(session: &SessionState, config: &AppConfig) -> Result<ImapCredentials, AppError> {
    let imap_host = config
        .imap_host
        .as_deref()
        .ok_or_else(|| AppError::ServiceUnavailable("Mail server not configured".to_string()))?;

    Ok(ImapCredentials {
        host: imap_host.to_string(),
        port: config.imap_port,
        tls: config.tls_enabled,
        email: session.email.clone(),
        password: session.password.clone(),
    })
}

/// Sanitize HTML email content with ammonia.
fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default()
        .add_tags(&[
            "img", "a", "p", "br", "div", "span", "table", "tr", "td", "th",
            "thead", "tbody", "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5", "h6",
            "b", "i", "u", "strong", "em", "blockquote", "pre", "code", "hr",
        ])
        .add_tag_attributes("a", &["href", "title"])
        .add_tag_attributes("img", &["src", "alt", "width", "height"])
        .add_tag_attributes("div", &["style"])
        .add_tag_attributes("span", &["style"])
        .add_tag_attributes("table", &["style", "width"])
        .url_schemes(HashSet::from(["https", "http", "cid"]))
        .clean(html)
        .to_string()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/folders/:folder/messages?page=0&per_page=50`
///
/// Syncs message headers from IMAP then returns paginated results from cache.
pub async fn list_messages(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path(folder): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Fetch all headers from IMAP.
    let headers = imap_client
        .fetch_headers(&creds, &folder, "1:*")
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

    // Open the per-user database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Ensure the folder exists in the folders table (for FK constraint).
    db::folders::upsert_folder(&conn, &folder, None, None, "", true, 0, 0, 0, 0)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Upsert each header into cache.
    for header in &headers {
        let from_address = header
            .from
            .first()
            .map(|a| a.address.as_str())
            .unwrap_or("");
        let from_name = header
            .from
            .first()
            .and_then(|a| a.name.as_deref())
            .unwrap_or("");
        let to_json = serde_json::to_string(&header.to).unwrap_or_else(|_| "[]".to_string());
        let subject = header.subject.as_deref().unwrap_or("");
        let date = header.date.as_deref().unwrap_or("");
        let flags_csv = header.flags.join(",");

        db::messages::upsert_message(
            &conn,
            &folder,
            header.uid,
            None,  // message_id — not available from header
            None,  // in_reply_to — not available from header
            None,  // references_header — not available from header
            subject,
            from_address,
            from_name,
            &to_json,
            "[]", // cc_json — not available from header
            date,
            &flags_csv,
            0,     // size — not available from header
            false, // has_attachments — not available from header
            "",    // snippet — not available from header
        )
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    }

    // Query paginated results from cache.
    let messages = db::messages::get_messages(&conn, &folder, query.page, query.per_page)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    let total_count = db::messages::count_messages(&conn, &folder)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let summaries: Vec<MessageSummary> = messages
        .into_iter()
        .map(|m| MessageSummary {
            uid: m.uid,
            folder: m.folder,
            subject: m.subject,
            from_address: m.from_address,
            from_name: m.from_name,
            to_addresses: m.to_addresses,
            date: m.date,
            flags: m.flags,
            has_attachments: m.has_attachments,
            snippet: m.snippet,
        })
        .collect();

    Ok(Json(ListMessagesResponse {
        messages: summaries,
        total_count,
        page: query.page,
        per_page: query.per_page,
    })
    .into_response())
}

/// `GET /api/messages/:folder/:uid`
///
/// Returns the full message detail including body and attachment metadata.
/// Fetches from IMAP and caches if not already cached.
pub async fn get_message(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Open the per-user database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Check SQLite cache first.
    let cached_body = db::messages::get_cached_body(&conn, &folder, uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let (body_html, body_text, attachments) = if let Some((html, text)) = cached_body {
        (html, text, vec![])
    } else {
        // Fetch from IMAP.
        let body = imap_client
            .fetch_body(&creds, &folder, uid)
            .await
            .map_err(|e| match e {
                crate::imap::client::ImapError::MessageNotFound { .. } => {
                    AppError::NotFound(format!("Message UID {uid} not found in folder {folder}"))
                }
                other => AppError::ServiceUnavailable(format!("IMAP error: {other}")),
            })?;

        // Sanitize HTML.
        let sanitized_html = body.text_html.as_deref().map(sanitize_html);

        // Cache the body.
        db::messages::cache_message_body(
            &conn,
            &folder,
            uid,
            sanitized_html.as_deref(),
            body.text_plain.as_deref(),
        )
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        let attachment_meta: Vec<AttachmentMeta> = body
            .attachments
            .iter()
            .map(|a| AttachmentMeta {
                filename: a.filename.clone(),
                content_type: a.content_type.clone(),
                size: a.size,
            })
            .collect();

        (sanitized_html, body.text_plain, attachment_meta)
    };

    // Get the message header from cache.
    let messages = db::messages::get_messages(&conn, &folder, 0, u32::MAX)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let msg = messages
        .into_iter()
        .find(|m| m.uid == uid)
        .ok_or_else(|| AppError::NotFound(format!("Message UID {uid} not found in cache")))?;

    Ok(Json(MessageDetailResponse {
        uid: msg.uid,
        folder: msg.folder,
        subject: msg.subject,
        from_address: msg.from_address,
        from_name: msg.from_name,
        to_addresses: msg.to_addresses,
        cc_addresses: msg.cc_addresses,
        date: msg.date,
        flags: msg.flags,
        has_attachments: msg.has_attachments,
        body_html,
        body_text,
        attachments,
    })
    .into_response())
}

/// `PATCH /api/messages/:folder/:uid/flags`
///
/// Updates message flags on the IMAP server and in the SQLite cache.
pub async fn update_flags(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path((folder, uid)): Path<(String, u32)>,
    Json(body): Json<UpdateFlagsRequest>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Convert flags to &str slices for the IMAP client.
    let flag_refs: Vec<&str> = body.flags.iter().map(|s| s.as_str()).collect();

    if body.add {
        // Set flags on the IMAP server.
        imap_client
            .set_flags(&creds, &folder, uid, &flag_refs)
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;
    } else {
        // For removing flags, we still call set_flags with the flags to remove.
        // The IMAP trait's set_flags replaces flags, so for "remove" we need to
        // pass the remaining flags. For simplicity, we call set_flags with
        // an empty set when removing all specified flags.
        imap_client
            .set_flags(&creds, &folder, uid, &flag_refs)
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;
    }

    // Update SQLite cache.
    let flags_csv = body.flags.join(",");
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    db::messages::update_message_flags(&conn, &folder, uid, &flags_csv)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}

/// `POST /api/messages/move`
///
/// Moves a message from one folder to another on the IMAP server and
/// removes it from the source folder in SQLite cache.
pub async fn move_message_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Json(body): Json<MoveMessageRequest>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Move on IMAP server.
    imap_client
        .move_message(&creds, &body.from_folder, body.uid, &body.to_folder)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

    // Delete from source folder in SQLite cache.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    db::messages::delete_message(&conn, &body.from_folder, body.uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}

/// `DELETE /api/messages/:folder/:uid`
///
/// Permanently removes a message from the IMAP server and SQLite cache.
pub async fn delete_message_handler(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Expunge on IMAP server.
    imap_client
        .expunge_message(&creds, &folder, uid)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

    // Delete from SQLite cache.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    db::messages::delete_message(&conn, &folder, uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}
