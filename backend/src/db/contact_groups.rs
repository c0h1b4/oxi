use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::contacts::Contact;

/// A contact group record, mirroring the `contact_groups` table.
#[derive(Debug, Clone, Serialize)]
pub struct ContactGroup {
    pub id: String,
    pub name: String,
    pub member_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Create a new contact group.
pub fn create_group(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO contact_groups (id, name) VALUES (?1, ?2)",
        params![id, name],
    )
    .map_err(|e| format!("Failed to create contact group: {e}"))?;
    Ok(())
}

/// List all contact groups with their member counts.
pub fn list_groups(conn: &Connection) -> Result<Vec<ContactGroup>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.name, g.created_at, g.updated_at,
                    (SELECT COUNT(*) FROM contact_group_members m WHERE m.group_id = g.id) AS member_count
             FROM contact_groups g
             ORDER BY g.name ASC",
        )
        .map_err(|e| format!("Failed to prepare list_groups: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ContactGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                member_count: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query groups: {e}"))?;

    let mut groups = Vec::new();
    for row in rows {
        groups.push(row.map_err(|e| format!("Failed to read group row: {e}"))?);
    }
    Ok(groups)
}

/// Update a group's name.
pub fn update_group(conn: &Connection, id: &str, name: &str) -> Result<bool, String> {
    let updated = conn
        .execute(
            "UPDATE contact_groups SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![name, id],
        )
        .map_err(|e| format!("Failed to update contact group: {e}"))?;
    Ok(updated > 0)
}

/// Delete a contact group. CASCADE handles member rows.
pub fn delete_group(conn: &Connection, id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM contact_groups WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete contact group: {e}"))?;
    Ok(deleted > 0)
}

/// Add a contact to a group (INSERT OR IGNORE to avoid duplicates).
pub fn add_member(conn: &Connection, group_id: &str, contact_id: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO contact_group_members (group_id, contact_id) VALUES (?1, ?2)",
        params![group_id, contact_id],
    )
    .map_err(|e| format!("Failed to add group member: {e}"))?;
    Ok(())
}

/// Remove a contact from a group.
pub fn remove_member(conn: &Connection, group_id: &str, contact_id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute(
            "DELETE FROM contact_group_members WHERE group_id = ?1 AND contact_id = ?2",
            params![group_id, contact_id],
        )
        .map_err(|e| format!("Failed to remove group member: {e}"))?;
    Ok(deleted > 0)
}

/// List members (full Contact objects) for a group.
pub fn list_group_members(conn: &Connection, group_id: &str) -> Result<Vec<Contact>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.email, c.name, c.company, c.notes, c.is_favorite,
                    c.last_contacted, c.contact_count, c.source, c.created_at, c.updated_at
             FROM contacts c
             JOIN contact_group_members m ON m.contact_id = c.id
             WHERE m.group_id = ?1
             ORDER BY c.name ASC",
        )
        .map_err(|e| format!("Failed to prepare list_group_members: {e}"))?;

    let rows = stmt
        .query_map(params![group_id], |row| {
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
        })
        .map_err(|e| format!("Failed to query group members: {e}"))?;

    let mut contacts = Vec::new();
    for row in rows {
        contacts.push(row.map_err(|e| format!("Failed to read group member row: {e}"))?);
    }
    Ok(contacts)
}

/// List all groups a given contact belongs to.
#[allow(dead_code)]
pub fn list_contact_groups(conn: &Connection, contact_id: &str) -> Result<Vec<ContactGroup>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.name, g.created_at, g.updated_at,
                    (SELECT COUNT(*) FROM contact_group_members m2 WHERE m2.group_id = g.id) AS member_count
             FROM contact_groups g
             JOIN contact_group_members m ON m.group_id = g.id
             WHERE m.contact_id = ?1
             ORDER BY g.name ASC",
        )
        .map_err(|e| format!("Failed to prepare list_contact_groups: {e}"))?;

    let rows = stmt
        .query_map(params![contact_id], |row| {
            Ok(ContactGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                member_count: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query contact groups: {e}"))?;

    let mut groups = Vec::new();
    for row in rows {
        groups.push(row.map_err(|e| format!("Failed to read contact group row: {e}"))?);
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::contacts::{upsert_contact, Contact as DbContact};
    use crate::db::pool::open_test_db;

    fn sample_contact(id: &str, email: &str, name: &str) -> DbContact {
        DbContact {
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
    fn test_create_and_list_groups() {
        let conn = open_test_db();
        create_group(&conn, "g1", "Friends").unwrap();
        create_group(&conn, "g2", "Work").unwrap();

        let groups = list_groups(&conn).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Friends");
        assert_eq!(groups[1].name, "Work");
        assert_eq!(groups[0].member_count, 0);
    }

    #[test]
    fn test_add_and_list_members() {
        let conn = open_test_db();
        create_group(&conn, "g1", "Friends").unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice")).unwrap();
        upsert_contact(&conn, &sample_contact("c2", "bob@ex.com", "Bob")).unwrap();

        add_member(&conn, "g1", "c1").unwrap();
        add_member(&conn, "g1", "c2").unwrap();
        // Duplicate add should be ignored
        add_member(&conn, "g1", "c1").unwrap();

        let members = list_group_members(&conn, "g1").unwrap();
        assert_eq!(members.len(), 2);

        let groups = list_groups(&conn).unwrap();
        assert_eq!(groups[0].member_count, 2);
    }

    #[test]
    fn test_remove_member() {
        let conn = open_test_db();
        create_group(&conn, "g1", "Friends").unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice")).unwrap();
        add_member(&conn, "g1", "c1").unwrap();

        let removed = remove_member(&conn, "g1", "c1").unwrap();
        assert!(removed);

        let members = list_group_members(&conn, "g1").unwrap();
        assert_eq!(members.len(), 0);
    }

    #[test]
    fn test_delete_group_cascades() {
        let conn = open_test_db();
        create_group(&conn, "g1", "Friends").unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice")).unwrap();
        add_member(&conn, "g1", "c1").unwrap();

        let deleted = delete_group(&conn, "g1").unwrap();
        assert!(deleted);

        let groups = list_groups(&conn).unwrap();
        assert_eq!(groups.len(), 0);
    }

    #[test]
    fn test_list_contact_groups() {
        let conn = open_test_db();
        create_group(&conn, "g1", "Friends").unwrap();
        create_group(&conn, "g2", "Work").unwrap();
        upsert_contact(&conn, &sample_contact("c1", "alice@ex.com", "Alice")).unwrap();
        add_member(&conn, "g1", "c1").unwrap();
        add_member(&conn, "g2", "c1").unwrap();

        let groups = list_contact_groups(&conn, "c1").unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_update_group() {
        let conn = open_test_db();
        create_group(&conn, "g1", "Friends").unwrap();

        let updated = update_group(&conn, "g1", "Close Friends").unwrap();
        assert!(updated);

        let groups = list_groups(&conn).unwrap();
        assert_eq!(groups[0].name, "Close Friends");
    }
}
