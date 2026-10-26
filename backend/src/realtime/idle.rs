use std::sync::Arc;

use dashmap::DashMap;
use futures::StreamExt;
use tokio::task::JoinHandle;

use crate::config::AppConfig;
use crate::imap::client::ImapCredentials;
use crate::imap::connection::ImapStream;
use crate::imap::parse::{decode_rfc2047, flag_to_string, has_attachments, imap_address_to_email};
use crate::realtime::events::{EventBus, MailEvent};
use crate::{db, imap};

/// Manages long-lived IMAP IDLE connections, one per (user, folder) pair.
/// Each WebSocket holds a subscription; only the last disconnect stops IDLE.
pub struct IdleManager {
    tasks: DashMap<(String, String), IdleTask>,
}

struct IdleTask {
    handle: JoinHandle<()>,
    subscribers: usize,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    /// Subscribe to IDLE for a user + folder, sharing an existing task.
    pub async fn start_idle(
        &self,
        user_hash: String,
        folder: String,
        creds: ImapCredentials,
        event_bus: Arc<EventBus>,
        config: Arc<AppConfig>,
    ) {
        use dashmap::mapref::entry::Entry;

        match self.tasks.entry((user_hash.clone(), folder.clone())) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().subscribers += 1;
            }
            Entry::Vacant(entry) => {
                let handle = tokio::spawn(async move {
                    idle_loop(&user_hash, &folder, &creds, &event_bus, &config).await;
                });
                entry.insert(IdleTask {
                    handle,
                    subscribers: 1,
                });
            }
        }
    }

    /// Release this socket's subscription, without stopping other sockets.
    pub async fn stop_idle(&self, user_hash: &str, folder: &str) {
        use dashmap::mapref::entry::Entry;

        let key = (user_hash.to_string(), folder.to_string());
        if let Entry::Occupied(mut entry) = self.tasks.entry(key) {
            if entry.get().subscribers > 1 {
                entry.get_mut().subscribers -= 1;
            } else {
                entry.remove().handle.abort();
            }
        }
    }
}

