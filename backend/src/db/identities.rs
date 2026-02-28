use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A sender identity record, mirroring the `identities` table.
#[derive(Debug, Clone, Serialize)]
pub struct Identity {
    pub id: i64,
    pub display_name: String,
    pub email: String,
    pub signature_html: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Fields for creating a new identity.
#[derive(Debug, Deserialize)]
pub struct CreateIdentity {
    #[serde(default)]
    pub display_name: String,
    pub email: String,
    #[serde(default)]
    pub signature_html: String,
    #[serde(default)]
    pub is_default: bool,
}

/// Fields for updating an existing identity. All are optional.
#[derive(Debug, Deserialize)]
pub struct UpdateIdentity {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub signature_html: Option<String>,
    pub is_default: Option<bool>,
}

/// Map a rusqlite row to an Identity struct.
fn row_to_identity(row: &rusqlite::Row<'_>) -> rusqlite::Result<Identity> {
    let is_default_int: i32 = row.get(4)?;
    Ok(Identity {
        id: row.get(0)?,
        display_name: row.get(1)?,
        email: row.get(2)?,
        signature_html: row.get(3)?,
        is_default: is_default_int != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

const IDENTITY_SELECT_COLS: &str =
    "id, display_name, email, signature_html, is_default, created_at, updated_at";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// List all identities, ordered by is_default DESC, then email ASC.
pub fn list_identities(conn: &Connection) -> Result<Vec<Identity>, String> {
    let sql = format!(
        "SELECT {IDENTITY_SELECT_COLS} FROM identities ORDER BY is_default DESC, email ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare list_identities: {e}"))?;
    let rows = stmt
        .query_map([], row_to_identity)
        .map_err(|e| format!("Failed to query identities: {e}"))?;

    let mut identities = Vec::new();
    for row in rows {
        identities.push(row.map_err(|e| format!("Failed to read identity row: {e}"))?);
    }
    Ok(identities)
}

/// Get a single identity by ID.
pub fn get_identity(conn: &Connection, id: i64) -> Result<Option<Identity>, String> {
    let sql = format!(
        "SELECT {IDENTITY_SELECT_COLS} FROM identities WHERE id = ?1"
    );
    match conn.query_row(&sql, params![id], row_to_identity) {
        Ok(identity) => Ok(Some(identity)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get identity: {e}")),
    }
}

/// Get the default identity. Returns the first one if multiple defaults exist.
#[allow(dead_code)]
pub fn get_default_identity(conn: &Connection) -> Result<Option<Identity>, String> {
    let sql = format!(
        "SELECT {IDENTITY_SELECT_COLS} FROM identities WHERE is_default = 1 LIMIT 1"
    );
    match conn.query_row(&sql, [], row_to_identity) {
        Ok(identity) => Ok(Some(identity)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get default identity: {e}")),
    }
}

/// Create a new identity. If `is_default` is true, clears the default flag
/// on all other identities first.
pub fn create_identity(
    conn: &Connection,
    data: &CreateIdentity,
) -> Result<Identity, String> {
    if data.is_default {
        clear_default(conn)?;
    }

    conn.execute(
        "INSERT INTO identities (display_name, email, signature_html, is_default)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            data.display_name,
            data.email,
            data.signature_html,
            data.is_default as i32,
        ],
    )
    .map_err(|e| format!("Failed to create identity: {e}"))?;

    let id = conn.last_insert_rowid();
    get_identity(conn, id)?
        .ok_or_else(|| "Failed to read back created identity".to_string())
}

/// Update an existing identity. Only provided fields are changed.
/// If `is_default` is set to true, clears the default flag on all others first.
pub fn update_identity(
    conn: &Connection,
    id: i64,
    data: &UpdateIdentity,
) -> Result<Option<Identity>, String> {
    // Check it exists.
    if get_identity(conn, id)?.is_none() {
        return Ok(None);
    }

    if data.is_default == Some(true) {
        clear_default(conn)?;
    }

    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(ref display_name) = data.display_name {
        sets.push(format!("display_name = ?{idx}"));
        values.push(Box::new(display_name.clone()));
        idx += 1;
    }
    if let Some(ref email) = data.email {
        sets.push(format!("email = ?{idx}"));
        values.push(Box::new(email.clone()));
        idx += 1;
    }
    if let Some(ref signature_html) = data.signature_html {
        sets.push(format!("signature_html = ?{idx}"));
        values.push(Box::new(signature_html.clone()));
        idx += 1;
    }
    if let Some(is_default) = data.is_default {
        sets.push(format!("is_default = ?{idx}"));
        values.push(Box::new(is_default as i32));
        idx += 1;
    }

    if sets.is_empty() {
        return get_identity(conn, id);
    }

    sets.push(format!("updated_at = datetime('now')"));
    let set_clause = sets.join(", ");
    let sql = format!("UPDATE identities SET {set_clause} WHERE id = ?{idx}");
    values.push(Box::new(id));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|v| v.as_ref()).collect();

    conn.execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("Failed to update identity: {e}"))?;

    get_identity(conn, id)
}

/// Delete an identity by ID. Returns true if a row was deleted.
pub fn delete_identity(conn: &Connection, id: i64) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM identities WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete identity: {e}"))?;
    Ok(deleted > 0)
}

/// Check if any identities exist.
pub fn has_identities(conn: &Connection) -> Result<bool, String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM identities", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count identities: {e}"))?;
    Ok(count > 0)
}

/// Clear the is_default flag on all identities.
fn clear_default(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE identities SET is_default = 0, updated_at = datetime('now') WHERE is_default = 1",
        [],
    )
    .map_err(|e| format!("Failed to clear default identity: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_create_and_list_identities() {
        let conn = open_test_db();

        let identity = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: "<p>Best,<br>Alice</p>".to_string(),
                is_default: true,
            },
        )
        .unwrap();

        assert_eq!(identity.display_name, "Alice");
        assert_eq!(identity.email, "alice@example.com");
        assert!(identity.is_default);

        let all = list_identities(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].email, "alice@example.com");
    }

    #[test]
    fn test_only_one_default() {
        let conn = open_test_db();

        let first = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .unwrap();

        let second = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Bob".to_string(),
                email: "bob@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .unwrap();

        // Second should be default, first should not.
        assert!(second.is_default);
        let refreshed_first = get_identity(&conn, first.id).unwrap().unwrap();
        assert!(!refreshed_first.is_default);
    }

    #[test]
    fn test_update_identity() {
        let conn = open_test_db();

        let identity = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: false,
            },
        )
        .unwrap();

        let updated = update_identity(
            &conn,
            identity.id,
            &UpdateIdentity {
                display_name: Some("Alice Smith".to_string()),
                email: None,
                signature_html: None,
                is_default: None,
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(updated.display_name, "Alice Smith");
        assert_eq!(updated.email, "alice@example.com");
    }

    #[test]
    fn test_delete_identity() {
        let conn = open_test_db();

        let identity = create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: false,
            },
        )
        .unwrap();

        assert!(delete_identity(&conn, identity.id).unwrap());
        assert!(get_identity(&conn, identity.id).unwrap().is_none());
        assert!(!delete_identity(&conn, identity.id).unwrap());
    }

    #[test]
    fn test_has_identities() {
        let conn = open_test_db();
        assert!(!has_identities(&conn).unwrap());

        create_identity(
            &conn,
            &CreateIdentity {
                display_name: "Alice".to_string(),
                email: "alice@example.com".to_string(),
                signature_html: String::new(),
                is_default: true,
            },
        )
        .unwrap();

        assert!(has_identities(&conn).unwrap());
    }
}
