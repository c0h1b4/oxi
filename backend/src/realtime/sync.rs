use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::db;
use crate::email_theme;
use crate::imap::client::{ImapClient, ImapCredentials};
use crate::realtime::events::{EventBus, MailEvent};
use crate::search::engine::UserIndex;
use crate::search::engine::{IndexableMessage, SearchEngine};

/// How often to run a sync check (seconds).
/// Reconcile every three minutes, including an INBOX safety net if IDLE drops.
const SYNC_INTERVAL_SECS: u64 = 180;

/// Run a periodic reconciliation loop for a user.
///
/// Uses a 3-tier strategy per folder:
/// 1. STATUS pre-check (cheap, no SELECT)
/// 2. CONDSTORE incremental fetch (only changed flags)
/// 3. Full fetch fallback (when CONDSTORE unavailable)
pub async fn sync_loop(
    user_hash: String,
    creds: ImapCredentials,
    config: Arc<AppConfig>,
    imap_client: Arc<dyn ImapClient>,
    event_bus: Arc<EventBus>,
    search_engine: Arc<SearchEngine>,
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(SYNC_INTERVAL_SECS));
    // Skip the first immediate tick — IDLE + initial list_messages handles that.
    interval.tick().await;

    loop {
        interval.tick().await;

        if let Err(e) = crate::imap::pool::background(run_sync(
            &user_hash,
            &creds,
            &config,
            imap_client.as_ref(),
            &event_bus,
            search_engine.clone(),
        ))
        .await
        {
            tracing::warn!(
                user = %creds.email,
                error = %e,
                "Periodic sync failed, will retry next interval"
            );
        }
    }
}

async fn run_sync(
    user_hash: &str,
    creds: &ImapCredentials,
    config: &AppConfig,
    imap_client: &dyn ImapClient,
    event_bus: &EventBus,
    search_engine: Arc<SearchEngine>,
) -> Result<(), String> {
    let database = config.user_db(user_hash).await?;
    let folder_snapshots = db::folders::get_all_folders(&database)
        .await?
        .into_iter()
        .map(|f| FolderSnapshot {
            name: f.name,
            uid_validity: f.uid_validity,
            highest_modseq: f.highest_modseq,
        })
        .collect::<Vec<_>>();

    let mut any_changes = false;

    for folder in &folder_snapshots {
        let folder_name = &folder.name;

        // ── Tier 1: STATUS pre-check (no SELECT needed) ──────────────
        let status = match imap_client.folder_status_extended(creds, folder_name).await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(
                    folder = %folder_name,
                    error = %e,
                    "Skipping folder sync (STATUS failed)"
                );
                continue;
            }
        };

        // Check UIDVALIDITY — if changed, the folder was rebuilt.
        if folder.uid_validity != 0 && status.uid_validity != folder.uid_validity {
            tracing::info!(
                folder = %folder_name,
                old_validity = folder.uid_validity,
                new_validity = status.uid_validity,
                "UIDVALIDITY changed, invalidating folder"
            );
            db::folders::invalidate_folder_freshness(&database, folder_name).await?;
            any_changes = true;
            event_bus.publish(user_hash, MailEvent::FolderUpdated).await;
            continue;
        }

        // If CONDSTORE is supported (highest_modseq > 0) and matches cached → skip.
        let cached_modseq = folder.highest_modseq;
        if status.highest_modseq > 0 && cached_modseq > 0 && status.highest_modseq == cached_modseq
        {
            let cached_count = db::messages::count_messages(&database, folder_name).await?;
            if status.exists == cached_count {
                tracing::debug!(
                    folder = %folder_name,
                    modseq = status.highest_modseq,
                    "Skipping unchanged folder"
                );
                continue;
            }
        }

        // ── Tier 2: CONDSTORE incremental fetch ──────────────────────
        if status.highest_modseq > 0 && cached_modseq > 0 {
            let folder_changed = sync_condstore(
                config,
                user_hash,
                creds,
                imap_client,
                folder_name,
                cached_modseq,
                &status,
                event_bus,
            )
            .await?;
            if folder_changed {
                any_changes = true;
            }
            continue;
        }

        // ── Tier 3: Full fetch fallback ──────────────────────────────
        let folder_changed = sync_full(
            config,
            user_hash,
            creds,
            imap_client,
            folder_name,
            &status,
            event_bus,
        )
        .await?;
        if folder_changed {
            any_changes = true;
        }
    }

    if any_changes {
        event_bus.publish(user_hash, MailEvent::FolderUpdated).await;
    }

    // ── Deep index phase: fetch & index bodies if enabled ────────────
    if let Err(e) = index_message_bodies(user_hash, creds, config, imap_client, search_engine).await
    {
        tracing::debug!(error = %e, "Deep index phase skipped or failed");
    }

    Ok(())
}

