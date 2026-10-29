use std::sync::Arc;

use axum::extract::Path;
use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};

pub mod types;
use types::*;

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::db::header_sync::HeaderSync;
use crate::email_theme;
use crate::error::AppError;
use crate::imap::client::{ImapClient, ImapCredentials};
use crate::realtime::events::{EventBus, MailEvent};
use crate::search::engine::{IndexableMessage, SearchEngine, UserIndex};

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

// ---------------------------------------------------------------------------
// Helper: validate IMAP flags from client input
// ---------------------------------------------------------------------------

/// Returns `true` if `flag` is a syntactically valid IMAP flag.
///
/// Accepts RFC 3501 system flags (`\Seen`, `\Answered`, `\Flagged`,
/// `\Deleted`, `\Draft`, `\Recent`) and keyword atoms (`Junk`,
/// `$Forwarded`, `$MDNSent`, etc.).
///
/// This is a restricted safe subset of the RFC 3501 §9 atom grammar:
/// an optional leading `\` followed by `[A-Za-z0-9$_\-.+]`, capped at
/// 64 characters. The cap is well above any flag seen in practice and
/// keeps the set narrow enough to exclude whitespace, parentheses, and
/// control characters without needing to enumerate every RFC exclusion.
fn is_valid_flag(flag: &str) -> bool {
    if flag.is_empty() || flag.len() > 64 {
        return false;
    }
    // Strip optional leading backslash (system flags like \Seen).
    let atom = flag.strip_prefix('\\').unwrap_or(flag);
    !atom.is_empty()
        && atom
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'$' | b'_' | b'-' | b'+' | b'.'))
}

fn validate_flags(flags: &[String]) -> Result<(), AppError> {
    for flag in flags {
        if !is_valid_flag(flag) {
            return Err(AppError::BadRequest("invalid flag".to_string()));
        }
    }
    Ok(())
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
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(folder): Path<String>,
    Query(query): Query<ListMessagesQuery>,
) -> Result<Response, AppError> {
    let mut syncing = false;

    let database = config
        .user_db(&session.user_hash)
        .await
        .map_err(AppError::InternalError)?;
    let mut header_sync = HeaderSync::capture(&database)
        .await
        .map_err(AppError::InternalError)?;
    let folder_fresh = db::folders::is_folder_fresh(&database, &folder, FOLDER_MESSAGES_TTL_SECS)
        .await
        .map_err(AppError::InternalError)?;
    let cached = db::folders::get_folder(&database, &folder)
        .await
        .map_err(AppError::InternalError)?;
    let cached_uid_validity = cached.as_ref().map(|f| f.uid_validity).unwrap_or(0);
    if cached.is_none() {
        db::folders::upsert_folder(&database, &folder, None, None, "", true, 0, 0, 0, 0)
            .await
            .map_err(AppError::InternalError)?;
    }
    let cached_count = db::messages::count_messages(&database, &folder)
        .await
        .map_err(AppError::InternalError)?;
    let folder_invalidated = db::folders::is_folder_messages_invalidated(&database, &folder)
        .await
        .map_err(AppError::InternalError)?;

    if !folder_fresh {
        let creds = build_creds(&session, &config)?;

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

        let needs_full_sync =
            cached_uid_validity != 0 && cached_uid_validity != status.uid_validity;

        if needs_full_sync {
            tracing::info!(folder = %folder, "UIDVALIDITY changed, clearing cache");
            let transaction = header_sync
                .begin_write(&database)
                .await
                .map_err(AppError::InternalError)?;
            db::messages::delete_folder_messages(&transaction, &folder)
                .await
                .map_err(AppError::InternalError)?;
            db::header_sync::invalidate(&transaction)
                .await
                .map_err(AppError::InternalError)?;
            db::folders::update_folder_sync_status(
                &transaction,
                &folder,
                status.uid_validity,
                status.exists,
                0,
            )
            .await
            .map_err(AppError::InternalError)?;
            header_sync = HeaderSync::capture(&transaction)
                .await
                .map_err(AppError::InternalError)?;
            transaction
                .commit()
                .await
                .map_err(AppError::InternalError)?;
        }

        let needs_sync = needs_full_sync
            || cached_count == 0
            || status.exists != cached_count
            || folder_invalidated;

        if needs_sync {
            // Cold start: no cached messages at all — fetch the ~100 most recent
            // synchronously for a fast first paint, then background-sync the rest.
            if (needs_full_sync || cached_count == 0) && status.exists > 0 {
                let recent_start = status.uid_next.saturating_sub(100).max(1);
                let recent_range = format!("{recent_start}:*");

                tracing::info!(folder = %folder, uid_range = %recent_range, "Partial sync: fetching recent headers");

                let headers = imap_client
                    .fetch_headers(&creds, &folder, &recent_range)
                    .await
                    .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

                tracing::info!(folder = %folder, fetched = headers.len(), "Partial sync: fetched recent headers");

                let transaction = header_sync
                    .begin_write(&database)
                    .await
                    .map_err(AppError::InternalError)?;
                upsert_and_index_headers(
                    &transaction,
                    &folder,
                    &headers,
                    &search_engine,
                    &session.user_hash,
                )
                .await?;
                db::folders::update_folder_status(
                    &transaction,
                    &folder,
                    status.uid_validity,
                    status.exists,
                )
                .await
                .map_err(AppError::InternalError)?;
                db::folders::refresh_unread_count(&transaction, &folder)
                    .await
                    .map_err(AppError::InternalError)?;
                transaction
                    .commit()
                    .await
                    .map_err(AppError::InternalError)?;

                // Spawn background task to fetch the remaining older headers.
                if recent_start > 1 {
                    let remaining_range = (1, recent_start - 1);
                    tokio::spawn(sync_remaining_headers(BackgroundSyncParams {
                        creds,
                        folder: folder.clone(),
                        imap_client: imap_client.clone(),
                        config: config.clone(),
                        user_hash: session.user_hash.clone(),
                        search_engine: search_engine.clone(),
                        event_bus: event_bus.clone(),
                        uid_range: remaining_range,
                        header_sync,
                        uid_validity: status.uid_validity,
                        exists: status.exists,
                    }));
                    syncing = true;
                }
            } else {
                // An interrupted initial sync can leave gaps below the highest
                // cached UID. Reconcile all server UIDs, fetching only missing ones.
                let uid_range = (1, status.uid_next.saturating_sub(1));

                tokio::spawn(sync_remaining_headers(BackgroundSyncParams {
                    creds,
                    folder: folder.clone(),
                    imap_client: imap_client.clone(),
                    config: config.clone(),
                    user_hash: session.user_hash.clone(),
                    search_engine: search_engine.clone(),
                    event_bus: event_bus.clone(),
                    uid_range,
                    header_sync,
                    uid_validity: status.uid_validity,
                    exists: status.exists,
                }));
                syncing = true;
            }
        } else {
            let transaction = header_sync
                .begin_write(&database)
                .await
                .map_err(AppError::InternalError)?;
            db::folders::update_folder_status(
                &transaction,
                &folder,
                status.uid_validity,
                status.exists,
            )
            .await
            .map_err(AppError::InternalError)?;
            transaction
                .commit()
                .await
                .map_err(AppError::InternalError)?;
        }
    }

    let messages = db::messages::get_messages(&database, &folder, query.page, query.per_page)
        .await
        .map_err(AppError::InternalError)?;
    let total_count = db::messages::count_messages(&database, &folder)
        .await
        .map_err(AppError::InternalError)?;
    let refs: Vec<_> = messages
        .iter()
        .map(|m| (m.uid, m.folder.as_str()))
        .collect();
    let tags_map = db::tags::get_tags_for_messages(&database, &refs)
        .await
        .map_err(AppError::InternalError)?;

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
        .map(|m| {
            let msg_tags = tags_map
                .get(&(m.uid, m.folder.clone()))
                .cloned()
                .unwrap_or_default();
            MessageSummary {
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
                reaction: m.reaction,
                tags: msg_tags,
            }
        })
        .collect();

    Ok(Json(ListMessagesResponse {
        messages: summaries,
        total_count,
        page: query.page,
        per_page: query.per_page,
        syncing,
    })
    .into_response())
}

