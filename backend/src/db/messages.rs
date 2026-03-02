use rusqlite::{Connection, params};
use serde::Serialize;

// mail_parser is used for RFC2822 date parsing in parse_date_to_epoch.

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
    let date_epoch = parse_date_to_epoch(date);
    conn.execute(
        "INSERT OR REPLACE INTO messages
            (uid, folder, message_id, in_reply_to, references_header,
             subject, from_address, from_name, to_addresses, cc_addresses,
             date, flags, size, has_attachments, snippet, date_epoch)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
            date_epoch,
        ],
    )
    .map_err(|e| format!("Failed to upsert message: {e}"))?;
    Ok(())
}

/// Delete all messages in a folder (used when UIDVALIDITY changes).
pub fn delete_folder_messages(conn: &Connection, folder: &str) -> Result<usize, String> {
    let deleted = conn
        .execute("DELETE FROM messages WHERE folder = ?1", params![folder])
        .map_err(|e| format!("Failed to delete folder messages: {e}"))?;
    Ok(deleted)
}

/// Return the highest cached UID for a folder, or 0 if no messages are cached.
pub fn max_uid(conn: &Connection, folder: &str) -> Result<u32, String> {
    conn.query_row(
        "SELECT COALESCE(MAX(uid), 0) FROM messages WHERE folder = ?1",
        params![folder],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to get max uid: {e}"))
}

/// Public wrapper to parse a date string to a Unix epoch timestamp.
/// Useful when other modules need to compute `date_epoch` from a raw date string.
pub fn parse_date_to_epoch_public(date: &str) -> i64 {
    parse_date_to_epoch(date)
}

/// Parse a date string to a Unix epoch timestamp (seconds).
/// Tries RFC2822 first (IMAP dates), then ISO 8601.
/// Returns 0 on parse failure.
fn parse_date_to_epoch(date: &str) -> i64 {
    if date.is_empty() {
        return 0;
    }
    // Try ISO 8601 format first (used in test fixtures and some IMAP servers).
    if let Ok(epoch) = parse_iso8601_to_epoch(date) {
        return epoch;
    }
    // Try mail-parser's RFC2822 date parsing (standard IMAP date format).
    if let Some(dt) = mail_parser::DateTime::parse_rfc822(date) {
        let epoch = datetime_to_epoch(&dt);
        // Sanity check: mail_parser can misparse non-RFC2822 dates.
        if dt.year >= 1970 && dt.month >= 1 && dt.month <= 12 && epoch > 0 {
            return epoch;
        }
    }
    0
}

/// Convert a mail_parser::DateTime to a Unix epoch timestamp.
fn datetime_to_epoch(dt: &mail_parser::DateTime) -> i64 {
    // Days from year 1970 to the start of the given year
    let year = dt.year as i64;
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    for y in (year..1970).rev() {
        days -= if is_leap_year(y) { 366 } else { 365 };
    }
    // Days from start of year to start of month
    let month_days: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let month = (dt.month as usize).saturating_sub(1).min(11);
    for (m, &d) in month_days.iter().enumerate().take(month) {
        days += d;
        if m == 1 && is_leap_year(year) {
            days += 1;
        }
    }
    days += (dt.day as i64) - 1;

    let secs = days * 86400 + (dt.hour as i64) * 3600 + (dt.minute as i64) * 60 + (dt.second as i64);
    // tz_before_gmt=true means the offset is negative (behind UTC)
    let tz_offset_secs = (dt.tz_hour as i64) * 3600 + (dt.tz_minute as i64) * 60;
    if dt.tz_before_gmt {
        secs + tz_offset_secs
    } else {
        secs - tz_offset_secs
    }
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Parse ISO 8601 dates like "2024-01-01T10:00:00Z"
fn parse_iso8601_to_epoch(s: &str) -> Result<i64, ()> {
    // Simple parser for YYYY-MM-DDTHH:MM:SSZ
    let s = s.trim();
    if s.len() < 19 {
        return Err(());
    }
    let year: i64 = s[0..4].parse().map_err(|_| ())?;
    let month: u32 = s[5..7].parse().map_err(|_| ())?;
    let day: u32 = s[8..10].parse().map_err(|_| ())?;
    let hour: u32 = s[11..13].parse().map_err(|_| ())?;
    let minute: u32 = s[14..16].parse().map_err(|_| ())?;
    let second: u32 = s[17..19].parse().map_err(|_| ())?;

    let dt = mail_parser::DateTime {
        year: year as u16,
        month: month as u8,
        day: day as u8,
        hour: hour as u8,
        minute: minute as u8,
        second: second as u8,
        tz_before_gmt: false,
        tz_hour: 0,
        tz_minute: 0,
    };
    Ok(datetime_to_epoch(&dt))
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
         ORDER BY date_epoch DESC
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

/// Cache a message body (HTML and/or plain text) along with attachment
/// metadata (as a JSON string) and the raw RFC-822 headers.
pub fn cache_message_body(
    conn: &Connection,
    folder: &str,
    uid: u32,
    html: Option<&str>,
    text: Option<&str>,
    attachments_json: Option<&str>,
    raw_headers: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "UPDATE messages
         SET body_html = ?1, body_text = ?2, body_cached = 1,
             attachments_json = ?3, raw_headers = ?4
         WHERE folder = ?5 AND uid = ?6",
        params![html, text, attachments_json, raw_headers, folder, uid],
    )
    .map_err(|e| format!("Failed to cache message body: {e}"))?;
    Ok(())
}

/// Cached body data including attachment metadata and raw headers.
pub struct CachedBody {
    pub html: Option<String>,
    pub text: Option<String>,
    pub attachments_json: Option<String>,
    pub raw_headers: Option<String>,
}

/// Return the cached body if `body_cached = 1`, otherwise `None`.
pub fn get_cached_body(
    conn: &Connection,
    folder: &str,
    uid: u32,
) -> Result<Option<CachedBody>, String> {
    let result = conn.query_row(
        "SELECT body_cached, body_html, body_text, attachments_json, raw_headers
         FROM messages
         WHERE folder = ?1 AND uid = ?2",
        params![folder, uid],
        |row| {
            let cached: i32 = row.get(0)?;
            if cached == 1 {
                Ok(Some(CachedBody {
                    html: row.get(1)?,
                    text: row.get(2)?,
                    attachments_json: row.get(3)?,
                    raw_headers: row.get(4)?,
                }))
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

/// Return a single message by folder and UID, or None if not found.
pub fn get_single_message(conn: &Connection, folder: &str, uid: u32) -> Result<Option<CachedMessage>, String> {
    let sql = format!(
        "SELECT {MSG_SELECT_COLS}
         FROM messages
         WHERE folder = ?1 AND uid = ?2"
    );

    let result = conn.query_row(&sql, params![folder, uid], |row| {
        row_to_cached_message(row)
    });

    match result {
        Ok(msg) => Ok(Some(msg)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get single message: {e}")),
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

/// Return all (uid, flags_csv) pairs for a folder, for reconciliation.
pub fn get_all_uids_and_flags(conn: &Connection, folder: &str) -> Result<Vec<(u32, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT uid, flags FROM messages WHERE folder = ?1")
        .map_err(|e| format!("Failed to prepare get_all_uids_and_flags: {e}"))?;
    let rows = stmt
        .query_map(params![folder], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Failed to query uids and flags: {e}"))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Failed to read uid/flags row: {e}"))?);
    }
    Ok(result)
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
    // Escape % and _ characters in the message_id to prevent LIKE injection.
    let escaped_target = target_message_id.replace("%", "\\%").replace("_", "\\_");
    let like_pattern = format!("%{}%", escaped_target);
    let sql = format!(
        "SELECT {MSG_SELECT_COLS}
         FROM messages
         WHERE message_id = ?1
            OR in_reply_to = ?1
            OR references_header LIKE ?2 ESCAPE '\\'
         ORDER BY date_epoch ASC"
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

/// Build a complete email thread by transitively walking the References chain.
///
/// Starting from `message_id` and the optional space-separated `references` string,
/// this function repeatedly queries related messages (via `get_thread_messages`) and
/// collects every message_id, in_reply_to, and references_header ID it encounters.
/// The process repeats until no new IDs are discovered (transitive closure).
///
/// The result is deduplicated by `(folder, uid)` and sorted by `date_epoch` ascending.
pub fn get_full_thread(
    conn: &Connection,
    message_id: &str,
    references: Option<&str>,
) -> Result<Vec<CachedMessage>, String> {
    use std::collections::HashSet;

    // Step 1: Seed the set of known message IDs.
    let mut known_ids: HashSet<String> = HashSet::new();
    known_ids.insert(message_id.to_string());
    if let Some(refs) = references {
        for id in refs.split_whitespace() {
            if !id.is_empty() {
                known_ids.insert(id.to_string());
            }
        }
    }

    // Step 2-4: Iteratively query until no new IDs are found.
    let mut processed_ids: HashSet<String> = HashSet::new();
    let mut all_messages: Vec<CachedMessage> = Vec::new();

    loop {
        // Find IDs we haven't queried yet.
        let to_query: Vec<String> = known_ids
            .difference(&processed_ids)
            .cloned()
            .collect();

        if to_query.is_empty() {
            break;
        }

        for id in &to_query {
            processed_ids.insert(id.clone());

            let found = get_thread_messages(conn, id)?;
            for msg in found {
                // Extract IDs from each found message and add to known set.
                if let Some(ref mid) = msg.message_id
                    && !mid.is_empty()
                {
                    known_ids.insert(mid.clone());
                }
                if let Some(ref irt) = msg.in_reply_to
                    && !irt.is_empty()
                {
                    known_ids.insert(irt.clone());
                }
                if let Some(ref refs) = msg.references_header {
                    for r in refs.split_whitespace() {
                        if !r.is_empty() {
                            known_ids.insert(r.to_string());
                        }
                    }
                }
                all_messages.push(msg);
            }
        }
    }

    // Step 5: Deduplicate by (folder, uid) pair.
    let mut seen: HashSet<(String, u32)> = HashSet::new();
    all_messages.retain(|msg| seen.insert((msg.folder.clone(), msg.uid)));

    // Step 6: Sort by date_epoch ascending.
    all_messages.sort_by_key(|msg| parse_date_to_epoch(&msg.date));

    Ok(all_messages)
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
    fn test_parse_date_to_epoch() {
        let e1 = parse_date_to_epoch("2024-01-01T10:00:00Z");
        let e2 = parse_date_to_epoch("2024-01-02T10:00:00Z");
        let e3 = parse_date_to_epoch("2024-01-03T10:00:00Z");
        assert!(e1 > 0, "epoch for date 1 should be > 0, got {e1}");
        assert!(e2 > e1, "date 2 ({e2}) should be after date 1 ({e1})");
        assert!(e3 > e2, "date 3 ({e3}) should be after date 2 ({e2})");

        // RFC2822 format.
        let e4 = parse_date_to_epoch("Mon, 1 Jan 2024 10:00:00 +0000");
        assert!(e4 > 0, "epoch for rfc2822 date should be > 0, got {e4}");
        assert_eq!(e1, e4, "ISO and RFC2822 should produce same epoch");

        // Empty returns 0.
        assert_eq!(parse_date_to_epoch(""), 0);
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

        cache_message_body(
            &conn, "INBOX", 1,
            Some("<h1>Hello</h1>"), Some("Hello"),
            Some(r#"[{"id":"0","filename":"test.pdf","content_type":"application/pdf","size":1024,"content_id":null}]"#),
            Some("From: alice@example.com"),
        )
            .unwrap();

        let body = get_cached_body(&conn, "INBOX", 1).unwrap();
        assert!(body.is_some());
        let cached = body.unwrap();
        assert_eq!(cached.html.unwrap(), "<h1>Hello</h1>");
        assert_eq!(cached.text.unwrap(), "Hello");
        assert!(cached.attachments_json.unwrap().contains("test.pdf"));
        assert_eq!(cached.raw_headers.unwrap(), "From: alice@example.com");
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

    #[test]
    fn test_get_full_thread_walks_references_chain() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        // Message 1: original
        upsert_message(&conn, "INBOX", 1, Some("<a@ex>"), None, None,
            "Hello", "alice@ex", "Alice", "[]", "[]", "2024-01-01T10:00:00Z", "", 100, false, "").unwrap();
        // Message 2: reply to 1
        upsert_message(&conn, "INBOX", 2, Some("<b@ex>"), Some("<a@ex>"), Some("<a@ex>"),
            "Re: Hello", "bob@ex", "Bob", "[]", "[]", "2024-01-02T10:00:00Z", "", 100, false, "").unwrap();
        // Message 3: reply to 2, references both
        upsert_message(&conn, "INBOX", 3, Some("<c@ex>"), Some("<b@ex>"), Some("<a@ex> <b@ex>"),
            "Re: Re: Hello", "carol@ex", "Carol", "[]", "[]", "2024-01-03T10:00:00Z", "", 100, false, "").unwrap();

        // Starting from message 2, should find the entire thread (1, 2, 3).
        let thread = get_full_thread(&conn, "<b@ex>", Some("<a@ex>")).unwrap();
        assert_eq!(thread.len(), 3);
        assert_eq!(thread[0].uid, 1);
        assert_eq!(thread[1].uid, 2);
        assert_eq!(thread[2].uid, 3);
    }

    #[test]
    fn test_get_full_thread_from_leaf_message() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        upsert_message(&conn, "INBOX", 1, Some("<root@ex>"), None, None,
            "Start", "alice@ex", "Alice", "[]", "[]", "2024-01-01T10:00:00Z", "", 100, false, "").unwrap();
        upsert_message(&conn, "INBOX", 2, Some("<mid@ex>"), Some("<root@ex>"), Some("<root@ex>"),
            "Re: Start", "bob@ex", "Bob", "[]", "[]", "2024-01-02T10:00:00Z", "", 100, false, "").unwrap();
        upsert_message(&conn, "INBOX", 3, Some("<leaf@ex>"), Some("<mid@ex>"), Some("<root@ex> <mid@ex>"),
            "Re: Re: Start", "carol@ex", "Carol", "[]", "[]", "2024-01-03T10:00:00Z", "", 100, false, "").unwrap();

        // Starting from the leaf message, should still find entire thread.
        let thread = get_full_thread(&conn, "<leaf@ex>", Some("<root@ex> <mid@ex>")).unwrap();
        assert_eq!(thread.len(), 3);
    }

    #[test]
    fn test_get_full_thread_single_message() {
        let conn = open_test_db();
        ensure_folder(&conn, "INBOX");

        upsert_message(&conn, "INBOX", 1, Some("<solo@ex>"), None, None,
            "Solo", "alice@ex", "Alice", "[]", "[]", "2024-01-01T10:00:00Z", "", 100, false, "").unwrap();

        let thread = get_full_thread(&conn, "<solo@ex>", None).unwrap();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].uid, 1);
    }
}
