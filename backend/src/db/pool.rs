use std::fs;
use std::path::Path;

use rusqlite::Connection;

/// Owned database context for short blocking jobs. Never hold SQLite on a Tokio worker.
#[derive(Clone)]
pub(crate) struct UserDb {
    data_dir: String,
    user_hash: String,
}

impl UserDb {
    pub(crate) fn new(data_dir: &str, user_hash: &str) -> Self {
        Self {
            data_dir: data_dir.into(),
            user_hash: user_hash.into(),
        }
    }

    pub(crate) async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Connection) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let context = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = open_user_db(&context.data_dir, &context.user_hash)?;
            work(&conn)
        })
        .await
        .map_err(|e| format!("Database task failed: {e}"))?
    }
}

/// All migration scripts in order. Each entry is (version, sql).
const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../../migrations/V001__initial_schema.sql")),
    (2, include_str!("../../migrations/V002__folders_and_messages.sql")),
    (3, include_str!("../../migrations/V003__add_date_epoch.sql")),
    (4, include_str!("../../migrations/V004__folder_messages_updated_at.sql")),
    (5, include_str!("../../migrations/V005__drafts_and_attachments.sql")),
    (6, include_str!("../../migrations/V006__cache_attachments_and_headers.sql")),
    (7, include_str!("../../migrations/V007__contacts.sql")),
    (8, include_str!("../../migrations/V008__identities.sql")),
    (9, include_str!("../../migrations/V009__notification_preferences.sql")),
    (10, include_str!("../../migrations/V010__contact_groups.sql")),
    (11, include_str!("../../migrations/V011__message_reaction.sql")),
    (12, include_str!("../../migrations/V012__tags.sql")),
    (13, include_str!("../../migrations/V013__display_preferences.sql")),
    (14, include_str!("../../migrations/V014__compose_format.sql")),
    (15, include_str!("../../migrations/V015__deep_index.sql")),
    (16, include_str!("../../migrations/V016__calendar.sql")),
    (17, include_str!("../../migrations/V017__add_email_theme.sql")),
    (18, include_str!("../../migrations/V018__animation_mode.sql")),
    (19, include_str!("../../migrations/V019__known_addresses.sql")),
];

/// Run any pending migrations based on SQLite's `user_version` PRAGMA.
///
/// If refinery has previously run migrations (indicated by the presence of
/// `refinery_schema_history`), we sync `user_version` to match the highest
/// refinery version before checking for new migrations. This avoids re-running
/// migrations that refinery already applied.
fn run_migrations(conn: &Connection) -> Result<(), String> {
    let mut current: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| format!("Failed to read user_version: {e}"))?;

    // Sync with refinery's migration state so we never re-apply migrations
    // that refinery already applied (refinery updates its own history table
    // but doesn't touch the user_version pragma).
    let refinery_max: Option<u32> = conn
        .query_row(
            "SELECT MAX(version) FROM refinery_schema_history",
            [],
            |row| row.get(0),
        )
        .unwrap_or(None);

    if let Some(v) = refinery_max
        && v > current
    {
        current = v;
        conn.execute_batch(&format!("PRAGMA user_version = {current};"))
            .map_err(|e| format!("Failed to sync user_version from refinery: {e}"))?;
    }

    for &(version, sql) in MIGRATIONS {
        if version > current {
            conn.execute_batch(sql)
                .map_err(|e| format!("Migration V{version:03} failed: {e}"))?;
            conn.execute_batch(&format!("PRAGMA user_version = {version};"))
                .map_err(|e| format!("Failed to set user_version to {version}: {e}"))?;
        }
    }

    Ok(())
}

/// Opens the per-user SQLite database at `{data_dir}/{user_hash}/db.sqlite`.
///
/// Creates the directory tree if it doesn't exist, enables WAL journal mode
/// and foreign key enforcement, then runs any pending migrations.
pub fn open_user_db(data_dir: &str, user_hash: &str) -> Result<Connection, String> {
    let dir = Path::new(data_dir).join(user_hash);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create db dir: {e}"))?;

    let db_path = dir.join("db.sqlite");
    let conn =
        Connection::open(&db_path).map_err(|e| format!("Failed to open SQLite: {e}"))?;

    // Enable WAL mode for better concurrent read performance.
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| format!("Failed to set WAL mode: {e}"))?;

    // Enable foreign key constraint enforcement.
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|e| format!("Failed to enable foreign keys: {e}"))?;

    // Run any pending schema migrations.
    run_migrations(&conn)?;

    Ok(conn)
}

/// Opens an in-memory SQLite database with all migration scripts applied.
/// Used exclusively by tests so every test starts with a clean, fully-migrated
/// schema without touching the filesystem.
#[cfg(test)]
pub fn open_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");

    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .expect("Failed to enable foreign keys");

    run_migrations(&conn).expect("Test migrations failed");

    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_test_db_has_tables() {
        let conn = open_test_db();

        // Verify the three expected tables exist.
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"user_meta".to_string()));
        assert!(tables.contains(&"folders".to_string()));
        assert!(tables.contains(&"messages".to_string()));
        assert!(tables.contains(&"drafts".to_string()));
        assert!(tables.contains(&"draft_attachments".to_string()));
    }

    #[test]
    fn test_open_user_db_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path().to_str().unwrap();

        let conn = open_user_db(data_dir, "abc123").unwrap();

        // The file should exist on disk.
        let db_file = tmp.path().join("abc123").join("db.sqlite");
        assert!(db_file.exists());

        // Foreign keys should be enabled.
        let fk: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);

        drop(conn);
    }
}

#[cfg(test)]
mod async_tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn database_jobs_leave_runtime_free() {
        let directory = tempfile::tempdir().unwrap();
        let database = UserDb::new(directory.path().to_str().unwrap(), "test");
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let job = tokio::spawn(async move {
            database.run(move |_| {
                started.send(()).unwrap();
                wait.recv_timeout(std::time::Duration::from_secs(2))
                    .map_err(|e| format!("runtime could not release database job: {e}"))?;
                Ok(())
            }).await
        });
        ready.await.unwrap();
        // On a current-thread runtime, an inline DB job prevents this send.
        release.send(()).unwrap();
        job.await.unwrap().unwrap();
    }
}
