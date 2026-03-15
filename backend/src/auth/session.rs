use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use dashmap::DashMap;
use rand::RngCore;

pub type SessionId = String;
pub type BrowserId = String;
pub type AccountId = String;

pub type SessionState = AccountSession;

#[derive(Debug, Clone)]
pub struct AccountSession {
    pub account_id: AccountId,
    pub email: String,
    #[allow(dead_code)]
    pub password: String,
    #[allow(dead_code)]
    pub user_hash: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: bool,
    pub smtp_host: String,
    #[allow(dead_code)]
    pub smtp_port: u16,
    #[allow(dead_code)]
    pub smtp_tls: bool,
    pub last_accessed: Instant,
    pub timeout_override: Option<Duration>,
}

/// Thread-safe, in-memory session store backed by `DashMap`.
///
/// Shared across all Axum handlers via `Arc`. Sessions expire after
/// `timeout` of inactivity (sliding window).
///
/// Browser binding allows multiple accounts to be associated with a single
/// browser session, enabling account switching without re-authentication.
#[derive(Debug, Clone)]
pub struct SessionStore {
    sessions: Arc<DashMap<SessionId, AccountSession>>,
    browsers: Arc<DashMap<BrowserId, Vec<AccountId>>>,
    account_to_session: Arc<DashMap<AccountId, SessionId>>,
    account_to_browser: Arc<DashMap<AccountId, BrowserId>>,
    timeout: Duration,
}

