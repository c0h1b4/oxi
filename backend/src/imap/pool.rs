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
    slots: Arc<Semaphore>,
    idle: Mutex<Vec<(Instant, Session)>>,
}

// Length-prefix fields to avoid ambiguous identities. Never retain plaintext passwords.
fn identity(creds: &ImapCredentials) -> [u8; 32] {
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
        let user = self
            .inner
            .users
            .lock()
            .unwrap()
            .entry(identity(creds))
            .or_insert_with(|| {
                Arc::new(UserPool {
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
                return Ok(Lease {
                    session: Some(session),
                    user,
                    _permit: permit,
                });
            }
        }
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
                    loop {
                        line.clear();
                        if socket.read_line(&mut line).await.unwrap_or(0) == 0 {
                            break;
                        }
                        let tag = line.split_whitespace().next().unwrap();
                        if line.contains(" NOOP") && kill.swap(false, Ordering::SeqCst) {
                            break;
                        }
                        let response = if line.contains(" LOGIN ") {
                            count.fetch_add(1, Ordering::SeqCst);
                            format!("{tag} OK authenticated\r\n")
                        } else if line.contains(" LIST ") {
                            format!("* LIST () \"/\" \"INBOX\"\r\n{tag} OK listed\r\n")
                        } else if line.contains(" SELECT ") {
                            format!("{tag} NO missing folder\r\n")
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
}
