use serde::Serialize;
use sqlx::{Row, any::AnyRow};
use std::collections::HashMap;

use super::pool::{UserDb, now};
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

/// Create a new tag.
pub async fn create_tag(db: &UserDb, id: &str, name: &str, color: &str) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "INSERT INTO tags (user_hash, id, name, color, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $5)",
    )
    .bind(db.user_hash())
    .bind(id)
    .bind(name)
    .bind(color)
    .bind(now())
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to create tag: {e}"))?;
    Ok(())
}

pub async fn list_tags(db: &UserDb) -> Result<Vec<Tag>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT t.id, t.name, t.color, t.created_at, t.updated_at,
        (SELECT COUNT(*) FROM message_tags mt WHERE mt.user_hash = t.user_hash AND mt.tag_id = t.id)
        FROM tags t WHERE t.user_hash = $1 ORDER BY t.name ASC",
    )
    .bind(db.user_hash())
    .fetch_all(&mut *connection)
    .await
    .map_err(|e| format!("Failed to query tags: {e}"))?;
    rows.iter()
        .map(|row| {
            Ok(Tag {
                id: row.try_get(0)?,
                name: row.try_get(1)?,
                color: row.try_get(2)?,
                created_at: row.try_get(3)?,
                updated_at: row.try_get(4)?,
                message_count: row.try_get(5)?,
            })
        })
        .collect::<Result<_, sqlx::Error>>()
        .map_err(|e| format!("Failed to read tag row: {e}"))
}

pub async fn update_tag(db: &UserDb, id: &str, name: &str, color: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query(
        "UPDATE tags SET name = $1, color = $2, updated_at = $3 WHERE user_hash = $4 AND id = $5",
    )
    .bind(name)
    .bind(color)
    .bind(now())
    .bind(db.user_hash())
    .bind(id)
    .execute(&mut *connection)
    .await
    .map(|r| r.rows_affected() > 0)
    .map_err(|e| format!("Failed to update tag: {e}"))
}

pub async fn delete_tag(db: &UserDb, id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM tags WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map(|r| r.rows_affected() > 0)
        .map_err(|e| format!("Failed to delete tag: {e}"))
}

/// Add a tag to a message, ignoring duplicate associations.
pub async fn add_tag_to_message(
    db: &UserDb,
    tag_id: &str,
    uid: u32,
    folder: &str,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    let result = sqlx::query(
        "INSERT INTO message_tags (user_hash, tag_id, message_uid, message_folder, created_at)
        SELECT $1, $2, $3, $4, $5
        WHERE EXISTS (SELECT 1 FROM tags WHERE user_hash = $1 AND id = $2)
          AND EXISTS (SELECT 1 FROM messages WHERE user_hash = $1 AND uid = $3 AND folder = $4)
        ON CONFLICT(user_hash, tag_id, message_uid, message_folder) DO NOTHING",
    )
    .bind(db.user_hash())
    .bind(tag_id)
    .bind(i64::from(uid))
    .bind(folder)
    .bind(now())
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to add tag to message: {e}"))?;
    if result.rows_affected() == 0 {
        let existing = sqlx::query(
            "SELECT tag_id FROM message_tags
            WHERE user_hash = $1 AND tag_id = $2 AND message_uid = $3 AND message_folder = $4",
        )
        .bind(db.user_hash())
        .bind(tag_id)
        .bind(i64::from(uid))
        .bind(folder)
        .fetch_optional(&mut *connection)
        .await
        .map_err(|e| format!("Failed to check message tag: {e}"))?;
        if existing.is_none() {
            return Err("Failed to add tag to message: tag or message not found".to_string());
        }
    }
    Ok(())
}

pub async fn remove_tag_from_message(
    db: &UserDb,
    tag_id: &str,
    uid: u32,
    folder: &str,
) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM message_tags WHERE user_hash = $1 AND tag_id = $2 AND message_uid = $3 AND message_folder = $4")
        .bind(db.user_hash()).bind(tag_id).bind(i64::from(uid)).bind(folder)
        .execute(&mut *connection).await.map(|r| r.rows_affected() > 0)
        .map_err(|e| format!("Failed to remove tag from message: {e}"))
}

fn row_to_message_tag(row: &AnyRow) -> Result<MessageTag, sqlx::Error> {
    Ok(MessageTag {
        id: row.try_get(0)?,
        name: row.try_get(1)?,
        color: row.try_get(2)?,
    })
}

pub async fn get_message_tags(
    db: &UserDb,
    uid: u32,
    folder: &str,
) -> Result<Vec<MessageTag>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT t.id, t.name, t.color FROM tags t
        JOIN message_tags mt ON mt.user_hash = t.user_hash AND mt.tag_id = t.id
        WHERE mt.user_hash = $1 AND mt.message_uid = $2 AND mt.message_folder = $3
        ORDER BY t.name ASC",
    )
    .bind(db.user_hash())
    .bind(i64::from(uid))
    .bind(folder)
    .fetch_all(&mut *connection)
    .await
    .map_err(|e| format!("Failed to query message tags: {e}"))?;
    rows.iter()
        .map(row_to_message_tag)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read message tag row: {e}"))
}