impl SessionStore {
    pub fn new(timeout: Duration) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            browsers: Arc::new(DashMap::new()),
            account_to_session: Arc::new(DashMap::new()),
            account_to_browser: Arc::new(DashMap::new()),
            timeout,
        }
    }

    pub fn generate_token() -> SessionId {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    fn generate_id() -> String {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    pub fn create_browser(&self) -> BrowserId {
        let browser_id = Self::generate_id();
        self.browsers.insert(browser_id.clone(), Vec::new());
        browser_id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_account_to_browser(
        &self,
        browser_id: &str,
        email: String,
        password: String,
        user_hash: String,
        imap_host: String,
        imap_port: u16,
        imap_tls: bool,
        smtp_host: String,
        smtp_port: u16,
        smtp_tls: bool,
    ) -> (SessionId, AccountId) {
        let token = Self::generate_token();
        let account_id = Self::generate_id();

        let session = AccountSession {
            account_id: account_id.clone(),
            email,
            password,
            user_hash,
            imap_host,
            imap_port,
            imap_tls,
            smtp_host,
            smtp_port,
            smtp_tls,
            last_accessed: Instant::now(),
            timeout_override: None,
        };

        // Insert into sessions first (independent operation).
        self.sessions.insert(token.clone(), session);

        // Insert into account_to_session; rollback sessions on panic.
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.account_to_session
                .insert(account_id.clone(), token.clone());
        })) {
            self.sessions.remove(&token);
            std::panic::resume_unwind(payload);
        }

        // Insert into account_to_browser; rollback prior maps on panic.
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.account_to_browser
                .insert(account_id.clone(), browser_id.to_string());
        })) {
            self.account_to_session.remove(&account_id);
            self.sessions.remove(&token);
            std::panic::resume_unwind(payload);
        }

        // Insert into browsers; rollback all prior maps on panic.
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.browsers
                .entry(browser_id.to_string())
                .or_default()
                .push(account_id.clone());
        })) {
            self.account_to_browser.remove(&account_id);
            self.account_to_session.remove(&account_id);
            self.sessions.remove(&token);
            std::panic::resume_unwind(payload);
        }

        (token, account_id)
    }

    pub fn get_browser_accounts(&self, browser_id: &str) -> Vec<AccountSession> {
        let account_ids = self
            .browsers
            .get(browser_id)
            .map(|entry| entry.clone())
            .unwrap_or_default();

        account_ids
            .into_iter()
            .filter_map(|account_id| {
                let session_id = self.account_to_session.get(&account_id)?;
                self.sessions.get(session_id.value()).map(|s| s.clone())
            })
            .collect()
    }

    pub fn browser_exists(&self, browser_id: &str) -> bool {
        self.browsers.contains_key(browser_id)
    }

    pub fn get_account_session(
        &self,
        browser_id: &str,
        account_id: &str,
        token: &str,
    ) -> Option<AccountSession> {
        let accounts = self.browsers.get(browser_id)?;
        if !accounts.contains(&account_id.to_string()) {
            return None;
        }
        drop(accounts);

        let session_id = self.account_to_session.get(account_id)?;
        if session_id.value() != token {
            return None;
        }
        drop(session_id);

        self.get(token)
    }

    pub fn remove_account(&self, account_id: &str) -> bool {
        if let Some((removed_account_id, session_id)) = self.account_to_session.remove(account_id) {
            // Remove from sessions; restore account_to_session on panic.
            if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.sessions.remove(&session_id);
            })) {
                self.account_to_session
                    .insert(removed_account_id, session_id);
                std::panic::resume_unwind(payload);
            }

            // Use reverse index for O(1) browser lookup instead of iterating.
            if let Some((_, browser_id)) = self.account_to_browser.remove(account_id)
                && let Some(mut entry) = self.browsers.get_mut(&browser_id)
                && let Some(pos) = entry.value().iter().position(|id| id == account_id)
            {
                entry.value_mut().remove(pos);
            }
            true
        } else {
            false
        }
    }

    pub fn remove_browser(&self, browser_id: &str) {
        if let Some((_, account_ids)) = self.browsers.remove(browser_id) {
            // Clean up each account. Individual failures are tolerated --
            // the browser entry is already gone, so the accounts become
            // unreachable via browser lookups. They will be reaped by
            // purge_expired or a subsequent remove_account call.
            for account_id in account_ids {
                self.account_to_browser.remove(&account_id);
                if let Some((_, session_id)) = self.account_to_session.remove(&account_id) {
                    self.sessions.remove(&session_id);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn insert(
        &self,
        email: String,
        password: String,
        user_hash: String,
        timeout_override: Option<Duration>,
    ) -> SessionId {
        let token = Self::generate_token();
        let account_id = Self::generate_id();
        let state = AccountSession {
            account_id,
            email,
            password,
            user_hash,
            imap_host: String::new(),
            imap_port: 993,
            imap_tls: true,
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_tls: true,
            last_accessed: Instant::now(),
            timeout_override,
        };
        self.sessions.insert(token.clone(), state);
        token
    }

    pub fn get(&self, token: &str) -> Option<AccountSession> {
        let mut entry = self.sessions.get_mut(token)?;
        let now = Instant::now();
        let effective_timeout = entry.timeout_override.unwrap_or(self.timeout);
        if now.duration_since(entry.last_accessed) > effective_timeout {
            let account_id = entry.account_id.clone();
            drop(entry);
            self.sessions.remove(token);
            self.evict_account(&account_id);
            return None;
        }
        entry.last_accessed = now;
        Some(entry.clone())
    }

    #[allow(dead_code)]
    pub fn remove(&self, token: &str) -> bool {
        self.sessions.remove(token).is_some()
    }

    /// Remove an account's entries from `account_to_session` and `browsers`.
    /// If a browser's account list becomes empty, the browser entry is removed
    /// too -- keeping it would serve no purpose and would itself be a leak.
    fn evict_account(&self, account_id: &str) {
        self.account_to_session.remove(account_id);

        // We cannot predict which browser owns this account, so scan all.
        self.browsers.retain(|_, accounts| {
            accounts.retain(|id| id != account_id);
            // Keep the browser entry only if it still has accounts.
            !accounts.is_empty()
        });
    }

    #[allow(dead_code)]
    pub fn purge_expired(&self) {
        let now = Instant::now();

        // Collect expired account IDs while retaining live sessions.
        let mut expired_accounts = Vec::new();
        self.sessions.retain(|_, state| {
            let effective_timeout = state.timeout_override.unwrap_or(self.timeout);
            let alive = now.duration_since(state.last_accessed) <= effective_timeout;
            if !alive {
                expired_accounts.push(state.account_id.clone());
            }
            alive
        });

        for account_id in &expired_accounts {
            self.account_to_browser.remove(account_id);
            self.evict_account(account_id);
        }
    }

    /// Verify internal consistency across all four maps. Returns a list of
    /// inconsistency descriptions. An empty vec means the store is consistent.
    /// Intended for testing and diagnostics.
    #[allow(dead_code)]
    pub fn check_consistency(&self) -> Vec<String> {
        let mut issues = Vec::new();

        // Every session's account_id should have a matching account_to_session entry.
        for entry in self.sessions.iter() {
            let token = entry.key();
            let account_id = &entry.value().account_id;
            match self.account_to_session.get(account_id) {
                Some(mapped_token) if mapped_token.value() == token => {}
                Some(mapped_token) => {
                    issues.push(format!(
                        "account_to_session[{account_id}] points to {} but session token is {token}",
                        mapped_token.value()
                    ));
                }
                None => {
                    issues.push(format!(
                        "session {token} references account {account_id} with no account_to_session entry"
                    ));
                }
            }
        }

        // Every account_to_session entry should point to an existing session.
        for entry in self.account_to_session.iter() {
            let account_id = entry.key();
            let session_id = entry.value();
            if !self.sessions.contains_key(session_id) {
                issues.push(format!(
                    "account_to_session[{account_id}] references missing session {session_id}"
                ));
            }
        }

        // Every account_id in a browser list should exist in account_to_session.
        for entry in self.browsers.iter() {
            let browser_id = entry.key();
            for account_id in entry.value().iter() {
                if !self.account_to_session.contains_key(account_id) {
                    issues.push(format!(
                        "browser {browser_id} references account {account_id} missing from account_to_session"
                    ));
                }
            }
        }

        issues
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

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

    fn long_lived_store() -> SessionStore {
        SessionStore::new(Duration::from_secs(3600))
    }

    fn short_lived_store() -> SessionStore {
        SessionStore::new(Duration::from_millis(50))
    }

    fn create_test_session(email: &str, password: &str, user_hash: &str) -> AccountSession {
        AccountSession {
            account_id: SessionStore::generate_id(),
            email: email.to_string(),
            password: password.to_string(),
            user_hash: user_hash.to_string(),
            imap_host: "imap.test.com".to_string(),
            imap_port: 993,
            imap_tls: true,
            smtp_host: "smtp.test.com".to_string(),
            smtp_port: 587,
            smtp_tls: true,
            last_accessed: Instant::now(),
            timeout_override: None,
        }
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
    fn generate_id_produces_22_chars() {
        let id = SessionStore::generate_id();
        assert_eq!(id.len(), 22);
    }

    #[test]
    fn create_browser_returns_valid_id() {
        let store = long_lived_store();
        let browser_id = store.create_browser();
        assert_eq!(browser_id.len(), 22);
        assert!(store.browsers.contains_key(&browser_id));
    }

    #[test]
    fn add_account_to_browser() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            "alice@example.com".into(),
            "password".into(),
            "hash123".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        assert_eq!(token.len(), 43);
        assert_eq!(account_id.len(), 22);

        let accounts = store.get_browser_accounts(&browser_id);
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, "alice@example.com");
        assert_eq!(accounts[0].imap_host, "imap.example.com");

        let session = store.get(&token).expect("session should exist");
        assert_eq!(session.email, "alice@example.com");
    }

    #[test]
    fn get_account_session_validates_browser_binding() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        let session = store
            .get_account_session(&browser_id, &account_id, &token)
            .expect("should return session");
        assert_eq!(session.email, "user@example.com");
    }

    #[test]
    fn get_account_session_rejects_wrong_browser() {
        let store = long_lived_store();
        let browser1 = store.create_browser();
        let browser2 = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser1,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        assert!(
            store
                .get_account_session(&browser2, &account_id, &token)
                .is_none(),
            "should reject wrong browser"
        );
    }

    #[test]
    fn get_account_session_rejects_wrong_token() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        let (_, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        let wrong_token = SessionStore::generate_token();
        assert!(
            store
                .get_account_session(&browser_id, &account_id, &wrong_token)
                .is_none(),
            "should reject wrong token"
        );
    }

    #[test]
    fn remove_account() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        assert!(store.remove_account(&account_id));
        assert!(store.get(&token).is_none());
        assert!(store.get_browser_accounts(&browser_id).is_empty());
        assert!(!store.remove_account(&account_id));
    }

    #[test]
    fn remove_browser_clears_all_accounts() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        let (token1, account1) = store.add_account_to_browser(
            &browser_id,
            "user1@example.com".into(),
            "pass".into(),
            "hash1".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );
        let (token2, account2) = store.add_account_to_browser(
            &browser_id,
            "user2@example.com".into(),
            "pass".into(),
            "hash2".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        assert_eq!(store.get_browser_accounts(&browser_id).len(), 2);

        store.remove_browser(&browser_id);

        assert!(store.get(&token1).is_none());
        assert!(store.get(&token2).is_none());
        assert!(store.get_browser_accounts(&browser_id).is_empty());
        assert!(!store.account_to_session.contains_key(&account1));
        assert!(!store.account_to_session.contains_key(&account2));
    }

    #[test]
    fn expired_session_returns_none() {
        let store = short_lived_store();
        let browser_id = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(100));

        assert!(
            store.get(&token).is_none(),
            "expired session should not be returned"
        );
        assert!(
            store
                .get_account_session(&browser_id, &account_id, &token)
                .is_none(),
            "expired session via get_account_session should return none"
        );
    }

    #[test]
    fn multiple_accounts_per_browser() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        store.add_account_to_browser(
            &browser_id,
            "user1@example.com".into(),
            "pass1".into(),
            "hash1".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );
        store.add_account_to_browser(
            &browser_id,
            "user2@example.com".into(),
            "pass2".into(),
            "hash2".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );
        store.add_account_to_browser(
            &browser_id,
            "user3@example.com".into(),
            "pass3".into(),
            "hash3".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        let accounts = store.get_browser_accounts(&browser_id);
        assert_eq!(accounts.len(), 3);

        let emails: HashSet<_> = accounts.iter().map(|a| a.email.as_str()).collect();
        assert!(emails.contains("user1@example.com"));
        assert!(emails.contains("user2@example.com"));
        assert!(emails.contains("user3@example.com"));
    }

    #[test]
    fn browsers_are_isolated() {
        let store = long_lived_store();
        let browser1 = store.create_browser();
        let browser2 = store.create_browser();

        store.add_account_to_browser(
            &browser1,
            "user1@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );
        store.add_account_to_browser(
            &browser2,
            "user2@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        let accounts1 = store.get_browser_accounts(&browser1);
        let accounts2 = store.get_browser_accounts(&browser2);

        assert_eq!(accounts1.len(), 1);
        assert_eq!(accounts2.len(), 1);
        assert_eq!(accounts1[0].email, "user1@example.com");
        assert_eq!(accounts2[0].email, "user2@example.com");

        store.remove_browser(&browser1);
        assert_eq!(store.get_browser_accounts(&browser2).len(), 1);
    }

    #[test]
    fn sliding_window_refreshes_expiry() {
        let store = SessionStore::new(Duration::from_millis(150));
        let browser_id = store.create_browser();

        let (token, _account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(80));
        assert!(store.get(&token).is_some());

        thread::sleep(Duration::from_millis(80));
        assert!(store.get(&token).is_some());
    }

    #[test]
    fn helper_creates_valid_session() {
        let session = create_test_session("test@example.com", "pass", "hash");
        assert_eq!(session.email, "test@example.com");
        assert_eq!(session.imap_host, "imap.test.com");
        assert_eq!(session.imap_port, 993);
    }

    // --- Consistency tests ---

    fn add_test_account(store: &SessionStore, browser_id: &str, email: &str) -> (SessionId, AccountId) {
        store.add_account_to_browser(
            browser_id,
            email.into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        )
    }

    #[test]
    fn consistency_after_add_account() {
        let store = long_lived_store();
        let browser = store.create_browser();
        add_test_account(&store, &browser, "a@example.com");
        add_test_account(&store, &browser, "b@example.com");
        assert!(
            store.check_consistency().is_empty(),
            "store should be consistent after adding accounts"
        );
    }

    #[test]
    fn consistency_after_remove_account() {
        let store = long_lived_store();
        let browser = store.create_browser();
        let (_, aid1) = add_test_account(&store, &browser, "a@example.com");
        let (_, _aid2) = add_test_account(&store, &browser, "b@example.com");

        store.remove_account(&aid1);

        let issues = store.check_consistency();
        assert!(
            issues.is_empty(),
            "store should be consistent after remove_account, got: {issues:?}"
        );
        assert_eq!(store.get_browser_accounts(&browser).len(), 1);
    }

    #[test]
    fn consistency_after_remove_browser() {
        let store = long_lived_store();
        let browser = store.create_browser();
        add_test_account(&store, &browser, "a@example.com");
        add_test_account(&store, &browser, "b@example.com");

        store.remove_browser(&browser);

        let issues = store.check_consistency();
        assert!(
            issues.is_empty(),
            "store should be consistent after remove_browser, got: {issues:?}"
        );
        assert_eq!(store.sessions.len(), 0);
        assert_eq!(store.account_to_session.len(), 0);
    }

    #[test]
    fn consistency_after_purge_expired() {
        let store = short_lived_store();
        let browser = store.create_browser();
        add_test_account(&store, &browser, "a@example.com");
        add_test_account(&store, &browser, "b@example.com");

        thread::sleep(Duration::from_millis(100));
        store.purge_expired();

        let issues = store.check_consistency();
        assert!(
            issues.is_empty(),
            "store should be consistent after purge_expired, got: {issues:?}"
        );
        assert_eq!(store.sessions.len(), 0);
        assert_eq!(store.account_to_session.len(), 0);
        // Browser entry is removed when its account list becomes empty.
        assert!(!store.browser_exists(&browser));
    }

    #[test]
    fn purge_expired_respects_timeout_override() {
        let store = short_lived_store(); // 50ms default
        let browser = store.create_browser();

        // Add one account with a long timeout override
        let (token, account_id) = add_test_account(&store, &browser, "long@example.com");
        store.sessions.get_mut(&token).unwrap().timeout_override = Some(Duration::from_secs(3600));

        // Add one with default (short) timeout
        add_test_account(&store, &browser, "short@example.com");

        thread::sleep(Duration::from_millis(100));
        store.purge_expired();

        // The long-timeout session should survive.
        assert!(store.get(&token).is_some());
        assert!(store.account_to_session.contains_key(&account_id));
        assert_eq!(store.sessions.len(), 1);

        let issues = store.check_consistency();
        assert!(
            issues.is_empty(),
            "store should be consistent after selective purge, got: {issues:?}"
        );
    }

    #[test]
    fn consistency_empty_store() {
        let store = long_lived_store();
        assert!(store.check_consistency().is_empty());
    }

    #[test]
    fn consistency_mixed_operations() {
        let store = long_lived_store();
        let b1 = store.create_browser();
        let b2 = store.create_browser();

        let (_, a1) = add_test_account(&store, &b1, "a@example.com");
        add_test_account(&store, &b1, "b@example.com");
        add_test_account(&store, &b2, "c@example.com");

        store.remove_account(&a1);
        store.remove_browser(&b2);

        let issues = store.check_consistency();
        assert!(
            issues.is_empty(),
            "store should be consistent after mixed operations, got: {issues:?}"
        );
        // Only b@example.com should remain.
        assert_eq!(store.sessions.len(), 1);
        assert_eq!(store.account_to_session.len(), 1);
    }

    // --- Reverse index tests ---

    #[test]
    fn remove_account_uses_reverse_index() {
        let store = long_lived_store();

        // Create many browsers to demonstrate the O(1) lookup
        let mut other_browsers = Vec::new();
        for _ in 0..100 {
            let bid = store.create_browser();
            store.add_account_to_browser(
                &bid,
                "other@example.com".into(),
                "pass".into(),
                "hash".into(),
                "imap.example.com".into(),
                993,
                true,
                "smtp.example.com".into(),
                587,
                true,
            );
            other_browsers.push(bid);
        }

        let target_browser = store.create_browser();
        let (token, account_id) = store.add_account_to_browser(
            &target_browser,
            "target@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        // Verify reverse index was populated
        assert!(store.account_to_browser.contains_key(&account_id));
        assert_eq!(
            store.account_to_browser.get(&account_id).unwrap().value(),
            &target_browser
        );

        // Remove the account -- uses O(1) reverse index, not O(n) iteration
        assert!(store.remove_account(&account_id));

        // Verify full cleanup
        assert!(store.get(&token).is_none());
        assert!(!store.account_to_session.contains_key(&account_id));
        assert!(!store.account_to_browser.contains_key(&account_id));
        assert!(store.get_browser_accounts(&target_browser).is_empty());

        // Other browsers are untouched
        for bid in &other_browsers {
            assert_eq!(store.get_browser_accounts(bid).len(), 1);
        }
    }

    #[test]
    fn remove_browser_cleans_up_reverse_index() {
        let store = long_lived_store();
        let browser_id = store.create_browser();

        let (_, account1) = store.add_account_to_browser(
            &browser_id,
            "user1@example.com".into(),
            "pass".into(),
            "hash1".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );
        let (_, account2) = store.add_account_to_browser(
            &browser_id,
            "user2@example.com".into(),
            "pass".into(),
            "hash2".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        assert!(store.account_to_browser.contains_key(&account1));
        assert!(store.account_to_browser.contains_key(&account2));

        store.remove_browser(&browser_id);

        assert!(!store.account_to_browser.contains_key(&account1));
        assert!(!store.account_to_browser.contains_key(&account2));
    }

    #[test]
    fn purge_expired_cleans_up_reverse_index() {
        let store = short_lived_store();
        let browser_id = store.create_browser();

        let (_, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        assert!(store.account_to_browser.contains_key(&account_id));

        thread::sleep(Duration::from_millis(100));
        store.purge_expired();

        assert!(!store.account_to_browser.contains_key(&account_id));
        assert!(!store.account_to_session.contains_key(&account_id));
        assert!(store.get_browser_accounts(&browser_id).is_empty());
    }

    // --- Cascade eviction tests ---

    #[test]
    fn purge_expired_cascades_to_browsers_and_account_map() {
        let store = short_lived_store();
        let browser_id = store.create_browser();

        let (_token, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(100));

        store.purge_expired();

        assert!(store.sessions.is_empty(), "session should be purged");
        assert!(
            !store.account_to_session.contains_key(&account_id),
            "account_to_session entry should be removed"
        );
        assert!(
            !store.browsers.contains_key(&browser_id),
            "empty browser entry should be removed"
        );
    }

    #[test]
    fn purge_expired_keeps_live_accounts_in_browser() {
        let store = SessionStore::new(Duration::from_millis(150));
        let browser_id = store.create_browser();

        // First account -- will expire.
        let (_token1, account1) = store.add_account_to_browser(
            &browser_id,
            "old@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(100));

        // Second account -- added later, still alive after sleep.
        let (_token2, account2) = store.add_account_to_browser(
            &browser_id,
            "new@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(60));

        store.purge_expired();

        assert_eq!(store.sessions.len(), 1, "only the live session should remain");
        assert!(
            !store.account_to_session.contains_key(&account1),
            "expired account mapping should be gone"
        );
        assert!(
            store.account_to_session.contains_key(&account2),
            "live account mapping should remain"
        );

        let browser_accounts = store.browsers.get(&browser_id).unwrap();
        assert_eq!(browser_accounts.len(), 1);
        assert_eq!(browser_accounts[0], account2);
    }

    #[test]
    fn lazy_eviction_on_get_cascades() {
        let store = short_lived_store();
        let browser_id = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(100));

        // Accessing the expired session should trigger cascade cleanup.
        assert!(store.get(&token).is_none());

        assert!(
            !store.account_to_session.contains_key(&account_id),
            "account_to_session should be cleaned up by lazy eviction"
        );
        assert!(
            !store.browsers.contains_key(&browser_id),
            "empty browser should be cleaned up by lazy eviction"
        );
    }

    #[test]
    fn lazy_eviction_on_get_account_session_cascades() {
        let store = short_lived_store();
        let browser_id = store.create_browser();

        let (token, account_id) = store.add_account_to_browser(
            &browser_id,
            "user@example.com".into(),
            "pass".into(),
            "hash".into(),
            "imap.example.com".into(),
            993,
            true,
            "smtp.example.com".into(),
            587,
            true,
        );

        thread::sleep(Duration::from_millis(100));

        assert!(store.get_account_session(&browser_id, &account_id, &token).is_none());

        assert!(
            !store.account_to_session.contains_key(&account_id),
            "account_to_session should be cleaned up via get_account_session"
        );
    }
}
