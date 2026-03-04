use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::db::messages::CachedMessage;

/// A tag record, mirroring the `tags` table.
#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight tag info returned alongside messages.
#[derive(Debug, Clone, Serialize)]
pub struct MessageTag {
    pub id: String,
    pub name: String,
    pub color: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Create a new tag.
pub fn create_tag(conn: &Connection, id: &str, name: &str, color: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)",
        params![id, name, color],
    )
    .map_err(|e| format!("Failed to create tag: {e}"))?;
    Ok(())
}

/// List all tags with their message counts.
pub fn list_tags(conn: &Connection) -> Result<Vec<Tag>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color, t.created_at, t.updated_at,
                    (SELECT COUNT(*) FROM message_tags mt WHERE mt.tag_id = t.id) AS message_count
             FROM tags t
             ORDER BY t.name ASC",
        )
        .map_err(|e| format!("Failed to prepare list_tags: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                message_count: row.get(5)?,
            })
        })
        .map_err(|e| format!("Failed to query tags: {e}"))?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row.map_err(|e| format!("Failed to read tag row: {e}"))?);
    }
    Ok(tags)
}

/// Update a tag's name and/or color.
pub fn update_tag(conn: &Connection, id: &str, name: &str, color: &str) -> Result<bool, String> {
    let updated = conn
        .execute(
            "UPDATE tags SET name = ?1, color = ?2, updated_at = datetime('now') WHERE id = ?3",
            params![name, color, id],
        )
        .map_err(|e| format!("Failed to update tag: {e}"))?;
    Ok(updated > 0)
}

/// Delete a tag. CASCADE handles junction rows.
pub fn delete_tag(conn: &Connection, id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM tags WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete tag: {e}"))?;
    Ok(deleted > 0)
}

/// Add a tag to a message (INSERT OR IGNORE to avoid duplicates).
pub fn add_tag_to_message(
    conn: &Connection,
    tag_id: &str,
    uid: u32,
    folder: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO message_tags (tag_id, message_uid, message_folder) VALUES (?1, ?2, ?3)",
        params![tag_id, uid, folder],
    )
    .map_err(|e| format!("Failed to add tag to message: {e}"))?;
    Ok(())
}

/// Remove a tag from a message.
pub fn remove_tag_from_message(
    conn: &Connection,
    tag_id: &str,
    uid: u32,
    folder: &str,
) -> Result<bool, String> {
    let deleted = conn
        .execute(
            "DELETE FROM message_tags WHERE tag_id = ?1 AND message_uid = ?2 AND message_folder = ?3",
            params![tag_id, uid, folder],
        )
        .map_err(|e| format!("Failed to remove tag from message: {e}"))?;
    Ok(deleted > 0)
}

/// Get all tags for a specific message.
pub fn get_message_tags(conn: &Connection, uid: u32, folder: &str) -> Result<Vec<MessageTag>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color
             FROM tags t
             JOIN message_tags mt ON mt.tag_id = t.id
             WHERE mt.message_uid = ?1 AND mt.message_folder = ?2
             ORDER BY t.name ASC",
        )
        .map_err(|e| format!("Failed to prepare get_message_tags: {e}"))?;

    let rows = stmt
        .query_map(params![uid, folder], |row| {
            Ok(MessageTag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
            })
        })
        .map_err(|e| format!("Failed to query message tags: {e}"))?;

    let mut tags = Vec::new();
    for row in rows {
        tags.push(row.map_err(|e| format!("Failed to read message tag row: {e}"))?);
    }
    Ok(tags)
}

/// Batch-fetch tags for multiple messages (efficient for list view).
pub fn get_tags_for_messages(
    conn: &Connection,
    messages: &[(u32, &str)],
) -> Result<HashMap<(u32, String), Vec<MessageTag>>, String> {
    if messages.is_empty() {
        return Ok(HashMap::new());
    }

    // Build a query with placeholders for all message refs.
    // Using a single query with OR conditions for reasonable batch sizes.
    let conditions: Vec<String> = messages
        .iter()
        .enumerate()
        .map(|(i, _)| format!("(mt.message_uid = ?{} AND mt.message_folder = ?{})", i * 2 + 1, i * 2 + 2))
        .collect();

    let sql = format!(
        "SELECT mt.message_uid, mt.message_folder, t.id, t.name, t.color
         FROM message_tags mt
         JOIN tags t ON t.id = mt.tag_id
         WHERE {}
         ORDER BY t.name ASC",
        conditions.join(" OR ")
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare get_tags_for_messages: {e}"))?;

    let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    for &(uid, folder) in messages {
        param_values.push(Box::new(uid));
        param_values.push(Box::new(folder.to_string()));
    }
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(params_ref.as_slice(), |row| {
            let uid: u32 = row.get(0)?;
            let folder: String = row.get(1)?;
            let tag = MessageTag {
                id: row.get(2)?,
                name: row.get(3)?,
                color: row.get(4)?,
            };
            Ok((uid, folder, tag))
        })
        .map_err(|e| format!("Failed to query tags for messages: {e}"))?;

    let mut map: HashMap<(u32, String), Vec<MessageTag>> = HashMap::new();
    for row in rows {
        let (uid, folder, tag) = row.map_err(|e| format!("Failed to read tags row: {e}"))?;
        map.entry((uid, folder)).or_default().push(tag);
    }
    Ok(map)
}

/// Get messages by tag (paginated), ordered by date descending.
pub fn get_messages_by_tag(
    conn: &Connection,
    tag_id: &str,
    page: u32,
    per_page: u32,
) -> Result<Vec<CachedMessage>, String> {
    let offset = page * per_page;
    let mut stmt = conn
        .prepare(
            "SELECT m.uid, m.folder, m.message_id, m.in_reply_to, m.references_header,
                    m.subject, m.from_address, m.from_name, m.to_addresses, m.cc_addresses,
                    m.date, m.flags, m.size, m.has_attachments, m.snippet, m.reaction
             FROM messages m
             JOIN message_tags mt ON mt.message_uid = m.uid AND mt.message_folder = m.folder
             WHERE mt.tag_id = ?1
             ORDER BY m.date_epoch DESC
             LIMIT ?2 OFFSET ?3",
        )
        .map_err(|e| format!("Failed to prepare get_messages_by_tag: {e}"))?;

    let rows = stmt
        .query_map(params![tag_id, per_page, offset], |row| {
            let has_attachments_int: i32 = row.get(13)?;
            Ok(CachedMessage {
                uid: row.get(0)?,
                folder: row.get(1)?,
                message_id: row.get(2)?,
                in_reply_to: row.get(3)?,
                references_header: row.get(4)?,
                subject: row.get(5)?,
                from_address: row.get(6)?,
                from_name: row.get(7)?,
                to_addresses: row.get(8)?,
                cc_addresses: row.get(9)?,
                date: row.get(10)?,
                flags: row.get(11)?,
                size: row.get(12)?,
                has_attachments: has_attachments_int != 0,
                snippet: row.get(14)?,
                reaction: row.get(15)?,
            })
        })
        .map_err(|e| format!("Failed to query messages by tag: {e}"))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Failed to read message row: {e}"))?);
    }
    Ok(messages)
}