// ---------------------------------------------------------------------------
// Helpers for eager loading
// ---------------------------------------------------------------------------

/// Upsert fetched headers into the database and index them for search.
async fn upsert_and_index_headers(
    conn: &db::pool::UserDb,
    folder: &str,
    headers: &[crate::imap::types::ImapMessageHeader],
    search_engine: &Arc<SearchEngine>,
    user_hash: &str,
) -> Result<(), AppError> {
    // Caller holds a checked HeaderSync write transaction through indexing.
    for header in headers {
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
        let cc_json = serde_json::to_string(&header.cc).unwrap_or_else(|_| "[]".to_string());
        let subject = header.subject.as_deref().unwrap_or("");
        let date = header.date.as_deref().unwrap_or("");
        let flags_csv = header.flags.join(",");

        db::messages::upsert_message(
            conn,
            folder,
            header.uid,
            header.message_id.as_deref(),
            header.in_reply_to.as_deref(),
            header.references.as_deref(),
            subject,
            from_address,
            from_name,
            &to_json,
            &cc_json,
            date,
            &flags_csv,
            header.size,
            header.has_attachments,
            "",
            header.reaction.as_deref(),
        )
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        // Populate denormalized known_addresses table.
        db::contacts::populate_known_addresses(conn, from_address, from_name, &to_json, &cc_json)
            .await
            .map_err(|e| AppError::InternalError(format!("Known addresses error: {e}")))?;
    }

    // Index for search (skip Spam/Junk/Trash).
    if !headers.is_empty() && !UserIndex::is_excluded_folder(folder) {
        let indexable: Vec<IndexableMessage> = headers
            .iter()
            .map(|h| {
                let from_address = h.from.first().map(|a| a.address.as_str()).unwrap_or("");
                let from_name = h.from.first().and_then(|a| a.name.as_deref()).unwrap_or("");
                let subject = h.subject.as_deref().unwrap_or("");
                let date = h.date.as_deref().unwrap_or("");
                let to_json = serde_json::to_string(&h.to).unwrap_or_else(|_| "[]".to_string());
                IndexableMessage {
                    uid: h.uid,
                    folder: folder.to_string(),
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
        let engine = search_engine.clone();
        let user_hash = user_hash.to_owned();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(user_index) = engine.open_user_index(&user_hash) {
                let _ = user_index.index_messages_batch(&indexable);
            }
        })
        .await;
    }

    Ok(())
}

/// Bound each FETCH by existing messages, not by the size of gaps between UIDs.
fn header_chunks(uids: &[u32]) -> impl Iterator<Item = String> + '_ {
    uids.chunks(250).map(|chunk| {
        chunk
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    })
}

/// Parameters for a background sync task, bundled to avoid too-many-arguments.
struct BackgroundSyncParams {
    creds: ImapCredentials,
    folder: String,
    imap_client: Arc<dyn ImapClient>,
    config: Arc<AppConfig>,
    user_hash: String,
    search_engine: Arc<SearchEngine>,
    event_bus: Arc<EventBus>,
    uid_range: (u32, u32),
    header_sync: HeaderSync,
    uid_validity: u32,
    exists: u32,
}

