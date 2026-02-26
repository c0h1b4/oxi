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
use crate::search::engine::{IndexableMessage, SearchEngine, UserIndex};

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
    size: u32,
    has_attachments: bool,
    snippet: String,
}

/// An email address entry for the detail response.
#[derive(Serialize, Deserialize, Clone)]
struct AddressEntry {
    name: Option<String>,
    address: String,
}

/// Parse a JSON-encoded address list string (e.g. from the SQLite cache) into
/// a `Vec<AddressEntry>`. Returns an empty vec on parse failure.
fn parse_address_list(json_str: &str) -> Vec<AddressEntry> {
    serde_json::from_str(json_str).unwrap_or_default()
}

/// Split a comma-separated flags string into a `Vec<String>`.
fn parse_flags(flags_csv: &str) -> Vec<String> {
    if flags_csv.is_empty() {
        return vec![];
    }
    flags_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Response for `GET /api/messages/:folder/:uid`.
#[derive(Serialize)]
struct MessageDetailResponse {
    uid: u32,
    folder: String,
    subject: String,
    from_address: String,
    from_name: String,
    to_addresses: Vec<AddressEntry>,
    cc_addresses: Vec<AddressEntry>,
    date: String,
    flags: Vec<String>,
    has_attachments: bool,
    html: Option<String>,
    text: Option<String>,
    raw_headers: String,
    attachments: Vec<AttachmentMeta>,
    thread: Vec<ThreadMessage>,
}

/// A message summary within a thread.
#[derive(Serialize)]
struct ThreadMessage {
    uid: u32,
    folder: String,
    message_id: Option<String>,
    in_reply_to: Option<String>,
    subject: String,
    from_address: String,
    from_name: String,
    to_addresses: String,
    cc_addresses: String,
    date: String,
    flags: String,
    size: u32,
    has_attachments: bool,
    snippet: String,
}

/// Attachment metadata (without the binary data).
#[derive(Serialize, Deserialize)]
struct AttachmentMeta {
    id: String,
    filename: Option<String>,
    content_type: String,
    size: usize,
    content_id: Option<String>,
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

/// How many seconds a folder's message cache is considered fresh.
const FOLDER_MESSAGES_TTL_SECS: u32 = 30;

/// `GET /api/folders/:folder/messages?page=0&per_page=50`
///
/// Returns paginated messages using a cache-first strategy:
/// 1. If the folder was synced within `FOLDER_MESSAGES_TTL_SECS`, serve from cache.
/// 2. Otherwise do a lightweight IMAP SELECT to check for new messages and sync
///    only what's new.
pub async fn list_messages(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Extension(search_engine): Extension<Arc<SearchEngine>>,
    Path(folder): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> Result<Response, AppError> {
    // Open the per-user database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // If this folder was synced recently, skip the IMAP round-trip.
    let folder_fresh = db::folders::is_folder_fresh(&conn, &folder, FOLDER_MESSAGES_TTL_SECS)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    if !folder_fresh {
        let creds = build_creds(&session, &config)?;

        // Check what we have in cache.
        let cached_folder = db::folders::get_folder(&conn, &folder)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        let cached_uid_validity = cached_folder.as_ref().map(|f| f.uid_validity).unwrap_or(0);

        // Ensure the folder exists in the folders table (for FK constraint).
        if cached_folder.is_none() {
            db::folders::upsert_folder(&conn, &folder, None, None, "", true, 0, 0, 0, 0)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        }

        let cached_count = db::messages::count_messages(&conn, &folder)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        // Do a lightweight IMAP SELECT to get folder status.
        let status = imap_client
            .folder_status(&creds, &folder)
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

        tracing::info!(
            folder = %folder,
            cached_uid_validity = cached_uid_validity,
            imap_uid_validity = status.uid_validity,
            cached_count = cached_count,
            imap_exists = status.exists,
            "list_messages: folder status check"
        );

        let needs_full_sync = cached_uid_validity != 0
            && cached_uid_validity != status.uid_validity;

        if needs_full_sync {
            tracing::info!(folder = %folder, "UIDVALIDITY changed, clearing cache");
            db::messages::delete_folder_messages(&conn, &folder)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        }

        let needs_sync = needs_full_sync || cached_count == 0 || status.exists != cached_count;

        if needs_sync {
            // Determine which UIDs to fetch.
            let uid_range = if needs_full_sync || cached_count == 0 {
                "1:*".to_string()
            } else {
                let max_cached_uid = db::messages::max_uid(&conn, &folder)
                    .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
                if max_cached_uid > 0 {
                    format!("{}:*", max_cached_uid + 1)
                } else {
                    "1:*".to_string()
                }
            };

            tracing::info!(folder = %folder, uid_range = %uid_range, "Syncing messages from IMAP");

            let headers = imap_client
                .fetch_headers(&creds, &folder, &uid_range)
                .await
                .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

            tracing::info!(folder = %folder, fetched = headers.len(), "Fetched headers from IMAP");

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
                let to_json =
                    serde_json::to_string(&header.to).unwrap_or_else(|_| "[]".to_string());
                let subject = header.subject.as_deref().unwrap_or("");
                let date = header.date.as_deref().unwrap_or("");
                let flags_csv = header.flags.join(",");

                db::messages::upsert_message(
                    &conn,
                    &folder,
                    header.uid,
                    None,
                    None,
                    None,
                    subject,
                    from_address,
                    from_name,
                    &to_json,
                    "[]",
                    date,
                    &flags_csv,
                    header.size,
                    header.has_attachments,
                    "",
                )
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
            }

            // Index newly synced messages into the search engine.
            // Skip indexing for Spam/Junk/Trash folders.
            if !headers.is_empty()
                && !UserIndex::is_excluded_folder(&folder)
                && let Ok(user_index) = search_engine.open_user_index(&session.user_hash)
            {
                let indexable: Vec<IndexableMessage> = headers
                    .iter()
                    .map(|h| {
                        let from_address = h
                            .from
                            .first()
                            .map(|a| a.address.as_str())
                            .unwrap_or("");
                        let from_name = h
                            .from
                            .first()
                            .and_then(|a| a.name.as_deref())
                            .unwrap_or("");
                        let subject = h.subject.as_deref().unwrap_or("");
                        let date = h.date.as_deref().unwrap_or("");
                        let to_json = serde_json::to_string(&h.to)
                            .unwrap_or_else(|_| "[]".to_string());
                        IndexableMessage {
                            uid: h.uid,
                            folder: folder.clone(),
                            subject: subject.to_string(),
                            from_address: from_address.to_string(),
                            from_name: from_name.to_string(),
                            to_addresses: to_json,
                            body_text: String::new(),
                            date_epoch: crate::db::messages::parse_date_to_epoch_public(date),
                            has_attachments: h.has_attachments,
                        }
                    })
                    .collect();
                let _ = user_index.index_messages_batch(&indexable);
            }

            // Update folder metadata with UIDVALIDITY and message count.
            db::folders::update_folder_status(&conn, &folder, status.uid_validity, status.exists)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

            // Refresh unread count from cached messages.
            db::folders::refresh_unread_count(&conn, &folder)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        } else {
            // No sync needed but still update the timestamp so TTL resets.
            db::folders::update_folder_status(&conn, &folder, status.uid_validity, status.exists)
                .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
        }
    }

    // Query paginated results from cache.
    let messages = db::messages::get_messages(&conn, &folder, query.page, query.per_page)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    let total_count = db::messages::count_messages(&conn, &folder)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    tracing::info!(
        folder = %folder,
        total_count = total_count,
        page_messages = messages.len(),
        page = query.page,
        per_page = query.per_page,
        "list_messages: returning results"
    );

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
            size: m.size,
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
    Extension(search_engine): Extension<Arc<SearchEngine>>,
    Path((folder, uid)): Path<(String, u32)>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Open the per-user database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Check SQLite cache first.
    let cached_body = db::messages::get_cached_body(&conn, &folder, uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Treat a cache hit with missing attachments_json as stale (pre-V006 cache).
    // Re-fetch from IMAP so attachments and inline images are properly resolved.
    let usable_cache = cached_body.filter(|c| c.attachments_json.is_some());

    let (body_html, body_text, attachments, raw_headers) = if let Some(cached) = usable_cache {
        let attachments: Vec<AttachmentMeta> = cached
            .attachments_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        (cached.html, cached.text, attachments, cached.raw_headers.unwrap_or_default())
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

        let attachment_meta: Vec<AttachmentMeta> = body
            .attachments
            .iter()
            .enumerate()
            .map(|(i, a)| AttachmentMeta {
                id: i.to_string(),
                filename: a.filename.clone(),
                content_type: a.content_type.clone(),
                size: a.size,
                content_id: a.content_id.clone(),
            })
            .collect();

        // Rewrite cid: URLs in the HTML to inline data URIs so the
        // sandboxed iframe can display embedded images without needing
        // network access.
        let resolved_html = sanitized_html.map(|mut html| {
            for att in &body.attachments {
                if let Some(ref cid) = att.content_id {
                    let cid_url = format!("cid:{cid}");
                    if html.contains(&cid_url) {
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&att.data);
                        let data_uri = format!("data:{};base64,{}", att.content_type, b64);
                        html = html.replace(&cid_url, &data_uri);
                    }
                }
            }
            html
        });

        // Serialize attachment metadata for caching.
        let att_json = serde_json::to_string(&attachment_meta).ok();

        // Cache the body along with attachments and raw headers.
        db::messages::cache_message_body(
            &conn,
            &folder,
            uid,
            resolved_html.as_deref(),
            body.text_plain.as_deref(),
            att_json.as_deref(),
            Some(&body.raw_headers),
        )
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        (resolved_html, body.text_plain, attachment_meta, body.raw_headers)
    };

    // Get the message header from cache (use efficient single-message lookup).
    let msg = db::messages::get_single_message(&conn, &folder, uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?
        .ok_or_else(|| AppError::NotFound(format!("Message UID {uid} not found in cache")))?;

    // Re-index message with full body text for search.
    // Skip indexing for Spam/Junk/Trash folders.
    if let Some(ref text) = body_text
        && !UserIndex::is_excluded_folder(&folder)
        && let Ok(user_index) = search_engine.open_user_index(&session.user_hash)
    {
        let indexable = IndexableMessage {
            uid: msg.uid,
            folder: msg.folder.clone(),
            subject: msg.subject.clone(),
            from_address: msg.from_address.clone(),
            from_name: msg.from_name.clone(),
            to_addresses: msg.to_addresses.clone(),
            body_text: text.clone(),
            date_epoch: crate::db::messages::parse_date_to_epoch_public(&msg.date),
            has_attachments: msg.has_attachments,
        };
        let _ = user_index.index_message(&indexable);
    }

    // Build thread using full References chain.
    let thread_messages = if let Some(ref message_id) = msg.message_id {
        db::messages::get_full_thread(&conn, message_id, msg.references_header.as_deref())
            .unwrap_or_default()
    } else {
        vec![]
    };

    let thread: Vec<ThreadMessage> = thread_messages
        .into_iter()
        .map(|m| ThreadMessage {
            uid: m.uid,
            folder: m.folder,
            message_id: m.message_id,
            in_reply_to: m.in_reply_to,
            subject: m.subject,
            from_address: m.from_address,
            from_name: m.from_name,
            to_addresses: m.to_addresses,
            cc_addresses: m.cc_addresses,
            date: m.date,
            flags: m.flags,
            size: m.size,
            has_attachments: m.has_attachments,
            snippet: m.snippet,
        })
        .collect();

    Ok(Json(MessageDetailResponse {
        uid: msg.uid,
        folder: msg.folder,
        subject: msg.subject,
        from_address: msg.from_address,
        from_name: msg.from_name,
        to_addresses: parse_address_list(&msg.to_addresses),
        cc_addresses: parse_address_list(&msg.cc_addresses),
        date: msg.date,
        flags: parse_flags(&msg.flags),
        has_attachments: msg.has_attachments,
        html: body_html,
        text: body_text,
        raw_headers,
        attachments,
        thread,
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
        imap_client
            .add_flags(&creds, &folder, uid, &flag_refs)
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;
    } else {
        imap_client
            .remove_flags(&creds, &folder, uid, &flag_refs)
            .await
            .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;
    }

    // Update SQLite cache: read current flags, add/remove, write back.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let current_flags_csv: String = conn
        .query_row(
            "SELECT flags FROM messages WHERE folder = ?1 AND uid = ?2",
            rusqlite::params![&folder, uid],
            |row| row.get(0),
        )
        .unwrap_or_default();

    let mut current_flags: Vec<String> = if current_flags_csv.is_empty() {
        vec![]
    } else {
        current_flags_csv.split(',').map(|s| s.to_string()).collect()
    };

    if body.add {
        for flag in &body.flags {
            if !current_flags.contains(flag) {
                current_flags.push(flag.clone());
            }
        }
    } else {
        current_flags.retain(|f| !body.flags.contains(f));
    }

    let new_flags_csv = current_flags.join(",");
    db::messages::update_message_flags(&conn, &folder, uid, &new_flags_csv)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Refresh unread count after flag change.
    db::folders::refresh_unread_count(&conn, &folder)
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

    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Check if the message was unread before removing it from the source cache,
    // so we can adjust the destination folder's unread count.
    let was_unread = db::messages::get_single_message(&conn, &body.from_folder, body.uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?
        .map(|m| !m.flags.contains("\\Seen"))
        .unwrap_or(false);

    // Delete from source folder cache. We don't keep the row in the destination
    // because the UID changes after an IMAP MOVE, and a stale UID would cause
    // 404s when trying to fetch the message body.
    db::messages::delete_message(&conn, &body.from_folder, body.uid)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Refresh source folder unread count (now accurate since the row is gone).
    db::folders::refresh_unread_count(&conn, &body.from_folder)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Bump destination folder unread count if the moved message was unread.
    if was_unread {
        db::folders::adjust_unread_count(&conn, &body.to_folder, 1)
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    }

    // Invalidate destination folder cache so the next list request forces an
    // IMAP resync and picks up the moved message with its new UID.
    db::folders::invalidate_folder_freshness(&conn, &body.to_folder)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}

/// `GET /api/messages/:folder/:uid/attachments/:attachment_id`
///
/// Downloads an attachment by its index from the message.
pub async fn download_attachment(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(imap_client): Extension<Arc<dyn ImapClient>>,
    Path((folder, uid, attachment_id)): Path<(String, u32, String)>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Parse the attachment index.
    let index: usize = attachment_id
        .parse()
        .map_err(|_| AppError::BadRequest(format!("Invalid attachment id: {attachment_id}")))?;

    // Fetch the full message body from IMAP.
    let body = imap_client
        .fetch_body(&creds, &folder, uid)
        .await
        .map_err(|e| match e {
            crate::imap::client::ImapError::MessageNotFound { .. } => {
                AppError::NotFound(format!("Message UID {uid} not found in folder {folder}"))
            }
            other => AppError::ServiceUnavailable(format!("IMAP error: {other}")),
        })?;

    // Find the attachment by index.
    let attachment = body
        .attachments
        .into_iter()
        .nth(index)
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Attachment {attachment_id} not found on message UID {uid}"
            ))
        })?;

    // Build the response with appropriate headers.
    let filename = attachment
        .filename
        .unwrap_or_else(|| format!("attachment_{index}"));
    let content_type = attachment.content_type;

<<<<<<< ours
    let is_inline = content_type.starts_with("image/") || content_type == "application/pdf";
    let disp_type = if is_inline { "inline" } else { "attachment" };
    let disposition = format!("{disp_type}; filename=\"{}\"", filename.replace('"', "\\\""));
=======
    // Use inline disposition for types the browser can display natively
    // (PDF, images) so the preview works; use attachment for everything else.
    let is_inline = content_type == "application/pdf"
        || content_type.starts_with("image/")
        || content_type.starts_with("text/");
    let disposition = if is_inline {
        format!("inline; filename=\"{}\"", filename.replace('"', "\\\""))
    } else {
        format!("attachment; filename=\"{}\"", filename.replace('"', "\\\""))
    };
>>>>>>> theirs

    Ok(Response::builder()
        .header("content-type", &content_type)
        .header("content-disposition", &disposition)
        .body(axum::body::Body::from(attachment.data))
        .unwrap())
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

    // Refresh unread count for folder.
    db::folders::refresh_unread_count(&conn, &folder)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}