/// Count messages with a given tag.
pub fn count_messages_by_tag(conn: &Connection, tag_id: &str) -> Result<u32, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM message_tags WHERE tag_id = ?1",
        params![tag_id],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to count messages by tag: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    fn insert_test_message(conn: &Connection, folder: &str, uid: u32) {
        conn.execute(
            "INSERT OR IGNORE INTO folders (name) VALUES (?1)",
            params![folder],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (uid, folder, subject, from_address, from_name, to_addresses, date, flags, size, has_attachments, snippet)
             VALUES (?1, ?2, 'Test', 'test@ex.com', 'Test', '[]', '2024-01-01', '', 100, 0, '')",
            params![uid, folder],
        )
        .unwrap();
    }

    #[test]
    fn test_create_and_list_tags() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();
        create_tag(&conn, "t2", "Project X", "#3b82f6").unwrap();

        let tags = list_tags(&conn).unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "Project X");
        assert_eq!(tags[1].name, "Urgent");
        assert_eq!(tags[0].message_count, 0);
    }

    #[test]
    fn test_update_tag() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();

        let updated = update_tag(&conn, "t1", "Very Urgent", "#dc2626").unwrap();
        assert!(updated);

        let tags = list_tags(&conn).unwrap();
        assert_eq!(tags[0].name, "Very Urgent");
        assert_eq!(tags[0].color, "#dc2626");
    }

    #[test]
    fn test_delete_tag_cascades() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();
        insert_test_message(&conn, "INBOX", 1);
        add_tag_to_message(&conn, "t1", 1, "INBOX").unwrap();

        let deleted = delete_tag(&conn, "t1").unwrap();
        assert!(deleted);

        let tags = list_tags(&conn).unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn test_add_and_get_message_tags() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();
        create_tag(&conn, "t2", "Work", "#3b82f6").unwrap();
        insert_test_message(&conn, "INBOX", 1);

        add_tag_to_message(&conn, "t1", 1, "INBOX").unwrap();
        add_tag_to_message(&conn, "t2", 1, "INBOX").unwrap();
        // Duplicate should be ignored
        add_tag_to_message(&conn, "t1", 1, "INBOX").unwrap();

        let tags = get_message_tags(&conn, 1, "INBOX").unwrap();
        assert_eq!(tags.len(), 2);

        let tag_list = list_tags(&conn).unwrap();
        assert_eq!(tag_list.iter().find(|t| t.name == "Urgent").unwrap().message_count, 1);
    }

    #[test]
    fn test_remove_tag_from_message() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();
        insert_test_message(&conn, "INBOX", 1);
        add_tag_to_message(&conn, "t1", 1, "INBOX").unwrap();

        let removed = remove_tag_from_message(&conn, "t1", 1, "INBOX").unwrap();
        assert!(removed);

        let tags = get_message_tags(&conn, 1, "INBOX").unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn test_get_tags_for_messages_batch() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();
        insert_test_message(&conn, "INBOX", 1);
        insert_test_message(&conn, "INBOX", 2);
        add_tag_to_message(&conn, "t1", 1, "INBOX").unwrap();

        let refs: Vec<(u32, &str)> = vec![(1, "INBOX"), (2, "INBOX")];
        let map = get_tags_for_messages(&conn, &refs).unwrap();

        assert_eq!(map.get(&(1, "INBOX".to_string())).unwrap().len(), 1);
        assert!(map.get(&(2, "INBOX".to_string())).is_none());
    }

    #[test]
    fn test_get_messages_by_tag() {
        let conn = open_test_db();
        create_tag(&conn, "t1", "Urgent", "#ef4444").unwrap();
        insert_test_message(&conn, "INBOX", 1);
        insert_test_message(&conn, "Sent", 2);
        add_tag_to_message(&conn, "t1", 1, "INBOX").unwrap();
        add_tag_to_message(&conn, "t1", 2, "Sent").unwrap();

        let messages = get_messages_by_tag(&conn, "t1", 0, 50).unwrap();
        assert_eq!(messages.len(), 2);

        let count = count_messages_by_tag(&conn, "t1").unwrap();
        assert_eq!(count, 2);
    }
}