/// Batch size for deep indexing per sync cycle.
const DEEP_INDEX_BATCH: u32 = 10;

/// If the `deep_index` preference is enabled, fetch bodies for messages that
/// don't have a cached body yet, cache them, and re-index with body text.
async fn index_message_bodies(
    user_hash: &str,
    creds: &ImapCredentials,
    config: &AppConfig,
    imap_client: &dyn ImapClient,
    search_engine: Arc<SearchEngine>,
) -> Result<(), String> {
    let database = config.user_db(user_hash).await?;
    if !db::display_preferences::get_preferences(&database)
        .await?
        .deep_index
    {
        return Ok(());
    }
    let unindexed = db::messages::get_unindexed_messages(&database, DEEP_INDEX_BATCH).await?;

    if unindexed.is_empty() {
        return Ok(());
    }

    tracing::debug!(count = unindexed.len(), "Deep indexing message bodies");

    for (folder, uid) in &unindexed {
        if UserIndex::is_excluded_folder(folder) {
            continue;
        }

        let body = match imap_client.fetch_body(creds, folder, *uid).await {
            Ok(b) => b,
            Err(e) => {
                tracing::debug!(folder = %folder, uid = uid, error = %e, "Deep index: failed to fetch body");
                continue;
            }
        };

        let att_json = serde_json::to_string(&body.attachments).ok();
        let detected_theme = body
            .text_html
            .as_ref()
            .and_then(|h| email_theme::detect_email_theme(h))
            .map(|t| t.as_i32());
        db::messages::cache_message_body(
            &database,
            folder,
            *uid,
            body.text_html.as_deref(),
            body.text_plain.as_deref(),
            att_json.as_deref(),
            Some(&body.raw_headers),
            detected_theme,
        )
        .await?;
        if let Some(text) = body.text_plain
            && let Some(msg) = db::messages::get_single_message(&database, folder, *uid).await?
        {
            let engine = search_engine.clone();
            let hash = user_hash.to_owned();
            let message = IndexableMessage {
                uid: msg.uid,
                folder: msg.folder,
                subject: msg.subject,
                from_address: msg.from_address,
                from_name: msg.from_name,
                to_addresses: msg.to_addresses,
                body_text: text,
                date_epoch: db::messages::parse_date_to_epoch_public(&msg.date),
                has_attachments: msg.has_attachments,
            };
            tokio::task::spawn_blocking(move || {
                engine.open_user_index(&hash)?.index_message(&message)
            })
            .await
            .map_err(|e| e.to_string())??;
        }
    }

    tracing::debug!(count = unindexed.len(), "Deep index batch complete");
    Ok(())
}

/// Lightweight snapshot of a cached folder for the sync loop.
struct FolderSnapshot {
    name: String,
    uid_validity: u32,
    highest_modseq: u64,
}