pub async fn get_tags_for_messages(
    db: &UserDb,
    messages: &[(u32, &str)],
) -> Result<HashMap<(u32, String), Vec<MessageTag>>, String> {
    if messages.is_empty() {
        return Ok(HashMap::new());
    }
    let conditions: Vec<_> = messages
        .iter()
        .enumerate()
        .map(|(i, _)| {
            format!(
                "(mt.message_uid = ${} AND mt.message_folder = ${})",
                i * 2 + 2,
                i * 2 + 3
            )
        })
        .collect();
    let sql = format!(
        "SELECT mt.message_uid, mt.message_folder, t.id, t.name, t.color
        FROM message_tags mt JOIN tags t ON t.user_hash = mt.user_hash AND t.id = mt.tag_id
        WHERE mt.user_hash = $1 AND ({}) ORDER BY t.name ASC",
        conditions.join(" OR ")
    );
    let mut query = sqlx::query(&sql).bind(db.user_hash());
    for &(uid, folder) in messages {
        query = query.bind(i64::from(uid)).bind(folder);
    }
    let mut connection = db.acquire().await?;
    let rows = query
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| format!("Failed to query tags for messages: {e}"))?;
    let mut map: HashMap<(u32, String), Vec<MessageTag>> = HashMap::new();
    for row in rows {
        let read = || -> Result<_, sqlx::Error> {
            let uid: i64 = row.try_get(0)?;
            let folder: String = row.try_get(1)?;
            let tag = MessageTag {
                id: row.try_get(2)?,
                name: row.try_get(3)?,
                color: row.try_get(4)?,
            };
            Ok((uid, folder, tag))
        };
        let (uid, folder, tag) = read().map_err(|e| format!("Failed to read tags row: {e}"))?;
        let uid = u32::try_from(uid).map_err(|e| format!("Invalid message UID: {e}"))?;
        map.entry((uid, folder)).or_default().push(tag);
    }
    Ok(map)
}

pub async fn get_messages_by_tag(
    db: &UserDb,
    tag_id: &str,
    page: u32,
    per_page: u32,
) -> Result<Vec<CachedMessage>, String> {
    let offset = i64::try_from(u64::from(page) * u64::from(per_page))
        .map_err(|e| format!("Invalid message offset: {e}"))?;
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT m.uid, m.folder, m.message_id, m.in_reply_to, m.references_header,
        m.subject, m.from_address, m.from_name, m.to_addresses, m.cc_addresses,
        m.date, m.flags, m.size, m.has_attachments, m.snippet, m.reaction
        FROM messages m JOIN message_tags mt
          ON mt.user_hash = m.user_hash AND mt.message_uid = m.uid AND mt.message_folder = m.folder
        WHERE mt.user_hash = $1 AND mt.tag_id = $2 ORDER BY m.date_epoch DESC LIMIT $3 OFFSET $4",
    )
    .bind(db.user_hash())
    .bind(tag_id)
    .bind(i64::from(per_page))
    .bind(offset)
    .fetch_all(&mut *connection)
    .await
    .map_err(|e| format!("Failed to query messages by tag: {e}"))?;
    rows.iter()
        .map(|row| {
            let uid: i64 = row
                .try_get(0)
                .map_err(|e| format!("Failed to read message UID: {e}"))?;
            let size: i64 = row
                .try_get(12)
                .map_err(|e| format!("Failed to read message size: {e}"))?;
            let uid = u32::try_from(uid).map_err(|e| format!("Invalid message UID: {e}"))?;
            let size = u32::try_from(size).map_err(|e| format!("Invalid message size: {e}"))?;
            let read = || -> Result<_, sqlx::Error> {
                Ok(CachedMessage {
                    uid,
                    folder: row.try_get(1)?,
                    message_id: row.try_get(2)?,
                    in_reply_to: row.try_get(3)?,
                    references_header: row.try_get(4)?,
                    subject: row.try_get(5)?,
                    from_address: row.try_get(6)?,
                    from_name: row.try_get(7)?,
                    to_addresses: row.try_get(8)?,
                    cc_addresses: row.try_get(9)?,
                    date: row.try_get(10)?,
                    flags: row.try_get(11)?,
                    size,
                    has_attachments: row.try_get::<i64, _>(13)? != 0,
                    snippet: row.try_get(14)?,
                    reaction: row.try_get(15)?,
                })
            };
            read().map_err(|e| format!("Failed to read message row: {e}"))
        })
        .collect()
}