/// Background task: fetch remaining headers for a folder and notify the
/// frontend via EventBus when done. All errors are logged but non-fatal.
async fn sync_remaining_headers(params: BackgroundSyncParams) {
    let BackgroundSyncParams {
        creds,
        folder,
        imap_client,
        config,
        user_hash,
        search_engine,
        event_bus,
        uid_range,
        header_sync,
        uid_validity,
        exists,
    } = params;
    let result: Result<(), String> = crate::imap::pool::background(async {
        let database = config.user_db(&user_hash).await?;
        let mut uids = if uid_range.0 <= uid_range.1 {
            imap_client
                .fetch_uids_and_flags(&creds, &folder)
                .await
                .map_err(|e| format!("IMAP error: {e}"))?
                .into_iter()
                .map(|(uid, _)| uid)
                .filter(|uid| (uid_range.0..=uid_range.1).contains(uid))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let cached_uids = db::messages::get_all_uids_and_flags(&database, &folder)
            .await?
            .into_iter()
            .map(|(uid, _)| uid)
            .collect::<std::collections::HashSet<_>>();
        uids.retain(|uid| !cached_uids.contains(uid));
        uids.sort_unstable();
        uids.dedup();
        for uid_set in header_chunks(&uids) {
            let headers = imap_client
                .fetch_headers(&creds, &folder, &uid_set)
                .await
                .map_err(|e| format!("IMAP error: {e}"))?;
            let transaction = header_sync.begin_write(&database).await?;
            upsert_and_index_headers(&transaction, &folder, &headers, &search_engine, &user_hash)
                .await
                .map_err(|e| e.to_string())?;
            transaction.commit().await?;
        }
        let transaction = header_sync.begin_write(&database).await?;
        db::folders::update_folder_status(&transaction, &folder, uid_validity, exists).await?;
        db::folders::refresh_unread_count(&transaction, &folder).await?;
        transaction.commit().await
    })
    .await;

    match result {
        Ok(()) => {
            tracing::info!(folder = %folder, "Background sync: complete");
            event_bus
                .publish(&user_hash, MailEvent::FolderUpdated)
                .await;
        }
        Err(e) => {
            tracing::warn!(folder = %folder, error = %e, "Background sync: failed (will retry on next request)");
            let superseded: Result<bool, String> = async {
                let database = config.user_db(&user_hash).await?;
                let superseded = HeaderSync::capture(&database).await? != header_sync;
                db::folders::invalidate_folder_freshness(&database, &folder).await?;
                Ok(superseded)
            }
            .await;
            // A completed mutation may invalidate a backfill in another folder.
            // Wake the client to retry missing UIDs with fresh IMAP reads, not
            // the rejected headers. Network errors wait for the next request.
            if matches!(superseded, Ok(true)) {
                event_bus
                    .publish(&user_hash, MailEvent::FolderUpdated)
                    .await;
            }
        }
    }
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
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Check SQLite cache first.
    let cached_body = db::messages::get_cached_body(&conn, &folder, uid)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Treat a cache hit with missing attachments_json as stale (pre-V006 cache).
    // Re-fetch from IMAP so attachments and inline images are properly resolved.
    let usable_cache = cached_body.filter(|c| c.attachments_json.is_some());
    let fetched_body = usable_cache.is_none();

    let (body_html, body_text, attachments, raw_headers, email_theme) = if let Some(cached) =
        usable_cache
    {
        let attachments: Vec<AttachmentMeta> = cached
            .attachments_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        let theme = cached.email_theme;
        if theme.is_none()
            && let Some(ref html) = cached.html
        {
            let detected = email_theme::detect_email_theme(html).map(|t| t.as_i32());
            if let Some(t) = detected {
                let _ = db::messages::update_email_theme(&conn, &folder, uid, t).await;
            }
            (
                cached.html,
                cached.text,
                attachments,
                cached.raw_headers.unwrap_or_default(),
                detected,
            )
        } else {
            (
                cached.html,
                cached.text,
                attachments,
                cached.raw_headers.unwrap_or_default(),
                theme,
            )
        }
    } else {
        // Fetch from IMAP.
        let body = match imap_client.fetch_body(&creds, &folder, uid).await {
            Ok(body) => body,
            Err(error) => {
                // A missing or unreachable message cannot be served without a cached body.
                let _ = db::messages::delete_message(&conn, &folder, uid).await;
                let _ = db::folders::invalidate_folder_freshness(&conn, &folder).await;
                let engine = search_engine.clone();
                let user_hash = session.user_hash.clone();
                let index_folder = folder.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(user_index) = engine.open_user_index(&user_hash) {
                        let _ = user_index.delete_message(uid, &index_folder);
                    }
                })
                .await;
                return Err(match error {
                    crate::imap::client::ImapError::MessageNotFound { .. } => AppError::NotFound(
                        format!("Message UID {uid} not found in folder {folder}"),
                    ),
                    other => AppError::ServiceUnavailable(format!("IMAP error: {other}")),
                });
            }
        };

        // Use HTML directly (frontend sandbox handles security).
        let sanitized_html = body.text_html.clone();

        let attachment_meta: Vec<AttachmentMeta> = body
            .attachments
            .iter()
            .enumerate()
            .map(|(i, a)| AttachmentMeta {
                id: i.to_string(),
                filename: a.filename.clone(),
                content_type: preview_content_type(&a.content_type, a.filename.as_deref(), &a.data),
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

        let detected_theme = resolved_html
            .as_ref()
            .and_then(|h| email_theme::detect_email_theme(h))
            .map(|t| t.as_i32());

        db::messages::cache_message_body(
            &conn,
            &folder,
            uid,
            resolved_html.as_deref(),
            body.text_plain.as_deref(),
            att_json.as_deref(),
            Some(&body.raw_headers),
            detected_theme,
        )
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

        (
            resolved_html,
            body.text_plain,
            attachment_meta,
            body.raw_headers,
            detected_theme,
        )
    };

    // Get the message header from cache (use efficient single-message lookup).
    // If the header hasn't been synced yet (e.g. DB was cleared and sync is
    // still running), fall back to parsing the raw headers we already fetched.
    let msg = db::messages::get_single_message(&conn, &folder, uid)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let msg = match msg {
        Some(m) => m,
        None => {
            // Parse header fields from raw_headers so we can still return a
            // useful response even when the message list hasn't been synced.
            let parsed = mail_parser::MessageParser::default().parse(raw_headers.as_bytes());
            let subject = parsed
                .as_ref()
                .and_then(|p| p.subject().map(|s| s.to_string()))
                .unwrap_or_default();
            let (from_address, from_name) = parsed
                .as_ref()
                .and_then(|p| p.from())
                .and_then(|addr| match addr {
                    mail_parser::Address::List(addrs) => addrs.first().map(|a| {
                        (
                            a.address
                                .as_ref()
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                            a.name.as_ref().map(|s| s.to_string()).unwrap_or_default(),
                        )
                    }),
                    _ => None,
                })
                .unwrap_or_default();
            let date = parsed
                .as_ref()
                .and_then(|p| p.date())
                .map(|d| {
                    let sign = if d.tz_before_gmt { '-' } else { '+' };
                    format!(
                        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
                        d.year,
                        d.month,
                        d.day,
                        d.hour,
                        d.minute,
                        d.second,
                        sign,
                        d.tz_hour,
                        d.tz_minute
                    )
                })
                .unwrap_or_default();

            db::messages::CachedMessage {
                uid,
                folder: folder.clone(),
                message_id: parsed
                    .as_ref()
                    .and_then(|p| p.message_id().map(|s| format!("<{s}>"))),
                in_reply_to: parsed
                    .as_ref()
                    .and_then(|p| p.in_reply_to().as_text().map(|s| format!("<{s}>"))),
                references_header: parsed.as_ref().and_then(|p| {
                    let val = p.references();
                    val.as_text_list()
                        .map(|list| {
                            list.iter()
                                .map(|s| format!("<{s}>"))
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .or_else(|| val.as_text().map(|s| format!("<{s}>")))
                }),
                subject,
                from_address,
                from_name,
                to_addresses: String::from("[]"),
                cc_addresses: String::from("[]"),
                date,
                flags: String::new(),
                size: 0,
                has_attachments: !attachments.is_empty(),
                snippet: String::new(),
                reaction: None,
            }
        }
    };

    // Re-index message with full body text for search.
    // Skip indexing for Spam/Junk/Trash folders.
    if fetched_body
        && let Some(ref text) = body_text
        && !UserIndex::is_excluded_folder(&folder)
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
        let search_engine = Arc::clone(&search_engine);
        let user_hash = session.user_hash.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(user_index) = search_engine.open_user_index(&user_hash) {
                let _ = user_index.index_message(&indexable);
            }
        });
    }

    // Build thread using full References chain.
    let thread_messages = if let Some(ref message_id) = msg.message_id {
        db::messages::get_full_thread(&conn, message_id, msg.references_header.as_deref())
            .await
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
        email_theme: email_theme.map(|t| match t {
            0 => EmailTheme::Light,
            1 => EmailTheme::Dark,
            2 => EmailTheme::Transparent,
            3 => EmailTheme::Adaptive,
            _ => EmailTheme::Light,
        }),
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
    validate_flags(&body.flags)?;

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
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    let current_flags_csv = db::messages::get_single_message(&conn, &folder, uid)
        .await
        .map_err(AppError::InternalError)?
        .map(|message| message.flags)
        .unwrap_or_default();

    let mut current_flags: Vec<String> = if current_flags_csv.is_empty() {
        vec![]
    } else {
        current_flags_csv
            .split(',')
            .map(|s| s.to_string())
            .collect()
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
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Refresh unread count after flag change.
    db::folders::refresh_unread_count(&conn, &folder)
        .await
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
    Extension(search_engine): Extension<Arc<SearchEngine>>,
    Json(body): Json<MoveMessageRequest>,
) -> Result<Response, AppError> {
    let creds = build_creds(&session, &config)?;

    // Move on IMAP server.
    imap_client
        .move_message(&creds, &body.from_folder, body.uid, &body.to_folder)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("IMAP error: {e}")))?;

    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Check if the message was unread before removing it from the source cache,
    // so we can adjust the destination folder's unread count.
    let was_unread = db::messages::get_single_message(&conn, &body.from_folder, body.uid)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?
        .map(|m| !m.flags.contains("\\Seen"))
        .unwrap_or(false);

    // Delete from source folder cache. We don't keep the row in the destination
    // because the UID changes after an IMAP MOVE, and a stale UID would cause
    // 404s when trying to fetch the message body.
    db::messages::remove_after_mutation(&conn, &body.from_folder, body.uid)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Remove the old folder/UID pair from search too. Destination UIDs are
    // discovered during folder sync; Spam/Junk intentionally remain unindexed.
    let user_hash = session.user_hash.clone();
    let from_folder = body.from_folder.clone();
    let uid = body.uid;
    let search_result = tokio::task::spawn_blocking(move || {
        let index = search_engine.open_user_index(&user_hash)?;
        index.delete_message(uid, &from_folder)
    })
    .await;
    if !matches!(search_result, Ok(Ok(()))) {
        // The IMAP move already succeeded: do not trigger a false client rollback.
        tracing::warn!(
            ?search_result,
            "Failed to remove moved message from search index"
        );
    }

    // Refresh source folder unread count (now accurate since the row is gone).
    db::folders::refresh_unread_count(&conn, &body.from_folder)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Bump destination folder unread count if the moved message was unread.
    if was_unread {
        db::folders::adjust_unread_count(&conn, &body.to_folder, 1)
            .await
            .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    }

    // Invalidate destination folder cache so the next list request forces an
    // IMAP resync and picks up the moved message with its new UID.
    db::folders::invalidate_folder_freshness(&conn, &body.to_folder)
        .await
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
    let attachment = body.attachments.into_iter().nth(index).ok_or_else(|| {
        AppError::NotFound(format!(
            "Attachment {attachment_id} not found on message UID {uid}"
        ))
    })?;

    // Build the response with appropriate headers.
    let raw_filename = attachment
        .filename
        .unwrap_or_else(|| format!("attachment_{index}"));
    // Strip ASCII control characters (U+0000-U+001F and U+007F). MIME
    // decoders can surface them via quoted-pair escapes in malformed emails,
    // and HeaderValue rejects those bytes. HeaderValue::from_str routes
    // through is_valid (accepts 0x09, 0x20-0x7E, 0x80-0xFF; rejects
    // 0x00-0x08, 0x0A-0x1F, 0x7F), so non-ASCII UTF-8 bytes are fine.
    let filename: String = raw_filename
        .chars()
        .filter(|c| !c.is_ascii_control())
        .collect();
    // Fall back to a safe default if stripping left nothing (e.g. the
    // original filename was composed entirely of control characters).
    let filename = if filename.is_empty() {
        format!("attachment_{index}")
    } else {
        filename
    };
    let content_type =
        preview_content_type(&attachment.content_type, Some(&filename), &attachment.data);

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

    Response::builder()
        .header("content-type", &content_type)
        .header("content-disposition", &disposition)
        .body(axum::body::Body::from(attachment.data))
        .map_err(|e| AppError::InternalError(format!("Failed to build response: {e}")))
}

