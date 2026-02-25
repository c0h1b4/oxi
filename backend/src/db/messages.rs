use rusqlite::{Connection, params};
use serde::Serialize;

/// A cached email message header, mirroring the query-visible columns of the
/// `messages` table.
#[derive(Debug, Clone, Serialize)]
pub struct CachedMessage {
    pub uid: u32,
    pub folder: String,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references_header: Option<String>,
    pub subject: String,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: String,
    pub cc_addresses: String,
    pub date: String,
    pub flags: String,
    pub size: u32,
    pub has_attachments: bool,
    pub snippet: String,
}

// ---------------------------------------------------------------------------
// Helper to map a row to CachedMessage (used in multiple queries)
// ---------------------------------------------------------------------------

fn row_to_cached_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<CachedMessage> {
    let has_attachments_int: i32 = row.get(12)?;
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
        size: row.get(12 + 1)?,     // size is column 13 (index 13)
        has_attachments: has_attachments_int != 0,
        snippet: row.get(14)?,
    })
}

/// The SELECT column list used by all queries that return `CachedMessage`.
const MSG_SELECT_COLS: &str =
    "uid, folder, message_id, in_reply_to, references_header,
     subject, from_address, from_name, to_addresses, cc_addresses,
     date, flags, has_attachments, size, snippet";

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Insert or replace a message header row.
#[allow(clippy::too_many_arguments)]
pub fn upsert_message(
    conn: &Connection,
    folder: &str,
    uid: u32,
    message_id: Option<&str>,
    in_reply_to: Option<&str>,
    references_header: Option<&str>,
    subject: &str,
    from_address: &str,
    from_name: &str,
    to_json: &str,
    cc_json: &str,
    date: &str,
    flags_csv: &str,
    size: u32,
    has_attachments: bool,
    snippet: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO messages
            (uid, folder, message_id, in_reply_to, references_header,
             subject, from_address, from_name, to_addresses, cc_addresses,
             date, flags, size, has_attachments, snippet)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            uid,
            folder,
            message_id,
            in_reply_to,
            references_header,
            subject,
            from_address,
            from_name,
            to_json,
            cc_json,
            date,
            flags_csv,
            size,
            has_attachments as i32,
            snippet,
        ],
    )
    .map_err(|e| format!("Failed to upsert message: {e}"))?;
    Ok(())
}

/// Return a page of messages for a folder, ordered by date descending.
/// `page` is 0-indexed.
pub fn get_messages(
    conn: &Connection,
    folder: &str,
    page: u32,
    per_page: u32,
) -> Result<Vec<CachedMessage>, String> {
    let offset = page * per_page;
    let sql = format!(
        "SELECT {MSG_SELECT_COLS}
         FROM messages
         WHERE folder = ?1
         ORDER BY date DESC
         LIMIT ?2 OFFSET ?3"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare get_messages: {e}"))?;

    let rows = stmt
        .query_map(params![folder, per_page, offset], |row| {
            row_to_cached_message(row)
        })
        .map_err(|e| format!("Failed to query messages: {e}"))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Failed to read message row: {e}"))?);
    }
    Ok(messages)
}

/// Return the total number of cached messages for a folder.
pub fn count_messages(conn: &Connection, folder: &str) -> Result<u32, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE folder = ?1",
        params![folder],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to count messages: {e}"))
}

/// Update only the flags column for a specific message.
pub fn update_message_flags(
    conn: &Connection,
    folder: &str,
    uid: u32,
    flags_csv: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages SET flags = ?1 WHERE folder = ?2 AND uid = ?3",
        params![flags_csv, folder, uid],
    )
    .map_err(|e| format!("Failed to update message flags: {e}"))?;
    Ok(())
}

/// Cache a message body (HTML and/or plain text).
pub fn cache_message_body(
    conn: &Connection,
    folder: &str,
    uid: u32,
    html: Option<&str>,
    text: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages
         SET body_html = ?1, body_text = ?2, body_cached = 1
         WHERE folder = ?3 AND uid = ?4",
        params![html, text, folder, uid],
    )
    .map_err(|e| format!("Failed to cache message body: {e}"))?;
    Ok(())
}