pub async fn count_messages_by_tag(db: &UserDb, tag_id: &str) -> Result<u32, String> {
    let mut connection = db.acquire().await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM message_tags WHERE user_hash = $1 AND tag_id = $2",
    )
    .bind(db.user_hash())
    .bind(tag_id)
    .fetch_one(&mut *connection)
    .await
    .map_err(|e| format!("Failed to count messages by tag: {e}"))?;
    u32::try_from(count).map_err(|e| format!("Invalid message count: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    async fn insert_test_message(conn: &UserDb, folder: &str, uid: u32) {
        let mut connection = conn.acquire().await.unwrap();
        sqlx::query("INSERT INTO folders (user_hash, name) VALUES ($1, $2) ON CONFLICT(user_hash, name) DO NOTHING")
            .bind(conn.user_hash()).bind(folder).execute(&mut *connection).await.unwrap();
        sqlx::query("INSERT INTO messages (user_hash, uid, folder, subject, from_address, from_name, to_addresses, date, flags, size, has_attachments, snippet)
             VALUES ($1, $2, $3, 'Test', 'test@ex.com', 'Test', '[]', '2024-01-01', '', 100, 0, '')")
            .bind(conn.user_hash()).bind(i64::from(uid)).bind(folder).execute(&mut *connection).await.unwrap();
    }

    #[tokio::test]
    async fn test_create_and_list_tags() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();
        create_tag(&conn, "t2", "Project X", "#3b82f6")
            .await
            .unwrap();

        let tags = list_tags(&conn).await.unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "Project X");
        assert_eq!(tags[1].name, "Urgent");
        assert_eq!(tags[0].message_count, 0);
    }

    #[tokio::test]
    async fn test_update_tag() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();

        let updated = update_tag(&conn, "t1", "Very Urgent", "#dc2626")
            .await
            .unwrap();
        assert!(updated);

        let tags = list_tags(&conn).await.unwrap();
        assert_eq!(tags[0].name, "Very Urgent");
        assert_eq!(tags[0].color, "#dc2626");
    }

    #[tokio::test]
    async fn test_delete_tag_cascades() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();
        insert_test_message(&conn, "INBOX", 1).await;
        add_tag_to_message(&conn, "t1", 1, "INBOX").await.unwrap();

        let deleted = delete_tag(&conn, "t1").await.unwrap();
        assert!(deleted);

        let tags = list_tags(&conn).await.unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[tokio::test]
    async fn test_add_and_get_message_tags() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();
        create_tag(&conn, "t2", "Work", "#3b82f6").await.unwrap();
        insert_test_message(&conn, "INBOX", 1).await;

        add_tag_to_message(&conn, "t1", 1, "INBOX").await.unwrap();
        add_tag_to_message(&conn, "t2", 1, "INBOX").await.unwrap();
        // Duplicate should be ignored
        add_tag_to_message(&conn, "t1", 1, "INBOX").await.unwrap();

        let tags = get_message_tags(&conn, 1, "INBOX").await.unwrap();
        assert_eq!(tags.len(), 2);

        let tag_list = list_tags(&conn).await.unwrap();
        assert_eq!(
            tag_list
                .iter()
                .find(|t| t.name == "Urgent")
                .unwrap()
                .message_count,
            1
        );
    }

    #[tokio::test]
    async fn test_remove_tag_from_message() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();
        insert_test_message(&conn, "INBOX", 1).await;
        add_tag_to_message(&conn, "t1", 1, "INBOX").await.unwrap();

        let removed = remove_tag_from_message(&conn, "t1", 1, "INBOX")
            .await
            .unwrap();
        assert!(removed);

        let tags = get_message_tags(&conn, 1, "INBOX").await.unwrap();
        assert_eq!(tags.len(), 0);
    }

    #[tokio::test]
    async fn test_get_tags_for_messages_batch() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();
        insert_test_message(&conn, "INBOX", 1).await;
        insert_test_message(&conn, "INBOX", 2).await;
        add_tag_to_message(&conn, "t1", 1, "INBOX").await.unwrap();

        let refs: Vec<(u32, &str)> = vec![(1, "INBOX"), (2, "INBOX")];
        let map = get_tags_for_messages(&conn, &refs).await.unwrap();

        assert_eq!(map.get(&(1, "INBOX".to_string())).unwrap().len(), 1);
        assert!(!map.contains_key(&(2, "INBOX".to_string())));
    }

    #[tokio::test]
    async fn test_get_messages_by_tag() {
        let conn = open_test_db().await;
        create_tag(&conn, "t1", "Urgent", "#ef4444").await.unwrap();
        insert_test_message(&conn, "INBOX", 1).await;
        insert_test_message(&conn, "Sent", 2).await;
        add_tag_to_message(&conn, "t1", 1, "INBOX").await.unwrap();
        add_tag_to_message(&conn, "t1", 2, "Sent").await.unwrap();

        let messages = get_messages_by_tag(&conn, "t1", 0, 50).await.unwrap();
        assert_eq!(messages.len(), 2);

        let count = count_messages_by_tag(&conn, "t1").await.unwrap();
        assert_eq!(count, 2);
    }
}