fn preview_content_type(content_type: &str, filename: Option<&str>, data: &[u8]) -> String {
    let media_type = content_type.split(';').next().unwrap_or("").trim();
    if media_type.eq_ignore_ascii_case("application/pdf")
        || (media_type.eq_ignore_ascii_case("application/octet-stream")
            && filename.is_some_and(|name| name.to_ascii_lowercase().ends_with(".pdf"))
            && data.starts_with(b"%PDF-"))
    {
        "application/pdf".to_string()
    } else {
        content_type.to_string()
    }
}

#[cfg(test)]
mod preview_tests {
    use super::{is_valid_flag, preview_content_type, validate_flags};

    #[test]
    fn accepts_common_system_flags_and_keywords() {
        for flag in [
            "\\Seen",
            "\\Answered",
            "\\Flagged",
            "\\Deleted",
            "\\Draft",
            "Junk",
            "$Forwarded",
            "$MDNSent",
            "custom_tag-1.0+",
        ] {
            assert!(is_valid_flag(flag), "rejected {flag:?}");
        }
        assert!(validate_flags(&[]).is_ok());
        assert!(is_valid_flag(&"a".repeat(64)));
    }

    #[test]
    fn rejects_unsafe_flags_and_enforces_length_limit() {
        for flag in [
            "",
            "\\",
            "two flags",
            "tag\r\nSTORE",
            "tag\0",
            "tag\t",
            "(tag)",
            "tag{1}",
            "tag\\Seen",
            "étiquette",
        ] {
            assert!(!is_valid_flag(flag), "accepted {flag:?}");
        }
        assert!(!is_valid_flag(&"a".repeat(65)));
        assert!(validate_flags(&["\\Seen".to_string(), "two flags".to_string()]).is_err());
    }

