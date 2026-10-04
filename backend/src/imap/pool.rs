//! Request-path sessions only; IDLE owns a separate connection.
use super::{
    connection::{ImapStream, connect},
    error::ImapError,
    types::ImapCredentials,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

type Session = async_imap::Session<ImapStream>;
const MAX_SESSIONS: usize = 3;
const IDLE_TTL: Duration = Duration::from_secs(120);
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(crate) struct ConnectionPool {
    inner: Arc<PoolInner>,
}
#[derive(Default)]
struct PoolInner {
    users: Mutex<HashMap<[u8; 32], Arc<UserPool>>>,
    reaper_started: AtomicBool,
}
struct UserPool {
    identity_prefix: u32,
    slots: Arc<Semaphore>,
    idle: Mutex<Vec<(Instant, Session)>>,
}

// Length-prefix fields to avoid ambiguous identities. Never retain plaintext passwords.
pub(super) fn identity(creds: &ImapCredentials) -> [u8; 32] {
    let mut hash = Sha256::new();
    for field in [
        creds.host.as_bytes(),
        &creds.port.to_be_bytes(),
        &[u8::from(creds.tls)],
        creds.email.as_bytes(),
        creds.password.as_bytes(),
    ] {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

// async-imap 0.10's noop parser accepts EOF without a tagged completion.
// Require the matching tagged OK ourselves so dead sockets cannot pass validation.
async fn validate(session: &mut Session) -> Result<(), ImapError> {
    use async_imap::imap_proto::{Response, Status};
    let id = session
        .run_command("NOOP")
        .await
        .map_err(super::connection::map_imap_error)?;
    while let Some(response) = session.read_response().await {
        let response = response.map_err(|e| ImapError::ConnectionFailed(e.to_string()))?;
        match response.parsed() {
            Response::Done { tag, status, .. } if *tag == id => {
                return if *status == Status::Ok {
                    Ok(())
                } else {
                    Err(ImapError::ProtocolError("NOOP rejected".into()))
                };
            }
            Response::Data {
                status: Status::Bye,
                ..
            } => break,
            _ => {}
        }
    }
    Err(ImapError::ConnectionFailed(
        "connection lost during NOOP".into(),
    ))
}

impl PoolInner {
    fn evict(&self) {
        self.users.lock().unwrap().retain(|_, user| {
            let mut idle = user.idle.lock().unwrap();
            // Dropping a session closes its socket; never CLOSE (which expunges).
            idle.retain(|(returned, _)| returned.elapsed() < IDLE_TTL);
            !idle.is_empty() || Arc::strong_count(user) > 1
        });
    }
}

impl ConnectionPool {
    pub(crate) async fn checkout(&self, creds: &ImapCredentials) -> Result<Lease, ImapError> {
        if !self.inner.reaper_started.swap(true, Ordering::Relaxed) {
            let weak = Arc::downgrade(&self.inner);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    let Some(inner) = weak.upgrade() else { break };
                    inner.evict();
                }
            });
        }
        let key = identity(creds);
        let user = self
            .inner
            .users
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| {
                Arc::new(UserPool {
                    identity_prefix: u32::from_be_bytes(key[..4].try_into().unwrap()),
                    slots: Arc::new(Semaphore::new(MAX_SESSIONS)),
                    idle: Mutex::new(Vec::new()),
                })
            })
            .clone();
        let permit = tokio::time::timeout(CHECK_TIMEOUT, user.slots.clone().acquire_owned())
            .await
            .map_err(|_| ImapError::ConnectionFailed("pool checkout timed out".into()))?
            .map_err(|_| ImapError::ConnectionFailed("pool closed".into()))?;
        loop {
            let candidate = user.idle.lock().unwrap().pop();
            let Some((returned, mut session)) = candidate else {
                break;
            };
            if returned.elapsed() < IDLE_TTL
                && matches!(
                    tokio::time::timeout(CHECK_TIMEOUT, validate(&mut session)).await,
                    Ok(Ok(()))
                )
            {
                tracing::debug!(
                    pool_identity = format_args!("{:08x}", user.identity_prefix),
                    action = "reuse",
                    "IMAP pool checkout"
                );
                return Ok(Lease {
                    session: Some(session),
                    user,
                    _permit: permit,
                });
            }
            tracing::debug!(
                pool_identity = format_args!("{:08x}", user.identity_prefix),
                action = "discard",
                "IMAP pool validation failed or session expired"
            );
        }
        tracing::debug!(
            pool_identity = format_args!("{:08x}", user.identity_prefix),
            action = "connect",
            "IMAP pool checkout"
        );
        // Bound TCP, TLS and LOGIN together, retaining connect's TCP timeout too.
        let session = tokio::time::timeout(CHECK_TIMEOUT, connect(creds))
            .await
            .map_err(|_| ImapError::ConnectionFailed("connection timed out".into()))??;
        Ok(Lease {
            session: Some(session),
            user,
            _permit: permit,
        })
    }
}