impl Default for IdleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The inner IDLE loop with auto-reconnect and exponential backoff.
///
/// This opens a dedicated IMAP connection, SELECTs the folder, and enters
/// IDLE mode. When the server notifies of changes, it publishes an event
/// to the EventBus and re-enters IDLE. If the connection drops, it
/// reconnects with exponential backoff.
async fn idle_loop(
    user_hash: &str,
    folder: &str,
    creds: &ImapCredentials,
    event_bus: &EventBus,
    config: &AppConfig,
) {
    let mut backoff = std::time::Duration::from_secs(1);
    let max_backoff = std::time::Duration::from_secs(60);

    loop {
        match run_idle_session(user_hash, folder, creds, event_bus, config).await {
            Ok(()) => {
                // Session ended normally (shouldn't happen in practice).
                tracing::info!(user_hash = %user_hash, folder = %folder, "IDLE session ended normally");
                break;
            }
            Err(e) => {
                tracing::warn!(
                    user_hash = %user_hash,
                    folder = %folder,
                    error = %e,
                    backoff_secs = backoff.as_secs(),
                    "IDLE connection failed, will retry"
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

/// Run a single IDLE session. Returns Err on connection/protocol errors.
///
/// The async-imap IDLE API follows an ownership pattern:
/// `Session` → `.idle()` consumes session → `Handle` → `.init()` sends IDLE
/// → `.wait()` listens → `.done()` sends DONE and returns `Session`.
async fn run_idle_session(
    user_hash: &str,
    folder: &str,
    creds: &ImapCredentials,
    event_bus: &EventBus,
    config: &AppConfig,
) -> Result<(), String> {
    use imap::client::connect;

    let mut session = connect(creds)
        .await
        .map_err(|e| format!("IDLE connect failed: {e}"))?;

    session
        .select_condstore(folder)
        .await
        .map_err(|e| format!("IDLE SELECT failed: {e}"))?;

    tracing::info!(user_hash = %user_hash, folder = %folder, "IDLE session started");

    // Re-enter IDLE every 25 minutes (RFC recommends max 29 min).
    let idle_timeout = std::time::Duration::from_secs(25 * 60);

    loop {
        // `.idle()` consumes the session, wrapping it in a Handle.
        let mut idle_handle = session.idle();

        // Initialize the IDLE command with the server.
        idle_handle
            .init()
            .await
            .map_err(|e| format!("IDLE init failed: {e}"))?;

        // Start listening for server notifications.
        // IMPORTANT: `interrupt` must be kept alive — dropping it triggers
        // ManualInterrupt and ends the wait immediately.
        let (idle_wait, interrupt) = idle_handle.wait();

        // Wait for the server to send an unsolicited response, or timeout.
        let result = tokio::time::timeout(idle_timeout, idle_wait).await;

        // Drop the interrupt handle now that we're done waiting.
        drop(interrupt);

        // Send DONE to end IDLE and get the session back.
        session = idle_handle
            .done()
            .await
            .map_err(|e| format!("IDLE done failed: {e}"))?;

        match result {
            Ok(Ok(idle_response)) => {
                use async_imap::extensions::idle::IdleResponse;
                match idle_response {
                    IdleResponse::NewData(_) => {
                        tracing::info!(
                            user_hash = %user_hash,
                            folder = %folder,
                            "IDLE: new data from server"
                        );

                        // CONDSTORE sync is fast (~200-500ms), so run it
                        // BEFORE publishing. This ensures the DB is up-to-date
                        // when the frontend refetches.
                        let result;
                        (session, result) = fetch_new_after_idle(
                            session, user_hash, folder, config,
                        )
                        .await;

                        // Publish the correct event based on what changed.
                        if result.new_count > 0 {
                            event_bus
                                .publish(
                                    user_hash,
                                    MailEvent::NewMessages {
                                        folder: folder.to_string(),
                                        count: result.new_count,
                                        latest_sender: result.latest_sender,
                                        latest_subject: result.latest_subject,
                                    },
                                )
                                .await;
                        } else if result.flags_updated > 0 || result.deleted_count > 0 {
                            event_bus
                                .publish(
                                    user_hash,
                                    MailEvent::FlagsChanged {
                                        folder: folder.to_string(),
                                    },
                                )
                                .await;
                        }
                    }
                    IdleResponse::Timeout => {
                        tracing::debug!(
                            user_hash = %user_hash,
                            folder = %folder,
                            "IDLE: server-side timeout, re-entering"
                        );
                    }
                    IdleResponse::ManualInterrupt => {
                        tracing::debug!(
                            user_hash = %user_hash,
                            folder = %folder,
                            "IDLE: manual interrupt, re-entering"
                        );
                    }
                }
            }
            Ok(Err(e)) => {
                // IDLE protocol error.
                return Err(format!("IDLE error: {e}"));
            }
            Err(_) => {
                // Our timeout — re-enter IDLE to keep the connection alive.
                tracing::debug!(
                    user_hash = %user_hash,
                    folder = %folder,
                    "IDLE keepalive timeout, re-entering"
                );
            }
        }
    }
}

/// Summary of what changed during a post-IDLE reconciliation.
struct ReconcileResult {
    new_count: u32,
    flags_updated: u32,
    deleted_count: u32,
    latest_sender: Option<String>,
    latest_subject: Option<String>,
}

impl ReconcileResult {
    fn empty() -> Self {
        Self {
            new_count: 0,
            flags_updated: 0,
            deleted_count: 0,
            latest_sender: None,
            latest_subject: None,
        }
    }
}

/// Reconcile the local cache after IDLE detects new data.
///
/// Uses CONDSTORE when available for fast incremental sync (~200-500ms),
/// falling back to a full UID scan when CONDSTORE is not supported.
///
/// If anything fails, it logs a warning and returns the session unchanged —
/// the periodic sync loop provides a safety net.
async fn fetch_new_after_idle(
    mut session: async_imap::Session<ImapStream>,
    user_hash: &str,
    folder: &str,
    config: &AppConfig,
) -> (async_imap::Session<ImapStream>, ReconcileResult) {
    // Read cached folder state from DB.
    let (cached_modseq, max_cached_uid, cached_count, cached_uid_validity, header_sync) =
        match db::pool::open_user_db(&config.data_dir, user_hash) {
            Ok(conn) => {
                let header_sync = match db::header_sync::HeaderSync::capture(&conn) {
                    Ok(sync) => sync,
                    Err(e) => {
                        tracing::warn!(error = %e, "IDLE fetch: failed to read header generation");
                        return (session, ReconcileResult::empty());
                    }
                };
                let cached = db::folders::get_folder(&conn, folder).ok().flatten();
                let modseq = cached.as_ref().map(|f| f.highest_modseq).unwrap_or(0);
                let validity = cached.as_ref().map(|f| f.uid_validity).unwrap_or(0);
                let max_uid = db::messages::max_uid(&conn, folder).unwrap_or(0);
                let count = db::messages::count_messages(&conn, folder).unwrap_or(0);
                (modseq, max_uid, count, validity, header_sync)
            }
            Err(e) => {
                tracing::warn!(error = %e, "IDLE fetch: failed to open DB");
                return (session, ReconcileResult::empty());
            }
        };

    // Re-SELECT with CONDSTORE to get fresh mailbox state.
    let mailbox = match session.select_condstore(folder).await {
        Ok(mb) => mb,
        Err(e) => {
            tracing::warn!(error = %e, "IDLE fetch: select_condstore failed");
            return (session, ReconcileResult::empty());
        }
    };

    let server_exists = mailbox.exists;
    let server_modseq = mailbox.highest_modseq.unwrap_or(0);
    let uid_validity = mailbox.uid_validity.unwrap_or(0);

    // Let list sync reset the namespace before IDLE writes any reused UIDs.
    if cached_uid_validity != 0 && cached_uid_validity != uid_validity {
        if let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash) {
            let _ = db::folders::invalidate_folder_freshness(&conn, folder);
        }
        return (session, ReconcileResult::empty());
    }

    // Early exit: nothing changed (CONDSTORE modseq and count both match).
    if server_modseq > 0 && server_modseq == cached_modseq && server_exists == cached_count {
        tracing::debug!(
            user_hash = %user_hash,
            folder = %folder,
            modseq = server_modseq,
            "IDLE fetch: nothing changed (modseq match)"
        );
        return (session, ReconcileResult::empty());
    }

    // ── CONDSTORE fast path ──────────────────────────────────────────
    if cached_modseq > 0 && server_modseq > 0 {
        let fetch_query = format!("(UID FLAGS) (CHANGEDSINCE {})", cached_modseq);
        let changed_items: Option<Vec<(u32, Vec<String>)>> =
            match session.uid_fetch("1:*", &fetch_query).await {
                Ok(stream) => {
                    let mut stream = std::pin::pin!(stream);
                    let mut items = Vec::new();
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(fetch) => {
                                if let Some(uid) = fetch.uid {
                                    let flags: Vec<String> =
                                        fetch.flags().map(|f| flag_to_string(&f)).collect();
                                    items.push((uid, flags));
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "IDLE fetch: error reading CHANGEDSINCE stream"
                                );
                                break;
                            }
                        }
                    }
                    Some(items)
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "IDLE fetch: CHANGEDSINCE fetch failed, falling back to full scan"
                    );
                    None
                }
            };

        if let Some(changed) = changed_items {
            // Separate flag changes (uid ≤ max) from new messages (uid > max).
            let mut flag_changes: Vec<(u32, Vec<String>)> = Vec::new();
            let mut has_new = false;
            for (uid, flags) in changed {
                if uid > max_cached_uid {
                    has_new = true;
                } else {
                    flag_changes.push((uid, flags));
                }
            }

            // Apply flag changes to DB.
            let mut flags_updated = 0u32;
            if !flag_changes.is_empty()
                && let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash)
            {
                for (uid, flags) in &flag_changes {
                    let mut sorted = flags.clone();
                    sorted.sort();
                    let flags_csv = sorted.join(",");
                    if db::messages::update_message_flags(&conn, folder, *uid, &flags_csv)
                        .is_ok()
                    {
                        flags_updated += 1;
                    }
                }
            }

            // Fetch full headers for new messages.
            let mut new_count = 0u32;
            if has_new {
                let uid_range = format!("{}:*", max_cached_uid + 1);
                new_count = fetch_and_store_headers(
                    &mut session, user_hash, folder, config, &uid_range, max_cached_uid, header_sync,
                )
                .await;
            }

            // Detect deletions if server count < cached count.
            let mut deleted_count = 0u32;
            if server_exists < cached_count {
                deleted_count =
                    fetch_uids_and_remove_deleted(&mut session, user_hash, folder, config).await;
            }

            // Look up the latest message's sender/subject if new messages arrived.
            let (latest_sender, latest_subject) = if new_count > 0 {
                lookup_latest_message(config, user_hash, folder)
            } else {
                (None, None)
            };

            // Update folder sync status with new modseq.
            if let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash)
                && let Ok(transaction) = header_sync.begin_write(&conn)
            {
                let _ = db::folders::update_folder_sync_status(
                    &conn, folder, uid_validity, server_exists, server_modseq,
                );
                let _ = db::folders::refresh_unread_count(&conn, folder);
                let _ = transaction.commit();
            }

            if flags_updated > 0 || new_count > 0 || deleted_count > 0 {
                tracing::info!(
                    user_hash = %user_hash,
                    folder = %folder,
                    flags_updated = flags_updated,
                    new = new_count,
                    deleted = deleted_count,
                    "Reconciled cache after IDLE (CONDSTORE)"
                );
            }

            return (session, ReconcileResult {
                new_count,
                flags_updated,
                deleted_count,
                latest_sender,
                latest_subject,
            });
        }
        // CHANGEDSINCE fetch failed — fall through to full scan below.
    }

    // ── Fallback: full UID scan (no CONDSTORE) ───────────────────────
    let cached_uids: std::collections::HashSet<u32> =
        match db::pool::open_user_db(&config.data_dir, user_hash) {
            Ok(conn) => {
                let uids_flags =
                    db::messages::get_all_uids_and_flags(&conn, folder).unwrap_or_default();
                uids_flags.iter().map(|(uid, _)| *uid).collect()
            }
            Err(e) => {
                tracing::warn!(error = %e, "IDLE fetch: failed to open DB for fallback");
                return (session, ReconcileResult::empty());
            }
        };

    // Fetch all UIDs from server.
    let imap_uids: Option<std::collections::HashSet<u32>> =
        match session.uid_fetch("1:*", "UID").await {
            Ok(stream) => {
                let mut stream = std::pin::pin!(stream);
                let mut uids = std::collections::HashSet::new();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(fetch) => {
                            if let Some(uid) = fetch.uid {
                                uids.insert(uid);
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "IDLE fetch: error reading UID stream");
                            break;
                        }
                    }
                }
                Some(uids)
            }
            Err(e) => {
                tracing::warn!(error = %e, "IDLE fetch: uid_fetch(UIDs) failed");
                None
            }
        };

    let Some(imap_uids) = imap_uids else {
        return (session, ReconcileResult::empty());
    };

    // Fetch headers for new messages.
    let has_new = imap_uids.iter().any(|&uid| uid > max_cached_uid);
    let mut new_count = 0u32;
    if has_new {
        let uid_range = format!("{}:*", max_cached_uid + 1);
        new_count = fetch_and_store_headers(
            &mut session, user_hash, folder, config, &uid_range, max_cached_uid, header_sync,
        )
        .await;
    }

    // Delete cached UIDs no longer on server.
    let mut deleted_count = 0u32;
    let deleted_uids: Vec<u32> = cached_uids
        .iter()
        .copied()
        .filter(|uid| !imap_uids.contains(uid))
        .collect();
    if !deleted_uids.is_empty()
        && let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash)
    {
        for uid in &deleted_uids {
            if db::messages::delete_message(&conn, folder, *uid).is_ok() {
                deleted_count += 1;
            }
        }
    }

    // Update folder sync status.
    if let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash)
        && let Ok(transaction) = header_sync.begin_write(&conn)
    {
        let _ = db::folders::update_folder_sync_status(
            &conn, folder, uid_validity, server_exists, server_modseq,
        );
        let _ = db::folders::refresh_unread_count(&conn, folder);
        let _ = transaction.commit();
    }

    // Look up the latest message's sender/subject if new messages arrived.
    let (latest_sender, latest_subject) = if new_count > 0 {
        lookup_latest_message(config, user_hash, folder)
    } else {
        (None, None)
    };

    if new_count > 0 || deleted_count > 0 {
        tracing::info!(
            user_hash = %user_hash,
            folder = %folder,
            new = new_count,
            deleted = deleted_count,
            "Reconciled cache after IDLE (full scan)"
        );
    }

    (session, ReconcileResult {
        new_count,
        flags_updated: 0,
        deleted_count,
        latest_sender,
        latest_subject,
    })
}