    #[test]
    fn normalizes_pdf_media_types() {
        assert_eq!(
            preview_content_type("Application/PDF; name=test.pdf", None, b""),
            "application/pdf"
        );
        assert_eq!(
            preview_content_type("application/octet-stream", Some("TEST.PDF"), b"%PDF-1.7"),
            "application/pdf"
        );
    }

    #[test]
    fn does_not_trust_filename_alone() {
        assert_eq!(
            preview_content_type("application/octet-stream", Some("test.pdf"), b"not a pdf"),
            "application/octet-stream"
        );
    }
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
    let conn = config
        .user_db(&session.user_hash)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;
    db::messages::remove_after_mutation(&conn, &folder, uid)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Refresh unread count for folder.
    db::folders::refresh_unread_count(&conn, &folder)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    Ok(Json(serde_json::json!({ "status": "ok" })).into_response())
}

#[cfg(test)]
mod sync_tests {
    use super::*;
    use crate::imap::client::mock::MockImapClient;

    fn sync_header(uid: u32) -> crate::imap::types::ImapMessageHeader {
        crate::imap::types::ImapMessageHeader {
            uid,
            subject: Some(format!("Message {uid}")),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            date: None,
            flags: Vec::new(),
            has_attachments: false,
            size: 42,
            message_id: None,
            in_reply_to: None,
            references: None,
            reaction: None,
        }
    }

    #[tokio::test]
    async fn fetched_background_headers_cannot_resurrect_completed_move() {
        fetched_headers_after_removal(true).await;
    }

    #[tokio::test]
    async fn fetched_background_headers_cannot_resurrect_completed_delete() {
        fetched_headers_after_removal(false).await;
    }

