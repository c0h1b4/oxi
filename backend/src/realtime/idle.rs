use std::sync::Arc;

use dashmap::DashMap;
use tokio::task::JoinHandle;

use crate::imap::client::ImapCredentials;
use crate::realtime::events::{EventBus, MailEvent};

/// Manages long-lived IMAP IDLE connections, one per (user, folder) pair.
///
/// When a WebSocket client connects, the IdleManager starts IDLE for INBOX.
/// When the client disconnects, all IDLE tasks for that user are stopped.
pub struct IdleManager {
    /// Active IDLE tasks keyed by `(user_hash, folder_name)`.
    tasks: DashMap<(String, String), JoinHandle<()>>,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    /// Start an IDLE task for a specific user + folder.
    /// If one is already running, this is a no-op.
    pub async fn start_idle(
        &self,
        user_hash: String,
        folder: String,
        creds: ImapCredentials,
        event_bus: Arc<EventBus>,
    ) {
        let key = (user_hash.clone(), folder.clone());
        if self.tasks.contains_key(&key) {
            return;
        }

        let task_user_hash = user_hash.clone();
        let task_folder = folder.clone();

        let handle = tokio::spawn(async move {
            idle_loop(&task_user_hash, &task_folder, &creds, &event_bus).await;
        });

        self.tasks.insert(key, handle);
    }

    /// Stop the IDLE task for a specific user + folder.
    #[allow(dead_code)]
    pub async fn stop_idle(&self, user_hash: &str, folder: &str) {
        let key = (user_hash.to_string(), folder.to_string());
        if let Some((_, handle)) = self.tasks.remove(&key) {
            handle.abort();
        }
    }

    /// Stop all IDLE tasks for a user (called on WebSocket disconnect).
    pub async fn stop_all(&self, user_hash: &str) {
        let keys_to_remove: Vec<_> = self
            .tasks
            .iter()
            .filter(|entry| entry.key().0 == user_hash)
            .map(|entry| entry.key().clone())
            .collect();

        for key in keys_to_remove {
            if let Some((_, handle)) = self.tasks.remove(&key) {
                handle.abort();
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
) {
    let mut backoff = std::time::Duration::from_secs(1);
    let max_backoff = std::time::Duration::from_secs(60);

    loop {
        match run_idle_session(user_hash, folder, creds, event_bus).await {
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
) -> Result<(), String> {
    use crate::imap::client::connect;

    let mut session = connect(creds)
        .await
        .map_err(|e| format!("IDLE connect failed: {e}"))?;

    session
        .select(folder)
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
        let (idle_wait, _stop) = idle_handle.wait();

        // Wait for the server to send an unsolicited response, or timeout.
        let result = tokio::time::timeout(idle_timeout, idle_wait).await;

        // Send DONE to end IDLE and get the session back.
        session = idle_handle
            .done()
            .await
            .map_err(|e| format!("IDLE done failed: {e}"))?;

        match result {
            Ok(Ok(_response)) => {
                // Server sent an update — publish event and re-enter IDLE.
                tracing::debug!(
                    user_hash = %user_hash,
                    folder = %folder,
                    "IDLE received update from server"
                );
                event_bus
                    .publish(
                        user_hash,
                        MailEvent::NewMessages {
                            folder: folder.to_string(),
                        },
                    )
                    .await;
            }
            Ok(Err(e)) => {
                // IDLE protocol error.
                return Err(format!("IDLE error: {e}"));
            }
            Err(_) => {
                // Timeout — re-enter IDLE to keep the connection alive.
                tracing::debug!(
                    user_hash = %user_hash,
                    folder = %folder,
                    "IDLE timeout, re-entering IDLE"
                );
            }
        }
    }
}