/// Return the cached body if `body_cached = 1`, otherwise `None`.
#[allow(clippy::type_complexity)]
pub fn get_cached_body(
    conn: &Connection,
    folder: &str,
    uid: u32,
) -> Result<Option<(Option<String>, Option<String>)>, String> {
    let result = conn.query_row(
        "SELECT body_cached, body_html, body_text
         FROM messages
         WHERE folder = ?1 AND uid = ?2",
        params![folder, uid],
        |row| {
            let cached: i32 = row.get(0)?;
            if cached == 1 {
                let html: Option<String> = row.get(1)?;
                let text: Option<String> = row.get(2)?;
                Ok(Some((html, text)))
            } else {
                Ok(None)
            }
        },
    );

    match result {
        Ok(body) => Ok(body),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get cached body: {e}")),
    }
}

/// Delete a single message by folder and UID.
pub fn delete_message(conn: &Connection, folder: &str, uid: u32) -> Result<(), String> {
    conn.execute(
        "DELETE FROM messages WHERE folder = ?1 AND uid = ?2",
        params![folder, uid],
    )
    .map_err(|e| format!("Failed to delete message: {e}"))?;
    Ok(())
}

/// Find messages related to the given `target_message_id` for threading.
///
/// Returns messages where:
/// - `message_id` equals `target_message_id`, OR
/// - `in_reply_to` equals `target_message_id`, OR
/// - `references_header` contains `target_message_id`
pub fn get_thread_messages(
    conn: &Connection,
    target_message_id: &str,
) -> Result<Vec<CachedMessage>, String> {
    let like_pattern = format!("%{target_message_id}%");
    let sql = format!(
        "SELECT {MSG_SELECT_COLS}
         FROM messages
         WHERE message_id = ?1
            OR in_reply_to = ?1
            OR references_header LIKE ?2
         ORDER BY date ASC"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare get_thread_messages: {e}"))?;

    let rows = stmt
        .query_map(params![target_message_id, like_pattern], |row| {
            row_to_cached_message(row)
        })
        .map_err(|e| format!("Failed to query thread messages: {e}"))?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Failed to read thread message row: {e}"))?);
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::folders::upsert_folder;
    use crate::db::pool::open_test_db;

    /// Helper: insert a folder so that the foreign key constraint is satisfied.
    fn ensure_folder(conn: &Connection, name: &str) {
        upsert_folder(conn, name, Some("/"), None, "", true, 0, 0, 1, 0).unwrap();
    }

    /// Helper: insert a sample message.
    fn insert_sample(conn: &Connection, folder: &str, uid: u32, date: &str) {
        upsert_message(
            conn,
            folder,
            uid,
            Some(&format!("<msg-{uid}@example.com>")),
            None,
            None,
            &format!("Subject {uid}"),
            "alice@example.com",
            "Alice",
            "[]",
            "[]",
            date,
            "\\Seen",
            1024,
            false,
            "snippet",
        )
        .unwrap();
    }

    #[test]
    fn test_upsert_and_get_messages() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z");
        insert_sample(&conn, "INBOX", 2, "2024-01-02T10:00:00Z");
        insert_sample(&conn, "INBOX", 3, "2024-01-03T10:00:00Z");

        let msgs = get_messages(&conn, "INBOX", 0, 10).unwrap();
        assert_eq!(msgs.len(), 3);

        // Should be date DESC: uid 3, 2, 1.
        assert_eq!(msgs[0].uid, 3);
        assert_eq!(msgs[1].uid, 2);
        assert_eq!(msgs[2].uid, 1);
    }

    #[test]
    fn test_pagination_no_overlap() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        for uid in 1..=5 {
            insert_sample(&conn, "INBOX", uid, &format!("2024-01-{:02}T10:00:00Z", uid));
        }

        let page0 = get_messages(&conn, "INBOX", 0, 2).unwrap();
        let page1 = get_messages(&conn, "INBOX", 1, 2).unwrap();
        let page2 = get_messages(&conn, "INBOX", 2, 2).unwrap();

        assert_eq!(page0.len(), 2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 1);

        // Verify no UIDs overlap between pages.
        let uids0: Vec<u32> = page0.iter().map(|m| m.uid).collect();
        let uids1: Vec<u32> = page1.iter().map(|m| m.uid).collect();
        let uids2: Vec<u32> = page2.iter().map(|m| m.uid).collect();

        for uid in &uids0 {
            assert!(!uids1.contains(uid));
            assert!(!uids2.contains(uid));
        }
        for uid in &uids1 {
            assert!(!uids2.contains(uid));
        }
    }

    #[test]
    fn test_count_messages() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        assert_eq!(count_messages(&conn, "INBOX").unwrap(), 0);

        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z");
        insert_sample(&conn, "INBOX", 2, "2024-01-02T10:00:00Z");

        assert_eq!(count_messages(&conn, "INBOX").unwrap(), 2);
    }

    #[test]
    fn test_update_message_flags() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z");

        update_message_flags(&conn, "INBOX", 1, "\\Seen,\\Flagged").unwrap();

        let msgs = get_messages(&conn, "INBOX", 0, 10).unwrap();
        assert_eq!(msgs[0].flags, "\\Seen,\\Flagged");
    }

    #[test]
    fn test_cache_and_get_body_initially_none() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z");

        // Body should not be cached yet.
        let body = get_cached_body(&conn, "INBOX", 1).unwrap();
        assert!(body.is_none());
    }

    #[test]
    fn test_cache_and_get_body_after_caching() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z");

        cache_message_body(&conn, "INBOX", 1, Some("<h1>Hello</h1>"), Some("Hello"))
            .unwrap();

        let body = get_cached_body(&conn, "INBOX", 1).unwrap();
        assert!(body.is_some());
        let (html, text) = body.unwrap();
        assert_eq!(html.unwrap(), "<h1>Hello</h1>");
        assert_eq!(text.unwrap(), "Hello");
    }

    #[test]
    fn test_delete_message() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z");
        insert_sample(&conn, "INBOX", 2, "2024-01-02T10:00:00Z");

        delete_message(&conn, "INBOX", 1).unwrap();

        let msgs = get_messages(&conn, "INBOX", 0, 10).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].uid, 2);
    }

    #[test]
    fn test_get_thread_messages_by_message_id() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        // Original message.
        upsert_message(
            &conn, "INBOX", 1,
            Some("<thread-1@example.com>"), None, None,
            "Hello", "alice@example.com", "Alice", "[]", "[]",
            "2024-01-01T10:00:00Z", "", 100, false, "",
        ).unwrap();

        // Reply referencing original via in_reply_to.
        upsert_message(
            &conn, "INBOX", 2,
            Some("<reply-1@example.com>"),
            Some("<thread-1@example.com>"),
            None,
            "Re: Hello", "bob@example.com", "Bob", "[]", "[]",
            "2024-01-02T10:00:00Z", "", 200, false, "",
        ).unwrap();

        let thread = get_thread_messages(&conn, "<thread-1@example.com>").unwrap();
        assert_eq!(thread.len(), 2);
        // ASC order: uid 1 first, uid 2 second.
        assert_eq!(thread[0].uid, 1);
        assert_eq!(thread[1].uid, 2);
    }

    #[test]
    fn test_get_thread_messages_by_references_header() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        // Original message.
        upsert_message(
            &conn, "INBOX", 1,
            Some("<orig@example.com>"), None, None,
            "Hello", "alice@example.com", "Alice", "[]", "[]",
            "2024-01-01T10:00:00Z", "", 100, false, "",
        ).unwrap();

        // A message that references the original only via references_header.
        upsert_message(
            &conn, "INBOX", 3,
            Some("<deep-reply@example.com>"),
            Some("<mid@example.com>"),
            Some("<orig@example.com> <mid@example.com>"),
            "Re: Re: Hello", "carol@example.com", "Carol", "[]", "[]",
            "2024-01-03T10:00:00Z", "", 300, false, "",
        ).unwrap();

        let thread = get_thread_messages(&conn, "<orig@example.com>").unwrap();
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[0].uid, 1); // matched by message_id
        assert_eq!(thread[1].uid, 3); // matched by references_header LIKE
    }
}
