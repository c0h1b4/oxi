use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use dashmap::DashMap;
use rand::RngCore;

/// A session identifier: a base64url-encoded string of 32 cryptographically
/// random bytes. Used as the session cookie value.
pub type SessionId = String;

/// In-memory state for a single authenticated session.
///
/// Holds the IMAP credentials so that every request can open a fresh IMAP
/// connection without asking the user to re-authenticate. Credentials are
/// never persisted to disk.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// The user's email address (also the IMAP username).
    pub email: String,
    /// The user's password (or app-specific password).
    #[allow(dead_code)]
    pub password: String,
    /// A SHA-256 hash that uniquely identifies the user. Used for
    /// per-user SQLite caching.
    #[allow(dead_code)]
    pub user_hash: String,
    /// Monotonic timestamp of the last time this session was accessed.
    /// Updated on every successful `get` to implement sliding-window expiry.
    pub last_accessed: Instant,
}

/// Thread-safe, in-memory session store backed by `DashMap`.
///
/// Shared across all Axum handlers via `Arc`. Sessions expire after
/// `timeout` of inactivity (sliding window).
#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions: Arc<DashMap<SessionId, SessionState>>,
    timeout: Duration,
}

impl SessionStore {
    /// Create a new, empty session store with the given inactivity timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            timeout,
        }
    }

    /// Generate a cryptographically random session token.
    ///
    /// Produces 32 bytes of randomness from the OS CSPRNG, then encodes
    /// them as a base64url string (no padding). The result is 43 characters.
    pub fn generate_token() -> SessionId {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Create a new session for the given credentials and return its token.
    ///
    /// The session's `last_accessed` timestamp is set to `Instant::now()`.
    pub fn insert(&self, email: String, password: String, user_hash: String) -> SessionId {
        let token = Self::generate_token();
        let state = SessionState {
            email,
            password,
            user_hash,
            last_accessed: Instant::now(),
        };
        self.sessions.insert(token.clone(), state);
        token
    }

    /// Look up a session by its token.
    ///
    /// If the session exists but has expired (inactive longer than `timeout`),
    /// it is removed and `None` is returned. If it is still valid, the
    /// `last_accessed` timestamp is refreshed (sliding window) and a clone
    /// of the session state is returned.
    pub fn get(&self, token: &str) -> Option<SessionState> {
        let mut entry = self.sessions.get_mut(token)?;
        let now = Instant::now();
        if now.duration_since(entry.last_accessed) > self.timeout {
            // Drop the mutable reference before removing so we don't
            // deadlock on the same shard.
            drop(entry);
            self.sessions.remove(token);
            return None;
        }
        entry.last_accessed = now;
        Some(entry.clone())
    }

    /// Remove a session by its token. Returns `true` if the session existed.
    pub fn remove(&self, token: &str) -> bool {
        self.sessions.remove(token).is_some()
    }

    /// Remove all sessions that have been inactive longer than the timeout.
    #[allow(dead_code)]
    pub fn purge_expired(&self) {
        let now = Instant::now();
        self.sessions
            .retain(|_, state| now.duration_since(state.last_accessed) <= self.timeout);
    }

    /// Return the number of sessions currently stored (including expired
    /// ones that have not yet been purged).
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Return `true` if the store contains no sessions.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::thread;

    /// Helper: create a store with a generous timeout so tests don't flake.
    fn long_lived_store() -> SessionStore {
        SessionStore::new(Duration::from_secs(3600))
    }

    /// Helper: create a store with a tiny timeout for expiry tests.
    fn short_lived_store() -> SessionStore {
        SessionStore::new(Duration::from_millis(50))
    }

    #[test]
    fn generate_token_produces_unique_values() {
        let mut tokens = HashSet::new();
        for _ in 0..1000 {
            let t = SessionStore::generate_token();
            assert!(tokens.insert(t), "duplicate token generated");
        }
    }

    #[test]
    fn generate_token_length_is_correct() {
        // 32 bytes -> base64url without padding = ceil(32*4/3) = 43 chars
        let token = SessionStore::generate_token();
        assert_eq!(token.len(), 43);
    }

    #[test]
    fn generate_token_is_valid_base64url() {
        let token = SessionStore::generate_token();
        let decoded = URL_SAFE_NO_PAD.decode(&token);
        assert!(decoded.is_ok(), "token is not valid base64url");
        assert_eq!(decoded.unwrap().len(), 32);
    }

    #[test]
    fn insert_and_get_returns_correct_state() {
        let store = long_lived_store();
        let token = store.insert(
            "alice@example.com".into(),
            "hunter2".into(),
            "abc123".into(),
        );

        let state = store.get(&token).expect("session should exist");
        assert_eq!(state.email, "alice@example.com");
        assert_eq!(state.password, "hunter2");
        assert_eq!(state.user_hash, "abc123");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = long_lived_store();
        assert!(store.get("no-such-token").is_none());
    }

    #[test]
    fn remove_existing_session_returns_true() {
        let store = long_lived_store();
        let token = store.insert("bob@test.com".into(), "pass".into(), "hash".into());

        assert!(store.remove(&token));
        assert!(store.get(&token).is_none());
    }

    #[test]
    fn remove_nonexistent_session_returns_false() {
        let store = long_lived_store();
        assert!(!store.remove("ghost"));
    }

    #[test]
    fn expired_session_returns_none_on_get() {
        let store = short_lived_store();
        let token = store.insert("eve@test.com".into(), "pass".into(), "hash".into());

        // Wait for the session to expire.
        thread::sleep(Duration::from_millis(100));

        assert!(
            store.get(&token).is_none(),
            "expired session should not be returned"
        );
        // The expired entry should also have been removed from the map.
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn purge_expired_removes_stale_sessions() {
        let store = short_lived_store();
        let _t1 = store.insert("a@test.com".into(), "p".into(), "h".into());
        let _t2 = store.insert("b@test.com".into(), "p".into(), "h".into());
        assert_eq!(store.len(), 2);

        thread::sleep(Duration::from_millis(100));
        store.purge_expired();

        assert_eq!(store.len(), 0);
    }

    #[test]
    fn purge_expired_keeps_active_sessions() {
        let store = SessionStore::new(Duration::from_secs(3600));
        let _t1 = store.insert("alive@test.com".into(), "p".into(), "h".into());

        store.purge_expired();

        assert_eq!(store.len(), 1);
    }

    #[test]
    fn sliding_window_refreshes_expiry() {
        // Use a 150ms timeout. Access the session at 80ms to refresh it.
        // At 160ms the session should still be alive because the sliding
        // window was refreshed.
        let store = SessionStore::new(Duration::from_millis(150));
        let token = store.insert("slide@test.com".into(), "p".into(), "h".into());

        thread::sleep(Duration::from_millis(80));
        assert!(store.get(&token).is_some(), "session should still be alive");

        thread::sleep(Duration::from_millis(80));
        assert!(
            store.get(&token).is_some(),
            "session should be alive after sliding refresh"
        );
    }

    #[test]
    fn concurrent_sessions_are_isolated() {
        let store = long_lived_store();
        let t1 = store.insert("user1@test.com".into(), "pass1".into(), "hash1".into());
        let t2 = store.insert("user2@test.com".into(), "pass2".into(), "hash2".into());

        // Each token resolves to its own session.
        let s1 = store.get(&t1).unwrap();
        let s2 = store.get(&t2).unwrap();
        assert_eq!(s1.email, "user1@test.com");
        assert_eq!(s2.email, "user2@test.com");

        // Removing one does not affect the other.
        store.remove(&t1);
        assert!(store.get(&t1).is_none());
        assert!(store.get(&t2).is_some());
    }

    #[test]
    fn len_and_is_empty() {
        let store = long_lived_store();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        let t = store.insert("x@test.com".into(), "p".into(), "h".into());
        assert!(!store.is_empty());
        assert_eq!(store.len(), 1);

        store.remove(&t);
        assert!(store.is_empty());
    }

    #[test]
    fn concurrent_access_from_multiple_threads() {
        let store = long_lived_store();
        let store_arc = Arc::new(store);
        let mut handles = vec![];

        // Spawn 10 threads, each inserting and retrieving its own session.
        for i in 0..10 {
            let s = Arc::clone(&store_arc);
            handles.push(thread::spawn(move || {
                let email = format!("thread{}@test.com", i);
                let token = s.insert(email.clone(), "pass".into(), "hash".into());
                let state = s.get(&token).expect("should find own session");
                assert_eq!(state.email, email);
                token
            }));
        }

        let tokens: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All sessions should still exist.
        assert_eq!(store_arc.len(), 10);

        // All tokens should be unique.
        let unique: HashSet<_> = tokens.into_iter().collect();
        assert_eq!(unique.len(), 10);
    }
}