    async fn fetched_headers_after_removal(move_message: bool) {
        let data_dir = tempfile::tempdir().unwrap();
        let path = data_dir.path().to_str().unwrap();
        let user_hash = "header-race";
        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        for folder in ["INBOX", "Archive"] {
            db::folders::upsert_folder(&conn, folder, None, None, "", true, 1, 1, 1, 0)
                .await
                .unwrap();
        }
        let search_engine = Arc::new(SearchEngine::new(data_dir.path().to_path_buf()));
        let header_sync = HeaderSync::capture(&conn).await.unwrap();
        let transaction = header_sync.begin_write(&conn).await.unwrap();
        upsert_and_index_headers(
            &transaction,
            "INBOX",
            &[sync_header(7)],
            &search_engine,
            user_hash,
        )
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        drop(conn);

        let creds = ImapCredentials {
            host: "localhost".into(),
            port: 143,
            tls: false,
            email: "test@example.com".into(),
            password: "test".into(),
        };
        let config = Arc::new(AppConfig {
            imap_host: Some(creds.host.clone()),
            data_dir: path.to_owned(),
            ..serde_json::from_str::<AppConfig>("{}").unwrap()
        });
        let session = SessionState {
            account_id: "test".into(),
            email: creds.email.clone(),
            password: creds.password.clone(),
            user_hash: user_hash.into(),
            imap_host: creds.host.clone(),
            imap_port: creds.port,
            imap_tls: false,
            smtp_host: "localhost".into(),
            smtp_port: 587,
            smtp_tls: false,
            last_accessed: std::time::Instant::now(),
            timeout_override: None,
        };
        let mut client = MockImapClient::new().with_headers(vec![sync_header(7), sync_header(8)]);
        client.header_fetch_gate = Some((tokio::sync::Notify::new(), tokio::sync::Notify::new()));
        let client = Arc::new(client);
        let gate = client.header_fetch_gate.as_ref().unwrap();
        let event_bus = Arc::new(EventBus::new());
        let mut events = event_bus.subscribe(user_hash).await;
        let sync = tokio::spawn(sync_remaining_headers(BackgroundSyncParams {
            creds,
            folder: "INBOX".into(),
            imap_client: client.clone(),
            config: config.clone(),
            user_hash: user_hash.into(),
            search_engine: search_engine.clone(),
            event_bus,
            uid_range: (1, 100),
            header_sync,
            uid_validity: 1,
            exists: 1,
        }));

        // The IMAP batch has already been snapshotted. Complete the real handler
        // before allowing the background writer to see those stale headers.
        gate.0.notified().await;
        let imap_client: Arc<dyn ImapClient> = client.clone();
        let response = if move_message {
            move_message_handler(
                Extension(session),
                Extension(config),
                Extension(imap_client),
                Extension(search_engine.clone()),
                Json(MoveMessageRequest {
                    uid: 7,
                    from_folder: "INBOX".into(),
                    to_folder: "Archive".into(),
                }),
            )
            .await
            .unwrap()
        } else {
            delete_message_handler(
                Extension(session),
                Extension(config),
                Extension(imap_client),
                Path(("INBOX".into(), 7)),
            )
            .await
            .unwrap()
        };
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        gate.1.notify_one();
        sync.await.unwrap();

        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 7)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db::folders::is_folder_messages_invalidated(&conn, "INBOX")
                .await
                .unwrap()
        );
        assert!(
            matches!(events.try_recv().unwrap(), MailEvent::FolderUpdated),
            "superseded sync must request fresh missing-UID reconciliation"
        );
        assert!(events.try_recv().is_err());
        // A move's index deletion also must not be undone by rejected headers.
        if move_message {
            let index = search_engine.open_user_index(user_hash).unwrap();
            assert!(
                index
                    .search(&crate::search::engine::SearchQuery {
                        text: "Message".into(),
                        limit: 10,
                        ..Default::default()
                    })
                    .unwrap()
                    .0
                    .is_empty()
            );
        }

        // New destination UIDs (even the same numeric UID in another folder)
        // remain valid, as do future arrivals in the source folder.
        let next_sync = HeaderSync::capture(&conn).await.unwrap();
        let transaction = next_sync.begin_write(&conn).await.unwrap();
        upsert_and_index_headers(
            &transaction,
            "Archive",
            &[sync_header(7), sync_header(42)],
            &search_engine,
            user_hash,
        )
        .await
        .unwrap();
        upsert_and_index_headers(
            &transaction,
            "INBOX",
            &[sync_header(8)],
            &search_engine,
            user_hash,
        )
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        assert_eq!(
            db::messages::count_messages(&conn, "Archive")
                .await
                .unwrap(),
            2
        );
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 8)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn interrupted_initial_backfill_recovers_after_same_folder_delete() {
        interrupted_backfill_recovers(false).await;
    }

    #[tokio::test]
    async fn interrupted_initial_backfill_recovers_after_unrelated_folder_delete() {
        interrupted_backfill_recovers(true).await;
    }

    async fn interrupted_backfill_recovers(other_folder: bool) {
        let data_dir = tempfile::tempdir().unwrap();
        let path = data_dir.path().to_str().unwrap();
        let user_hash = "backfill-recovery";
        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        for folder in ["INBOX", "Archive"] {
            db::folders::upsert_folder(&conn, folder, None, None, "", true, 1000, 0, 1, 0)
                .await
                .unwrap();
        }
        let engine = Arc::new(SearchEngine::new(data_dir.path().to_path_buf()));
        let header_sync = HeaderSync::capture(&conn).await.unwrap();
        let transaction = header_sync.begin_write(&conn).await.unwrap();
        upsert_and_index_headers(
            &transaction,
            "INBOX",
            &(901..=1000).map(sync_header).collect::<Vec<_>>(),
            &engine,
            user_hash,
        )
        .await
        .unwrap();
        upsert_and_index_headers(
            &transaction,
            "Archive",
            &[sync_header(950)],
            &engine,
            user_hash,
        )
        .await
        .unwrap();
        db::folders::update_folder_status(&transaction, "INBOX", 1, 1000)
            .await
            .unwrap();
        transaction.commit().await.unwrap();
        drop(conn);
        let creds = ImapCredentials {
            host: "localhost".into(),
            port: 143,
            tls: false,
            email: "test@example.com".into(),
            password: "test".into(),
        };
        let config = Arc::new(AppConfig {
            imap_host: Some(creds.host.clone()),
            data_dir: path.to_owned(),
            ..serde_json::from_str::<AppConfig>("{}").unwrap()
        });
        let session = SessionState {
            account_id: "test".into(),
            email: creds.email.clone(),
            password: creds.password.clone(),
            user_hash: user_hash.into(),
            imap_host: creds.host.clone(),
            imap_port: 143,
            imap_tls: false,
            smtp_host: "localhost".into(),
            smtp_port: 587,
            smtp_tls: false,
            last_accessed: std::time::Instant::now(),
            timeout_override: None,
        };
        let mut client = MockImapClient::new().with_headers((1..=900).map(sync_header).collect());
        client.header_fetch_gate = Some((tokio::sync::Notify::new(), tokio::sync::Notify::new()));
        let client = Arc::new(client);
        let gate = client.header_fetch_gate.as_ref().unwrap();
        let bus = Arc::new(EventBus::new());
        let mut events = bus.subscribe(user_hash).await;
        let sync = tokio::spawn(sync_remaining_headers(BackgroundSyncParams {
            creds,
            folder: "INBOX".into(),
            imap_client: client.clone(),
            config: config.clone(),
            user_hash: user_hash.into(),
            search_engine: engine.clone(),
            event_bus: bus.clone(),
            uid_range: (1, 900),
            header_sync,
            uid_validity: 1,
            exists: 1000,
        }));
        gate.0.notified().await;
        let folder = if other_folder { "Archive" } else { "INBOX" };
        delete_message_handler(
            Extension(session.clone()),
            Extension(config.clone()),
            Extension(client.clone() as Arc<dyn ImapClient>),
            Path((folder.into(), 950)),
        )
        .await
        .unwrap();
        gate.1.notify_one();
        sync.await.unwrap();
        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        assert!(
            db::folders::is_folder_messages_invalidated(&conn, "INBOX")
                .await
                .unwrap()
        );
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 1)
                .await
                .unwrap()
                .is_none()
        );
        drop(conn);
        assert!(matches!(
            events.recv().await.unwrap(),
            MailEvent::FolderUpdated
        ));

        // The next actual list request must recover UIDs below max_cached_uid.
        // A fresh server snapshot omits only the UID deleted in this folder.
        let remaining = (1..=1000)
            .filter(|uid| other_folder || *uid != 950)
            .map(sync_header)
            .collect::<Vec<_>>();
        let fresh_client = Arc::new(MockImapClient::new().with_headers(remaining));
        let response = list_messages(
            Extension(session),
            Extension(config),
            Extension(fresh_client.clone() as Arc<dyn ImapClient>),
            Extension(engine.clone()),
            Extension(bus),
            Path("INBOX".into()),
            Query(ListMessagesQuery {
                page: 0,
                per_page: 50,
            }),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        events.recv().await.unwrap();
        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        assert_eq!(
            db::messages::count_messages(&conn, "INBOX").await.unwrap(),
            if other_folder { 1000 } else { 999 }
        );
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 1)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 900)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            db::messages::get_single_message(&conn, folder, 950)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db::folders::is_folder_fresh(&conn, "INBOX", 30)
                .await
                .unwrap()
        );
        let requests = fresh_client.header_requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|(_, uids)| {
            uids.split(',')
                .all(|uid| uid.parse::<u32>().unwrap() <= 900)
        }));
        let results = engine
            .open_user_index(user_hash)
            .unwrap()
            .search(&crate::search::engine::SearchQuery {
                text: "Message".into(),
                folder: Some("INBOX".into()),
                limit: 1100,
                ..Default::default()
            })
            .unwrap()
            .0;
        assert!(results.iter().any(|message| message.uid == 1));
    }

    #[tokio::test]
    async fn uidvalidity_reset_rejects_old_batch_but_allows_reused_uid() {
        let data_dir = tempfile::tempdir().unwrap();
        let conn = db::pool::open_test_db().await;
        db::folders::upsert_folder(&conn, "INBOX", None, None, "", true, 1, 1, 1, 0)
            .await
            .unwrap();
        let engine = Arc::new(SearchEngine::new(data_dir.path().to_path_buf()));
        let stale = HeaderSync::capture(&conn).await.unwrap();
        db::messages::remove_after_mutation(&conn, "INBOX", 7)
            .await
            .unwrap();
        let before_reset = HeaderSync::capture(&conn).await.unwrap();
        let transaction = before_reset.begin_write(&conn).await.unwrap();
        db::messages::delete_folder_messages(&transaction, "INBOX")
            .await
            .unwrap();
        db::header_sync::invalidate(&transaction).await.unwrap();
        db::folders::update_folder_sync_status(&transaction, "INBOX", 2, 1, 0)
            .await
            .unwrap();
        let new_namespace = HeaderSync::capture(&transaction).await.unwrap();
        transaction.commit().await.unwrap();
        assert!(stale.begin_write(&conn).await.is_err());
        assert!(before_reset.begin_write(&conn).await.is_err());
        let transaction = new_namespace.begin_write(&conn).await.unwrap();
        upsert_and_index_headers(&transaction, "INBOX", &[sync_header(7)], &engine, "test")
            .await
            .unwrap();
        transaction.commit().await.unwrap();
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 7)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            db::folders::get_folder(&conn, "INBOX")
                .await
                .unwrap()
                .unwrap()
                .uid_validity,
            2
        );
    }

    #[test]
    fn chunks_bound_existing_messages_even_with_sparse_uids() {
        assert_eq!(header_chunks(&[]).count(), 0);
        let sparse = [1, 10_000_000, u32::MAX];
        assert_eq!(
            header_chunks(&sparse).collect::<Vec<_>>(),
            vec!["1,10000000,4294967295"]
        );

        let uids: Vec<_> = (1..=501).map(|uid| uid * 1_000_000).collect();
        let chunks: Vec<_> = header_chunks(&uids).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].split(',').count(), 250);
        assert_eq!(chunks[1].split(',').count(), 250);
        assert_eq!(chunks[2].split(',').count(), 1);
        let fetched: Vec<u32> = chunks
            .iter()
            .flat_map(|chunk| chunk.split(',').map(|uid| uid.parse().unwrap()))
            .collect();
        assert_eq!(fetched, uids);
    }

    #[tokio::test]
    async fn empty_background_sync_records_completion() {
        // Both an exhausted range and an empty server listing are successful syncs.
        for uid_range in [(1, 0), (1, 100)] {
            let data_dir = tempfile::tempdir().unwrap();
            let path = data_dir.path().to_str().unwrap();
            let user_hash = "empty-sync";
            let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
            db::folders::upsert_folder(&conn, "Sent", None, None, "", true, 99, 7, 1, 0)
                .await
                .unwrap();
            db::folders::invalidate_folder_freshness(&conn, "Sent")
                .await
                .unwrap();
            assert!(
                !db::folders::is_folder_fresh(&conn, "Sent", 30)
                    .await
                    .unwrap()
            );
            let header_sync = HeaderSync::capture(&conn).await.unwrap();
            drop(conn);

            let client = Arc::new(MockImapClient::new());
            let config = Arc::new(AppConfig {
                data_dir: path.to_owned(),
                ..serde_json::from_str::<AppConfig>("{}").unwrap()
            });
            let event_bus = Arc::new(EventBus::new());
            let mut events = event_bus.subscribe(user_hash).await;

            sync_remaining_headers(BackgroundSyncParams {
                creds: ImapCredentials {
                    host: "localhost".into(),
                    port: 143,
                    tls: false,
                    email: "test@example.com".into(),
                    password: "test".into(),
                },
                folder: "Sent".into(),
                imap_client: client.clone(),
                config,
                user_hash: user_hash.into(),
                search_engine: Arc::new(SearchEngine::new(data_dir.path().to_path_buf())),
                event_bus,
                uid_range,
                header_sync,
                uid_validity: 42,
                exists: 0,
            })
            .await;

            let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
            let folder = db::folders::get_folder(&conn, "Sent")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(folder.uid_validity, 42);
            assert_eq!(folder.total_count, 0);
            assert_eq!(folder.unread_count, 0);
            assert!(
                db::folders::is_folder_fresh(&conn, "Sent", 30)
                    .await
                    .unwrap()
            );
            assert!(
                !db::folders::is_folder_messages_invalidated(&conn, "Sent")
                    .await
                    .unwrap()
            );
            assert!(client.header_requests.lock().unwrap().is_empty());
            assert!(matches!(
                events.try_recv().unwrap(),
                MailEvent::FolderUpdated
            ));
            assert!(events.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn sparse_background_sync_fetches_existing_uids_in_one_batch() {
        let data_dir = tempfile::tempdir().unwrap();
        let path = data_dir.path().to_str().unwrap();
        let user_hash = "sparse-sync";
        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        db::folders::upsert_folder(&conn, "INBOX", None, None, "", true, 0, 0, 1, 0)
            .await
            .unwrap();
        let header_sync = HeaderSync::capture(&conn).await.unwrap();
        drop(conn);
        let headers = [1, 9_999_899, 10_000_000].map(|uid| crate::imap::types::ImapMessageHeader {
            uid,
            subject: Some(format!("Message {uid}")),
            from: Vec::new(),
            to: Vec::new(),
            cc: Vec::new(),
            date: None,
            flags: Vec::new(),
            has_attachments: false,
            size: 42,
            message_id: None,
            in_reply_to: None,
            references: None,
            reaction: None,
        });
        let client = Arc::new(MockImapClient::new().with_headers(headers.to_vec()));
        let config = Arc::new(AppConfig {
            data_dir: path.to_owned(),
            ..serde_json::from_str::<AppConfig>("{}").unwrap()
        });
        let event_bus = Arc::new(EventBus::new());
        let mut events = event_bus.subscribe(user_hash).await;

        sync_remaining_headers(BackgroundSyncParams {
            creds: ImapCredentials {
                host: "localhost".into(),
                port: 143,
                tls: false,
                email: "test@example.com".into(),
                password: "test".into(),
            },
            folder: "INBOX".into(),
            imap_client: client.clone(),
            config,
            user_hash: user_hash.into(),
            search_engine: Arc::new(SearchEngine::new(data_dir.path().to_path_buf())),
            event_bus,
            uid_range: (1, 9_999_900),
            header_sync,
            uid_validity: 1,
            exists: 3,
        })
        .await;

        assert_eq!(
            *client.header_requests.lock().unwrap(),
            vec![("INBOX".into(), "1,9999899".into())]
        );
        assert!(matches!(
            events.try_recv().unwrap(),
            MailEvent::FolderUpdated
        ));
        let conn = db::pool::open_user_db(path, user_hash).await.unwrap();
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 1)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            db::messages::get_single_message(&conn, "INBOX", 9_999_899)
                .await
                .unwrap()
                .is_some()
        );
    }
}