/// Look up the most recent message in the DB for sender/subject details.
fn lookup_latest_message(
    config: &AppConfig,
    user_hash: &str,
    folder: &str,
) -> (Option<String>, Option<String>) {
    let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash) else {
        return (None, None);
    };
    let max_uid = db::messages::max_uid(&conn, folder).unwrap_or(0);
    if max_uid == 0 {
        return (None, None);
    }
    match db::messages::get_single_message(&conn, folder, max_uid) {
        Ok(Some(msg)) => {
            let sender = if !msg.from_name.is_empty() {
                Some(msg.from_name)
            } else if !msg.from_address.is_empty() {
                Some(msg.from_address)
            } else {
                None
            };
            let subject = if !msg.subject.is_empty() {
                Some(msg.subject)
            } else {
                None
            };
            (sender, subject)
        }
        _ => (None, None),
    }
}

/// Fetch full headers for messages in the given UID range and store them in the DB.
/// Skips messages with UID ≤ `min_uid` (handles the `*` wildcard returning the last message).
/// Returns the number of new messages stored.
async fn fetch_and_store_headers(
    session: &mut async_imap::Session<ImapStream>,
    user_hash: &str,
    folder: &str,
    config: &AppConfig,
    uid_range: &str,
    min_uid: u32,
    header_sync: db::header_sync::HeaderSync,
) -> u32 {
    let fetches: Option<Vec<_>> = match session
        .uid_fetch(
            uid_range,
            "(UID ENVELOPE FLAGS BODYSTRUCTURE RFC822.SIZE BODY.PEEK[HEADER.FIELDS (Message-ID In-Reply-To References Content-Class x-ms-exchange-generated-message-class)])",
        )
        .await
    {
        Ok(stream) => {
            let mut stream = std::pin::pin!(stream);
            let mut items = Vec::new();
            while let Some(result) = stream.next().await {
                match result {
                    Ok(fetch) => items.push(fetch),
                    Err(e) => {
                        tracing::warn!(error = %e, "IDLE fetch: error reading header stream");
                        break;
                    }
                }
            }
            Some(items)
        }
        Err(e) => {
            tracing::warn!(error = %e, "IDLE fetch: uid_fetch(headers) failed");
            None
        }
    };

    let mut new_count = 0u32;
    if let Some(fetches) = fetches
        && let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash)
    {
        let transaction = match header_sync.begin_write(&conn) {
            Ok(transaction) => transaction,
            Err(e) => {
                tracing::debug!(error = %e, "IDLE fetch: discarded stale headers");
                return 0;
            }
        };
        for fetch in &fetches {
            let uid = match fetch.uid {
                Some(u) if u > min_uid => u,
                Some(_) => continue, // UID "*" can return the last message
                None => continue,
            };

            // Parse threading headers.
            let raw_header_bytes = fetch.header();
            let parsed_threading = raw_header_bytes
                .and_then(|raw| mail_parser::MessageParser::default().parse(raw));

            let (subject, from, to, cc, date) = if let Some(env) = fetch.envelope() {
                let subject = env
                    .subject
                    .as_ref()
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .map(decode_rfc2047);

                let from: Vec<_> = env
                    .from
                    .as_ref()
                    .map(|addrs| addrs.iter().map(imap_address_to_email).collect())
                    .unwrap_or_default();

                let to: Vec<_> = env
                    .to
                    .as_ref()
                    .map(|addrs| addrs.iter().map(imap_address_to_email).collect())
                    .unwrap_or_default();

                let cc: Vec<_> = env
                    .cc
                    .as_ref()
                    .map(|addrs| addrs.iter().map(imap_address_to_email).collect())
                    .unwrap_or_default();

                let date = env
                    .date
                    .as_ref()
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .map(|s| s.to_string());

                (subject, from, to, cc, date)
            } else {
                (None, vec![], vec![], vec![], None)
            };

            let message_id = parsed_threading
                .as_ref()
                .and_then(|p| p.message_id().map(|s| format!("<{s}>")));
            let in_reply_to = parsed_threading.as_ref().and_then(|p| {
                let val = p.in_reply_to();
                val.as_text().map(|s| format!("<{s}>"))
            });
            let references = parsed_threading.as_ref().and_then(|p| {
                let val = p.references();
                val.as_text_list()
                    .map(|list| {
                        list.iter()
                            .map(|s| format!("<{s}>"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .or_else(|| val.as_text().map(|s| format!("<{s}>")))
            });

            // Detect Outlook/Exchange reaction emails from headers.
            let reaction = raw_header_bytes.and_then(|raw| {
                let header_str = std::str::from_utf8(raw).ok()?;
                let lower = header_str.to_lowercase();
                let is_reaction = lower.contains("content-class: activitynotification")
                    || lower.contains("urn:content-class:reaction");
                if !is_reaction {
                    return None;
                }
                let subj = subject.as_deref().unwrap_or("").to_lowercase();
                let emoji = match subj.trim() {
                    s if s.contains("like") => "\u{1f44d}",
                    s if s.contains("heart") || s.contains("love") => "\u{2764}\u{fe0f}",
                    s if s.contains("laugh") => "\u{1f604}",
                    s if s.contains("surprised") || s.contains("wow") => "\u{1f62e}",
                    s if s.contains("sad") => "\u{1f622}",
                    s if s.contains("angry") => "\u{1f620}",
                    _ => "\u{1f44d}",
                };
                Some(emoji.to_string())
            });

            let flags: Vec<String> = fetch.flags().map(|f| flag_to_string(&f)).collect();
            let has_attach = fetch
                .bodystructure()
                .map(|bs| has_attachments(bs))
                .unwrap_or(false);
            let size = fetch.size.unwrap_or(0);

            let from_address = from.first().map(|a| a.address.as_str()).unwrap_or("");
            let from_name = from.first().and_then(|a| a.name.as_deref()).unwrap_or("");
            let to_json = serde_json::to_string(&to).unwrap_or_else(|_| "[]".to_string());
            let cc_json = serde_json::to_string(&cc).unwrap_or_else(|_| "[]".to_string());
            let subject_str = subject.as_deref().unwrap_or("");
            let date_str = date.as_deref().unwrap_or("");
            let flags_csv = flags.join(",");

            if let Err(e) = db::messages::upsert_message(
                &conn,
                folder,
                uid,
                message_id.as_deref(),
                in_reply_to.as_deref(),
                references.as_deref(),
                subject_str,
                from_address,
                from_name,
                &to_json,
                &cc_json,
                date_str,
                &flags_csv,
                size,
                has_attach,
                "",
                reaction.as_deref(),
            ) {
                tracing::warn!(uid = uid, error = %e, "IDLE fetch: failed to upsert message");
            } else {
                // Populate denormalized known_addresses table.
                if let Err(e) = db::contacts::populate_known_addresses(
                    &conn, from_address, from_name, &to_json, &cc_json,
                ) {
                    tracing::warn!(uid = uid, error = %e, "IDLE fetch: failed to populate known addresses");
                }
                new_count += 1;
            }
        }
        if let Err(e) = transaction.commit() {
            tracing::warn!(error = %e, "IDLE fetch: failed to commit headers");
            return 0;
        }
    }

    new_count
}

/// Fetch all UIDs from the server and remove cached messages no longer present.
/// Used for deletion detection when server count < cached count.
/// Returns the number of deleted messages.
async fn fetch_uids_and_remove_deleted(
    session: &mut async_imap::Session<ImapStream>,
    user_hash: &str,
    folder: &str,
    config: &AppConfig,
) -> u32 {
    let imap_uids: Option<std::collections::HashSet<u32>> =
        match session.uid_fetch("1:*", "UID").await {
            Ok(stream) => {
                let mut stream = std::pin::pin!(stream);
                let mut uids = std::collections::HashSet::new();
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(fetch) => {
                            if let Some(uid) = fetch.uid {
                                uids.insert(uid);
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "IDLE fetch: error reading UID stream for deletion check"
                            );
                            break;
                        }
                    }
                }
                Some(uids)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "IDLE fetch: uid_fetch(UIDs) failed for deletion check"
                );
                None
            }
        };

    let Some(imap_uids) = imap_uids else {
        return 0;
    };

    let mut deleted_count = 0u32;
    if let Ok(conn) = db::pool::open_user_db(&config.data_dir, user_hash) {
        let cached = db::messages::get_all_uids_and_flags(&conn, folder).unwrap_or_default();
        for (uid, _) in &cached {
            if !imap_uids.contains(uid)
                && db::messages::delete_message(&conn, folder, *uid).is_ok()
            {
                deleted_count += 1;
            }
        }
    }

    deleted_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn overlapping_sockets_keep_idle_until_the_last_disconnect() {
        let manager = IdleManager::new();
        let config = Arc::new(serde_json::from_str::<AppConfig>("{}").unwrap());
        let event_bus = Arc::new(EventBus::new());
        let creds = ImapCredentials {
            host: "localhost".into(),
            port: 143,
            tls: false,
            email: "test@example.com".into(),
            password: "test".into(),
        };
        let key = ("user".to_string(), "INBOX".to_string());

        manager
            .start_idle(
                key.0.clone(),
                key.1.clone(),
                creds.clone(),
                event_bus.clone(),
                config.clone(),
            )
            .await;
        let original_task = manager.tasks.get(&key).unwrap().handle.id();
        // A reconnect may subscribe before the old socket finishes closing.
        manager
            .start_idle(
                key.0.clone(),
                key.1.clone(),
                creds.clone(),
                event_bus.clone(),
                config.clone(),
            )
            .await;
        assert_eq!(manager.tasks.len(), 1);
        assert_eq!(manager.tasks.get(&key).unwrap().subscribers, 2);
        assert_eq!(manager.tasks.get(&key).unwrap().handle.id(), original_task);

        manager.stop_idle(&key.0, &key.1).await;
        assert_eq!(manager.tasks.get(&key).unwrap().subscribers, 1);
        assert!(!manager.tasks.get(&key).unwrap().handle.is_finished());
        manager.stop_idle(&key.0, &key.1).await;
        assert!(manager.tasks.is_empty());

        // A connection arriving after the last disconnect gets a new task.
        manager
            .start_idle(key.0.clone(), key.1.clone(), creds, event_bus, config)
            .await;
        assert_ne!(manager.tasks.get(&key).unwrap().handle.id(), original_task);
        manager.stop_idle(&key.0, &key.1).await;
        manager.stop_idle(&key.0, &key.1).await;
        assert!(manager.tasks.is_empty());
    }
}