/// Defaults to discard, including when a request is cancelled mid-command.
/// Only explicitly completed operations may return the session for reuse.
pub(crate) struct Lease {
    session: Option<Session>,
    user: Arc<UserPool>,
    _permit: OwnedSemaphorePermit,
}
impl Lease {
    pub(crate) fn recycle(mut self) {
        tracing::debug!(
            pool_identity = format_args!("{:08x}", self.user.identity_prefix),
            action = "recycle",
            "IMAP pool return"
        );
        self.user
            .idle
            .lock()
            .unwrap()
            .push((Instant::now(), self.session.take().unwrap()));
    }
}
impl Deref for Lease {
    type Target = Session;
    fn deref(&self) -> &Session {
        self.session.as_ref().unwrap()
    }
}
impl DerefMut for Lease {
    fn deref_mut(&mut self) -> &mut Session {
        self.session.as_mut().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imap::client::{ImapClient, RealImapClient};
    use std::sync::atomic::AtomicUsize;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    async fn server() -> (
        ImapCredentials,
        Arc<AtomicUsize>,
        Arc<AtomicBool>,
        tokio::task::JoinHandle<()>,
    ) {
        server_with_quota("{tag} OK no quota configured\r\n").await
    }

    async fn server_with_quota(
        quota_reply: &'static str,
    ) -> (
        ImapCredentials,
        Arc<AtomicUsize>,
        Arc<AtomicBool>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let creds = ImapCredentials {
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port(),
            tls: false,
            email: "test@example.com".into(),
            password: "secret".into(),
        };
        let logins = Arc::new(AtomicUsize::new(0));
        let count = logins.clone();
        let kill_noop = Arc::new(AtomicBool::new(false));
        let kill = kill_noop.clone();
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let count = count.clone();
                let kill = kill.clone();
                tokio::spawn(async move {
                    let mut socket = BufReader::new(socket);
                    socket
                        .get_mut()
                        .write_all(b"* OK test server\r\n")
                        .await
                        .unwrap();
                    let mut line = String::new();
                    let mut selected = String::new();
                    loop {
                        line.clear();
                        if socket.read_line(&mut line).await.unwrap_or(0) == 0 {
                            break;
                        }
                        let tag = line.split_whitespace().next().unwrap();
                        if line.contains(" NOOP") && kill.swap(false, Ordering::SeqCst) {
                            break;
                        }
                        if line.contains(" GETQUOTAROOT ") && quota_reply.is_empty() {
                            break;
                        }
                        if line.contains(" UID FETCH ") && selected == "stalled" {
                            // No completion. The caller must discard on cancellation/timeout.
                            continue;
                        }
                        let response = if line.contains(" GETQUOTAROOT ") {
                            quota_reply.replace("{tag}", tag)
                        } else if line.contains(" UID FETCH ") && selected == "broken" {
                            break;
                        } else if line.contains(" UID FETCH ") && selected == "rejected" {
                            format!("{tag} NO fetch rejected\r\n")
                        } else if line.contains(" UID FETCH ") {
                            format!("* 1 FETCH (UID 1 RFC822.SIZE 42)\r\n{tag} OK fetched\r\n")
                        } else if line.contains(" LOGIN ") {
                            count.fetch_add(1, Ordering::SeqCst);
                            format!("{tag} OK authenticated\r\n")
                        } else if line.contains(" LIST ") {
                            format!("* LIST () \"/\" \"INBOX\"\r\n{tag} OK listed\r\n")
                        } else if line.contains(" SELECT ") && line.contains("missing") {
                            format!("{tag} NO missing folder\r\n")
                        } else if line.contains(" SELECT ") {
                            selected = line
                                .split_whitespace()
                                .nth(2)
                                .unwrap()
                                .trim_matches('"')
                                .to_string();
                            let exists = if selected == "INBOX" || selected == "Archive" {
                                0
                            } else {
                                1
                            };
                            format!(
                                "* {exists} EXISTS\r\n* OK [UIDVALIDITY 1] stable\r\n* OK [UIDNEXT 1] next\r\n{tag} OK selected\r\n"
                            )
                        } else {
                            format!("{tag} OK done\r\n")
                        };
                        if socket
                            .get_mut()
                            .write_all(response.as_bytes())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
        });
        (creds, logins, kill_noop, task)
    }

    #[tokio::test]
    async fn read_operations_reuse_login_and_errors_discard() {
        let (creds, logins, _, task) = server().await;
        let client = RealImapClient::default();
        for _ in 0..5 {
            assert_eq!(client.list_folders(&creds).await.unwrap()[0].name, "INBOX");
        }
        assert_eq!(logins.load(Ordering::SeqCst), 1);
        assert!(client.folder_status(&creds, "missing").await.is_err());
        client.list_folders(&creds).await.unwrap();
        assert_eq!(logins.load(Ordering::SeqCst), 2);
        task.abort();
    }

    #[tokio::test]
    async fn bounds_exclusive_leases_and_discards_cancelled_work() {
        let (creds, logins, _, task) = server().await;
        let pool = ConnectionPool::default();
        let mut leases = Vec::new();
        for _ in 0..MAX_SESSIONS {
            leases.push(pool.checkout(&creds).await.unwrap());
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(50), pool.checkout(&creds))
                .await
                .is_err()
        );
        assert_eq!(logins.load(Ordering::SeqCst), MAX_SESSIONS);
        drop(leases.pop()); // Unfinished work must not be reused.
        let lease = pool.checkout(&creds).await.unwrap();
        assert_eq!(logins.load(Ordering::SeqCst), MAX_SESSIONS + 1);
        lease.recycle();
        let reused = pool.checkout(&creds).await.unwrap();
        assert_eq!(logins.load(Ordering::SeqCst), MAX_SESSIONS + 1);
        drop(reused);
        task.abort();
    }

    #[tokio::test]
    async fn stale_sessions_and_credential_identities_are_isolated() {
        let (creds, logins, _, task) = server().await;
        let pool = ConnectionPool::default();
        pool.checkout(&creds).await.unwrap().recycle();
        let mut changed = creds.clone();
        changed.password = "different".into();
        pool.checkout(&changed).await.unwrap().recycle();
        assert_eq!(logins.load(Ordering::SeqCst), 2);
        {
            let users = pool.inner.users.lock().unwrap();
            for user in users.values() {
                for (returned, _) in user.idle.lock().unwrap().iter_mut() {
                    *returned = Instant::now() - IDLE_TTL;
                }
            }
        }
        pool.inner.evict();
        assert!(pool.inner.users.lock().unwrap().is_empty());
        pool.checkout(&creds).await.unwrap().recycle();
        assert_eq!(logins.load(Ordering::SeqCst), 3);
        task.abort();
    }
    #[tokio::test]
    async fn dead_session_is_replaced_before_operation() {
        let (creds, logins, kill_noop, task) = server().await;
        let client = RealImapClient::default();
        client.list_folders(&creds).await.unwrap();
        kill_noop.store(true, Ordering::SeqCst);
        client.list_folders(&creds).await.unwrap();
        assert_eq!(logins.load(Ordering::SeqCst), 2);
        task.abort();
    }
    #[tokio::test]
    #[ignore = "requires TEST_IMAP_HOST, TEST_IMAP_EMAIL and TEST_IMAP_PASSWORD"]
    async fn real_tls_reads_reuse_one_login() {
        let creds = ImapCredentials {
            host: std::env::var("TEST_IMAP_HOST").expect("TEST_IMAP_HOST required"),
            port: 993,
            tls: true,
            email: std::env::var("TEST_IMAP_EMAIL").expect("TEST_IMAP_EMAIL required"),
            password: std::env::var("TEST_IMAP_PASSWORD").expect("TEST_IMAP_PASSWORD required"),
        };
        let client = RealImapClient::default();
        let before = super::super::connection::connection_count(&creds);
        for _ in 0..3 {
            client.list_folders(&creds).await.unwrap();
            client.folder_status(&creds, "INBOX").await.unwrap();
            client.fetch_headers(&creds, "INBOX", "1:1").await.unwrap();
            let quota = client.get_quota(&creds).await.unwrap();
            println!("Server returned quota: {}", quota.is_some());
            client.fetch_folder_size(&creds, "INBOX").await.unwrap();
        }
        let connections = super::super::connection::connection_count(&creds) - before;
        println!("15 TLS read operations established {connections} connections");
        assert_eq!(connections, 1);
    }

    #[tokio::test]
    async fn quota_fallback_reads_reuse_the_request_pool() {
        let (creds, logins, _, task) = server().await;
        let client = RealImapClient::default();
        for _ in 0..5 {
            client.list_folders(&creds).await.unwrap();
            assert!(client.get_quota(&creds).await.unwrap().is_none());
            // A server without a configured quota triggers a size read per folder.
            assert_eq!(client.fetch_folder_size(&creds, "INBOX").await.unwrap(), 0);
            assert_eq!(
                client.fetch_folder_size(&creds, "Archive").await.unwrap(),
                0
            );
        }
        assert_eq!(logins.load(Ordering::SeqCst), 1);
        task.abort();
    }

    #[tokio::test]
    async fn quota_payload_is_consumed_before_reuse() {
        let (creds, logins, _, task) = server_with_quota(
            "* QUOTAROOT INBOX root\r\n* QUOTA root (STORAGE 10 100)\r\n{tag} OK quota\r\n",
        )
        .await;
        let client = RealImapClient::default();
        for _ in 0..3 {
            let quota = client.get_quota(&creds).await.unwrap().unwrap();
            assert_eq!(quota.usage_bytes, 10 * 1024);
            assert_eq!(quota.limit_bytes, 100 * 1024);
            client.list_folders(&creds).await.unwrap();
        }
        assert_eq!(logins.load(Ordering::SeqCst), 1);
        task.abort();
    }

    #[tokio::test]
    async fn rejected_or_interrupted_quota_discards_session() {
        for reply in [
            "{tag} NO unsupported\r\n",
            "{tag} BAD unsupported\r\n",
            "* BYE closing\r\n",
            "",
        ] {
            let (creds, logins, _, task) = server_with_quota(reply).await;
            let client = RealImapClient::default();
            let result = client.get_quota(&creds).await;
            if reply.contains("unsupported") {
                assert!(result.unwrap().is_none());
            } else {
                assert!(result.is_err());
            }
            client.list_folders(&creds).await.unwrap();
            assert_eq!(logins.load(Ordering::SeqCst), 2);
            task.abort();
        }
    }

    #[tokio::test]
    async fn folder_size_completion_reuses_and_incomplete_work_discards() {
        let (creds, logins, _, task) = server().await;
        let client = RealImapClient::default();
        for _ in 0..3 {
            assert_eq!(client.fetch_folder_size(&creds, "Full").await.unwrap(), 42);
        }
        assert_eq!(logins.load(Ordering::SeqCst), 1);
        for folder in ["rejected", "broken", "stalled"] {
            let result = tokio::time::timeout(
                Duration::from_millis(200),
                client.fetch_folder_size(&creds, folder),
            )
            .await;
            assert!(!matches!(result, Ok(Ok(_))));
            client.list_folders(&creds).await.unwrap();
        }
        assert_eq!(logins.load(Ordering::SeqCst), 4);
        task.abort();
    }
}
