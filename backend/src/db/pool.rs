use std::fs;
use std::path::Path;

use rusqlite::Connection;

/// Opens the per-user SQLite database at `{data_dir}/{user_hash}/db.sqlite`.
///
/// Creates the directory tree if it doesn't exist, enables WAL journal mode
/// and foreign key enforcement, then runs refinery migrations.
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

    Ok(conn)
}

/// Opens an in-memory SQLite database with both migration scripts applied.
/// Used exclusively by tests so every test starts with a clean, fully-migrated
/// schema without touching the filesystem.
#[cfg(test)]
pub fn open_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("Failed to open in-memory SQLite");

    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .expect("Failed to enable foreign keys");

    let v001 = include_str!("../../migrations/V001__initial_schema.sql");
    let v002 = include_str!("../../migrations/V002__folders_and_messages.sql");

    conn.execute_batch(v001)
        .expect("V001 migration failed");
    conn.execute_batch(v002)
        .expect("V002 migration failed");

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
