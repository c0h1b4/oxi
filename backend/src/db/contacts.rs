use rusqlite::{params, Connection};
use serde::Serialize;
use uuid::Uuid;

/// A contact record, mirroring the `contacts` table.
#[derive(Debug, Clone, Serialize)]
pub struct Contact {
    pub id: String,
    pub email: String,
    pub name: String,
    pub company: String,
    pub notes: String,
    pub is_favorite: bool,
    pub last_contacted: Option<String>,
    pub contact_count: i64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Map a rusqlite row to a Contact struct.
fn row_to_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
    let is_favorite_int: i32 = row.get(5)?;
    Ok(Contact {
        id: row.get(0)?,
        email: row.get(1)?,
        name: row.get(2)?,
        company: row.get(3)?,
        notes: row.get(4)?,
        is_favorite: is_favorite_int != 0,
        last_contacted: row.get(6)?,
        contact_count: row.get(7)?,
        source: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

/// The SELECT column list used by all queries that return `Contact`.
const CONTACT_SELECT_COLS: &str =
    "id, email, name, company, notes, is_favorite, last_contacted, contact_count, source, created_at, updated_at";

/// Escape `%` and `_` in a search term to prevent LIKE wildcard injection.
fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Insert or replace a contact. All fields are written.
pub fn upsert_contact(conn: &Connection, contact: &Contact) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO contacts
            (id, email, name, company, notes, is_favorite, last_contacted,
             contact_count, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            contact.id,
            contact.email,
            contact.name,
            contact.company,
            contact.notes,
            contact.is_favorite as i32,
            contact.last_contacted,
            contact.contact_count,
            contact.source,
            contact.created_at,
            contact.updated_at,
        ],
    )
    .map_err(|e| format!("Failed to upsert contact: {e}"))?;
    Ok(())
}

/// Get a contact by its primary key `id`. Returns `None` if not found.
pub fn get_contact(conn: &Connection, id: &str) -> Result<Option<Contact>, String> {
    let sql = format!(
        "SELECT {CONTACT_SELECT_COLS} FROM contacts WHERE id = ?1"
    );
    let result = conn.query_row(&sql, params![id], row_to_contact);

    match result {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get contact: {e}")),
    }
}

/// Get a contact by email address. Returns `None` if not found.
pub fn get_contact_by_email(conn: &Connection, email: &str) -> Result<Option<Contact>, String> {
    let sql = format!(
        "SELECT {CONTACT_SELECT_COLS} FROM contacts WHERE email = ?1"
    );
    let result = conn.query_row(&sql, params![email], row_to_contact);

    match result {
        Ok(c) => Ok(Some(c)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get contact by email: {e}")),
    }
}

/// List contacts with optional search filter on name/email.
/// Results are ordered by name ascending, with LIMIT/OFFSET pagination.
pub fn list_contacts(
    conn: &Connection,
    search: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<Contact>, String> {
    let (sql, has_search) = match search {
        Some(q) if !q.is_empty() => {
            let escaped = escape_like(q);
            let pattern = format!("%{escaped}%");
            let sql = format!(
                "SELECT {CONTACT_SELECT_COLS} FROM contacts
                 WHERE name LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\'
                 ORDER BY name ASC
                 LIMIT ?2 OFFSET ?3"
            );
            // We need to pass the pattern, so we store it and handle below.
            // To keep things simple we branch on has_search.
            drop(pattern);
            (sql, Some(escaped))
        }
        _ => {
            let sql = format!(
                "SELECT {CONTACT_SELECT_COLS} FROM contacts
                 ORDER BY name ASC
                 LIMIT ?1 OFFSET ?2"
            );
            (sql, None)
        }
    };

    match has_search {
        Some(escaped) => {
            let pattern = format!("%{escaped}%");
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("Failed to prepare list_contacts: {e}"))?;
            let rows = stmt
                .query_map(params![pattern, limit, offset], row_to_contact)
                .map_err(|e| format!("Failed to query contacts: {e}"))?;
            let mut contacts = Vec::new();
            for row in rows {
                contacts.push(row.map_err(|e| format!("Failed to read contact row: {e}"))?);
            }
            Ok(contacts)
        }
        None => {
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| format!("Failed to prepare list_contacts: {e}"))?;
            let rows = stmt
                .query_map(params![limit, offset], row_to_contact)
                .map_err(|e| format!("Failed to query contacts: {e}"))?;
            let mut contacts = Vec::new();
            for row in rows {
                contacts.push(row.map_err(|e| format!("Failed to read contact row: {e}"))?);
            }
            Ok(contacts)
        }
    }
}

/// Delete a contact by id. Returns `true` if a row was deleted.
pub fn delete_contact(conn: &Connection, id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM contacts WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete contact: {e}"))?;
    Ok(deleted > 0)
}

/// Search contacts by name or email using LIKE with escaped wildcards.
/// Results are ordered by name ascending.
pub fn search_contacts(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<Contact>, String> {
    let escaped = escape_like(query);
    let pattern = format!("%{escaped}%");

    let sql = format!(
        "SELECT {CONTACT_SELECT_COLS} FROM contacts
         WHERE name LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\'
         ORDER BY name ASC
         LIMIT ?2"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare search_contacts: {e}"))?;
    let rows = stmt
        .query_map(params![pattern, limit], row_to_contact)
        .map_err(|e| format!("Failed to query search contacts: {e}"))?;

    let mut contacts = Vec::new();
    for row in rows {
        contacts.push(row.map_err(|e| format!("Failed to read contact row: {e}"))?);
    }
    Ok(contacts)
}

/// Increment the contact_count by 1 and set last_contacted to now for the
/// contact with the given email address.
#[allow(dead_code)]
pub fn increment_contact_count(conn: &Connection, email: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE contacts
         SET contact_count = contact_count + 1,
             last_contacted = datetime('now'),
             updated_at = datetime('now')
         WHERE email = ?1",
        params![email],
    )
    .map_err(|e| format!("Failed to increment contact count: {e}"))?;
    Ok(())
}

