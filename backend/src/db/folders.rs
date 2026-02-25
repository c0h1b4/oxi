use rusqlite::{Connection, params};
use serde::Serialize;

/// A cached IMAP folder, mirroring the `folders` table schema.
#[derive(Debug, Clone, Serialize)]
pub struct CachedFolder {
    pub name: String,
    pub delimiter: Option<String>,
    pub parent: Option<String>,
    pub flags: String,
    pub is_subscribed: bool,
    pub total_count: u32,
    pub unread_count: u32,
    pub uid_validity: u32,
    pub highest_modseq: u64,
}

/// Insert or replace a folder row in the `folders` table.
pub fn upsert_folder(
    conn: &Connection,
    folder_name: &str,
    delimiter: Option<&str>,
    parent: Option<&str>,
    flags_csv: &str,
    is_subscribed: bool,
    total_count: u32,
    unread_count: u32,
    uid_validity: u32,
    highest_modseq: u64,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO folders
            (name, delimiter, parent, flags, is_subscribed,
             total_count, unread_count, uid_validity, highest_modseq, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))",
        params![
            folder_name,
            delimiter,
            parent,
            flags_csv,
            is_subscribed as i32,
            total_count,
            unread_count,
            uid_validity,
            highest_modseq as i64,
        ],
    )
    .map_err(|e| format!("Failed to upsert folder: {e}"))?;
    Ok(())
}

/// Return all cached folders, sorted alphabetically by name.
pub fn get_all_folders(conn: &Connection) -> Result<Vec<CachedFolder>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT name, delimiter, parent, flags, is_subscribed,
                    total_count, unread_count, uid_validity, highest_modseq
             FROM folders
             ORDER BY name",
        )
        .map_err(|e| format!("Failed to prepare get_all_folders: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            let is_subscribed_int: i32 = row.get(4)?;
            let highest_modseq_int: i64 = row.get(8)?;
            Ok(CachedFolder {
                name: row.get(0)?,
                delimiter: row.get(1)?,
                parent: row.get(2)?,
                flags: row.get(3)?,
                is_subscribed: is_subscribed_int != 0,
                total_count: row.get(5)?,
                unread_count: row.get(6)?,
                uid_validity: row.get(7)?,
                highest_modseq: highest_modseq_int as u64,
            })
        })
        .map_err(|e| format!("Failed to query folders: {e}"))?;

    let mut folders = Vec::new();
    for row in rows {
        folders.push(row.map_err(|e| format!("Failed to read folder row: {e}"))?);
    }
    Ok(folders)
}

/// Delete folders whose names are NOT in the provided `current_names` list.
/// Returns the number of deleted rows.
pub fn remove_stale_folders(
    conn: &Connection,
    current_names: &[String],
) -> Result<usize, String> {
    if current_names.is_empty() {
        // Delete all folders when the current list is empty.
        let deleted = conn
            .execute("DELETE FROM folders", [])
            .map_err(|e| format!("Failed to delete all folders: {e}"))?;
        return Ok(deleted);
    }

    // Build a parameterized WHERE NOT IN clause.
    let placeholders: Vec<String> = (1..=current_names.len())
        .map(|i| format!("?{i}"))
        .collect();
    let sql = format!(
        "DELETE FROM folders WHERE name NOT IN ({})",
        placeholders.join(", ")
    );

    let params: Vec<&dyn rusqlite::types::ToSql> = current_names
        .iter()
        .map(|n| n as &dyn rusqlite::types::ToSql)
        .collect();

    let deleted = conn
        .execute(&sql, params.as_slice())
        .map_err(|e| format!("Failed to remove stale folders: {e}"))?;

    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_upsert_and_get_folders() {
        let conn = open_test_db();

        upsert_folder(&conn, "INBOX", Some("/"), None, "\\HasNoChildren", true, 42, 5, 100, 200)
            .unwrap();
        upsert_folder(&conn, "Sent", Some("/"), None, "\\Sent", true, 10, 0, 101, 300)
            .unwrap();

        let folders = get_all_folders(&conn).unwrap();
        assert_eq!(folders.len(), 2);

        // Sorted alphabetically: INBOX < Sent
        assert_eq!(folders[0].name, "INBOX");
        assert_eq!(folders[0].total_count, 42);
        assert_eq!(folders[0].unread_count, 5);
        assert!(folders[0].is_subscribed);

        assert_eq!(folders[1].name, "Sent");
        assert_eq!(folders[1].highest_modseq, 300);
    }

    #[test]
    fn test_upsert_updates_existing_folder() {
        let conn = open_test_db();

        upsert_folder(&conn, "INBOX", Some("/"), None, "\\HasNoChildren", true, 10, 2, 100, 50)
            .unwrap();

        // Upsert again with different counts.
        upsert_folder(&conn, "INBOX", Some("/"), None, "\\HasNoChildren", true, 99, 33, 100, 75)
            .unwrap();

        let folders = get_all_folders(&conn).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].total_count, 99);
        assert_eq!(folders[0].unread_count, 33);
        assert_eq!(folders[0].highest_modseq, 75);
    }

    #[test]
    fn test_remove_stale_folders() {
        let conn = open_test_db();

        upsert_folder(&conn, "INBOX", None, None, "", true, 0, 0, 0, 0).unwrap();
        upsert_folder(&conn, "Sent", None, None, "", true, 0, 0, 0, 0).unwrap();
        upsert_folder(&conn, "Trash", None, None, "", true, 0, 0, 0, 0).unwrap();

        // Keep only INBOX and Sent; Trash should be removed.
        let current = vec!["INBOX".to_string(), "Sent".to_string()];
        let deleted = remove_stale_folders(&conn, &current).unwrap();
        assert_eq!(deleted, 1);

        let folders = get_all_folders(&conn).unwrap();
        let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"INBOX"));
        assert!(names.contains(&"Sent"));
        assert!(!names.contains(&"Trash"));
    }
}