/// CONDSTORE path: fetch only changed flags, detect deletions via count comparison.
#[allow(clippy::too_many_arguments)]
async fn sync_condstore(
    config: &AppConfig,
    user_hash: &str,
    creds: &ImapCredentials,
    imap_client: &dyn ImapClient,
    folder_name: &str,
    cached_modseq: u64,
    status: &crate::imap::types::FolderStatusExtended,
    event_bus: &EventBus,
) -> Result<bool, String> {
    // Fetch only messages whose flags changed since our cached modseq.
    let (changed, new_modseq) = match imap_client
        .fetch_changed_flags(creds, folder_name, cached_modseq)
        .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::debug!(
                folder = %folder_name,
                error = %e,
                "CONDSTORE fetch failed, falling back to full sync"
            );
            return sync_full(
                config,
                user_hash,
                creds,
                imap_client,
                folder_name,
                status,
                event_bus,
            )
            .await;
        }
    };

    let database = config.user_db(user_hash).await?;
    let cached_count = db::messages::count_messages(&database, folder_name).await?;
    let imap_uids = if status.exists < cached_count {
        Some(
            imap_client
                .fetch_uids_and_flags(creds, folder_name)
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|(uid, _)| uid)
                .collect::<HashSet<_>>(),
        )
    } else {
        None
    };
    let transaction = database.begin_write().await?;
    let mut folder_changed = !changed.is_empty();
    for (uid, mut flags) in changed {
        flags.sort();
        db::messages::update_message_flags(&transaction, folder_name, uid, &flags.join(","))
            .await?;
    }
    if let Some(imap_uids) = imap_uids {
        for (uid, _) in db::messages::get_all_uids_and_flags(&transaction, folder_name).await? {
            if !imap_uids.contains(&uid) {
                db::messages::delete_message(&transaction, folder_name, uid).await?;
                folder_changed = true;
            }
        }
    }
    let max_uid = db::messages::max_uid(&transaction, folder_name).await?;
    let new_messages = status.uid_next > max_uid.saturating_add(1);
    let modseq = if new_modseq > 0 {
        new_modseq
    } else {
        status.highest_modseq
    };
    db::folders::update_folder_sync_status(
        &transaction,
        folder_name,
        status.uid_validity,
        status.exists,
        modseq,
    )
    .await?;
    if new_messages {
        db::folders::invalidate_folder_freshness(&transaction, folder_name).await?;
        folder_changed = true;
    }
    transaction.commit().await?;
    if new_messages {
        event_bus
            .publish(
                user_hash,
                MailEvent::NewMessages {
                    folder: folder_name.to_owned(),
                    count: 0,
                    latest_sender: None,
                    latest_subject: None,
                },
            )
            .await;
    }

    if folder_changed {
        event_bus
            .publish(
                user_hash,
                MailEvent::FlagsChanged {
                    folder: folder_name.to_string(),
                },
            )
            .await;
    }

    Ok(folder_changed)
}

/// Full fetch fallback: fetch all UIDs+FLAGS and reconcile (original behavior).
async fn sync_full(
    config: &AppConfig,
    user_hash: &str,
    creds: &ImapCredentials,
    imap_client: &dyn ImapClient,
    folder_name: &str,
    status: &crate::imap::types::FolderStatusExtended,
    event_bus: &EventBus,
) -> Result<bool, String> {
    let database = config.user_db(user_hash).await?;
    let cached = db::messages::get_all_uids_and_flags(&database, folder_name).await?;
    if cached.is_empty() {
        db::folders::update_folder_sync_status(
            &database,
            folder_name,
            status.uid_validity,
            status.exists,
            status.highest_modseq,
        )
        .await?;
        return Ok(false);
    }

    let imap_state = match imap_client.fetch_uids_and_flags(creds, folder_name).await {
        Ok(state) => state,
        Err(e) => {
            tracing::debug!(
                folder = %folder_name,
                error = %e,
                "Skipping folder sync (full fetch failed)"
            );
            return Ok(false);
        }
    };

    let transaction = database.begin_write().await?;
    let imap_map: HashMap<u32, String> = imap_state
        .into_iter()
        .map(|(uid, flags)| {
            let mut sorted = flags;
            sorted.sort();
            (uid, sorted.join(","))
        })
        .collect();

    let mut folder_changed = false;

    for (uid, cached_flags_csv) in &cached {
        match imap_map.get(uid) {
            None => {
                db::messages::delete_message(&transaction, folder_name, *uid).await?;
                folder_changed = true;
                tracing::debug!(
                    folder = %folder_name,
                    uid = uid,
                    "Removed deleted message from cache"
                );
            }
            Some(imap_flags_csv) => {
                let mut cached_sorted: Vec<&str> = cached_flags_csv.split(',').collect();
                cached_sorted.sort();
                let cached_normalized = cached_sorted.join(",");

                if cached_normalized != *imap_flags_csv {
                    db::messages::update_message_flags(
                        &transaction,
                        folder_name,
                        *uid,
                        imap_flags_csv,
                    )
                    .await?;
                    folder_changed = true;
                }
            }
        }
    }

    db::folders::update_folder_sync_status(
        &transaction,
        folder_name,
        status.uid_validity,
        status.exists,
        status.highest_modseq,
    )
    .await?;
    transaction.commit().await?;

    if folder_changed {
        event_bus
            .publish(
                user_hash,
                MailEvent::FlagsChanged {
                    folder: folder_name.to_string(),
                },
            )
            .await;
    }

    Ok(folder_changed)
}