/// Auto-add a contact if one with the same email does not already exist.
/// Uses INSERT OR IGNORE so existing contacts are not overwritten.
/// Source is set to 'auto'.
#[allow(dead_code)]
pub fn auto_add_contact(conn: &Connection, email: &str, name: &str) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO contacts (id, email, name, source)
         VALUES (?1, ?2, ?3, 'auto')",
        params![id, email, name],
    )
    .map_err(|e| format!("Failed to auto-add contact: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    /// Helper: create a sample Contact with sensible defaults.
    fn sample_contact(id: &str, email: &str, name: &str) -> Contact {
        Contact {
            id: id.to_string(),
            email: email.to_string(),
            name: name.to_string(),
            company: String::new(),
            notes: String::new(),
            is_favorite: false,
            last_contacted: None,
            contact_count: 0,
            source: "manual".to_string(),
            created_at: "2024-01-01 00:00:00".to_string(),
            updated_at: "2024-01-01 00:00:00".to_string(),
        }
    }

    #[test]
    fn test_upsert_and_get_contact() {
        let conn = open_test_db();
        let contact = sample_contact("c1", "alice@example.com", "Alice");

        upsert_contact(&conn, &contact).unwrap();

        let fetched = get_contact(&conn, "c1").unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, "c1");
        assert_eq!(fetched.email, "alice@example.com");
        assert_eq!(fetched.name, "Alice");
        assert_eq!(fetched.source, "manual");
    }

    #[test]
    fn test_get_contact_by_email() {
        let conn = open_test_db();
        let contact = sample_contact("c2", "bob@example.com", "Bob");
        upsert_contact(&conn, &contact).unwrap();

        let fetched = get_contact_by_email(&conn, "bob@example.com").unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, "c2");
        assert_eq!(fetched.name, "Bob");

        // Non-existent email returns None.
        let missing = get_contact_by_email(&conn, "nobody@example.com").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_list_contacts() {
        let conn = open_test_db();
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice")).unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@example.com", "Bob")).unwrap();

        let all = list_contacts(&conn, None, 100, 0).unwrap();
        assert_eq!(all.len(), 2);

        // Ordered by name ASC: Alice, Bob.
        assert_eq!(all[0].name, "Alice");
        assert_eq!(all[1].name, "Bob");
    }

    #[test]
    fn test_search_contacts() {
        let conn = open_test_db();
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice Smith")).unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@example.com", "Bob Jones")).unwrap();

        // Search by name should find only Alice.
        let results = search_contacts(&conn, "Alice", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Alice Smith");

        // Search by email should find only Bob.
        let results = search_contacts(&conn, "bob@", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Bob Jones");
    }

    #[test]
    fn test_delete_contact() {
        let conn = open_test_db();
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice")).unwrap();

        let deleted = delete_contact(&conn, "c1").unwrap();
        assert!(deleted);

        // Should be gone now.
        let fetched = get_contact(&conn, "c1").unwrap();
        assert!(fetched.is_none());

        // Deleting again returns false.
        let deleted_again = delete_contact(&conn, "c1").unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_auto_add_contact() {
        let conn = open_test_db();

        auto_add_contact(&conn, "auto@example.com", "Auto User").unwrap();

        let fetched = get_contact_by_email(&conn, "auto@example.com").unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.source, "auto");
        assert_eq!(fetched.name, "Auto User");

        // Auto-adding the same email again should be a no-op (INSERT OR IGNORE).
        auto_add_contact(&conn, "auto@example.com", "Different Name").unwrap();
        let fetched2 = get_contact_by_email(&conn, "auto@example.com").unwrap().unwrap();
        assert_eq!(fetched2.name, "Auto User"); // Name should NOT change.
    }

    #[test]
    fn test_increment_contact_count() {
        let conn = open_test_db();
        upsert_contact(&conn, &sample_contact("c1", "alice@example.com", "Alice")).unwrap();

        // Increment twice.
        increment_contact_count(&conn, "alice@example.com").unwrap();
        increment_contact_count(&conn, "alice@example.com").unwrap();

        let fetched = get_contact_by_email(&conn, "alice@example.com").unwrap().unwrap();
        assert_eq!(fetched.contact_count, 2);
        assert!(fetched.last_contacted.is_some());
    }
}
