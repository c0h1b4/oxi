use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::db;
use crate::imap::client::{ImapClient, ImapCredentials};
use crate::realtime::events::{EventBus, MailEvent};

/// How often to run a sync check (seconds).
/// STATUS checks are cheap (no SELECT), so 30s is a tight safety net
/// for non-INBOX folders that don't have IDLE.
const SYNC_INTERVAL_SECS: u64 = 30;

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
) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(SYNC_INTERVAL_SECS));
    // Skip the first immediate tick — IDLE + initial list_messages handles that.
    interval.tick().await;

    loop {
        interval.tick().await;

        if let Err(e) = run_sync(&user_hash, &creds, &config, imap_client.as_ref(), &event_bus).await {
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
) -> Result<(), String> {
    // Collect folder metadata in a non-async block so `conn` is dropped before awaits.
    let folder_snapshots = {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        let cached_folders = db::folders::get_all_folders(&conn)
            .map_err(|e| format!("DB error: {e}"))?;
        cached_folders
            .into_iter()
            .map(|f| FolderSnapshot {
                name: f.name,
                uid_validity: f.uid_validity,
                highest_modseq: f.highest_modseq,
            })
            .collect::<Vec<_>>()
    };

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
            let conn = db::pool::open_user_db(&config.data_dir, user_hash)
                .map_err(|e| format!("DB error: {e}"))?;
            let _ = db::folders::invalidate_folder_freshness(&conn, folder_name);
            any_changes = true;
            event_bus
                .publish(user_hash, MailEvent::FolderUpdated)
                .await;
            continue;
        }

        // If CONDSTORE is supported (highest_modseq > 0) and matches cached → skip.
        let cached_modseq = folder.highest_modseq;
        if status.highest_modseq > 0 && cached_modseq > 0 && status.highest_modseq == cached_modseq {
            let conn = db::pool::open_user_db(&config.data_dir, user_hash)
                .map_err(|e| format!("DB error: {e}"))?;
            let cached_count = db::messages::count_messages(&conn, folder_name).unwrap_or(0);
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
                config, user_hash, creds, imap_client, folder_name, cached_modseq, &status, event_bus,
            ).await?;
            if folder_changed {
                any_changes = true;
            }
            continue;
        }

        // ── Tier 3: Full fetch fallback ──────────────────────────────
        let folder_changed = sync_full(
            config, user_hash, creds, imap_client, folder_name, &status, event_bus,
        ).await?;
        if folder_changed {
            any_changes = true;
        }
    }

    if any_changes {
        event_bus.publish(user_hash, MailEvent::FolderUpdated).await;
    }

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
    let mut folder_changed = false;

    // Fetch only messages whose flags changed since our cached modseq.
    let (changed, new_modseq) = match imap_client.fetch_changed_flags(creds, folder_name, cached_modseq).await {
        Ok(result) => result,
        Err(e) => {
            tracing::debug!(
                folder = %folder_name,
                error = %e,
                "CONDSTORE fetch failed, falling back to full sync"
            );
            return sync_full(config, user_hash, creds, imap_client, folder_name, status, event_bus).await;
        }
    };

    // Apply changed flags (DB operations in a non-async block).
    {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        for (uid, flags) in &changed {
            let mut sorted = flags.clone();
            sorted.sort();
            let flags_csv = sorted.join(",");
            let _ = db::messages::update_message_flags(&conn, folder_name, *uid, &flags_csv);
        }
        if !changed.is_empty() {
            folder_changed = true;
        }
    }

    // Detect deletions: if server count < cached count, some messages were removed.
    let cached_count = {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        db::messages::count_messages(&conn, folder_name).unwrap_or(0)
    };

    if status.exists < cached_count {
        // Need to fetch all UIDs to find which ones were deleted.
        if let Ok(imap_state) = imap_client.fetch_uids_and_flags(creds, folder_name).await {
            let conn = db::pool::open_user_db(&config.data_dir, user_hash)
                .map_err(|e| format!("DB error: {e}"))?;
            let imap_uids: HashSet<u32> = imap_state.iter().map(|(uid, _)| *uid).collect();
            let cached = db::messages::get_all_uids_and_flags(&conn, folder_name)
                .unwrap_or_default();
            for (uid, _) in &cached {
                if !imap_uids.contains(uid) {
                    let _ = db::messages::delete_message(&conn, folder_name, *uid);
                    folder_changed = true;
                    tracing::debug!(
                        folder = %folder_name,
                        uid = uid,
                        "Removed deleted message from cache (CONDSTORE path)"
                    );
                }
            }
        }
    }

    // Detect new messages.
    let max_cached_uid = {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        db::messages::max_uid(&conn, folder_name).unwrap_or(0)
    };

    if status.uid_next > max_cached_uid + 1 {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        let _ = db::folders::invalidate_folder_freshness(&conn, folder_name);
        folder_changed = true;
        event_bus
            .publish(
                user_hash,
                MailEvent::NewMessages {
                    folder: folder_name.to_string(),
                    count: 0,
                    latest_sender: None,
                    latest_subject: None,
                },
            )
            .await;
    }

    // Update stored modseq.
    let final_modseq = if new_modseq > 0 { new_modseq } else { status.highest_modseq };
    {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        let _ = db::folders::update_folder_sync_status(
            &conn, folder_name, status.uid_validity, status.exists, final_modseq,
        );
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
    let cached = {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        db::messages::get_all_uids_and_flags(&conn, folder_name)
            .map_err(|e| format!("DB error: {e}"))?
    };

    if cached.is_empty() {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;
        let _ = db::folders::update_folder_sync_status(
            &conn, folder_name, status.uid_validity, status.exists, status.highest_modseq,
        );
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

    let imap_map: HashMap<u32, String> = imap_state
        .into_iter()
        .map(|(uid, flags)| {
            let mut sorted = flags;
            sorted.sort();
            (uid, sorted.join(","))
        })
        .collect();

    let mut folder_changed = false;

    {
        let conn = db::pool::open_user_db(&config.data_dir, user_hash)
            .map_err(|e| format!("DB error: {e}"))?;

        for (uid, cached_flags_csv) in &cached {
            match imap_map.get(uid) {
                None => {
                    let _ = db::messages::delete_message(&conn, folder_name, *uid);
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
                        let _ = db::messages::update_message_flags(
                            &conn,
                            folder_name,
                            *uid,
                            imap_flags_csv,
                        );
                        folder_changed = true;
                    }
                }
            }
        }

        let _ = db::folders::update_folder_sync_status(
            &conn, folder_name, status.uid_validity, status.exists, status.highest_modseq,
        );
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
