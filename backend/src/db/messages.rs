use super::pool::UserDb;
use serde::Serialize;
use sqlx::Row;

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
    pub reaction: Option<String>,
}

fn row_to_cached_message(row: sqlx::any::AnyRow) -> Result<CachedMessage, String> {
    let integer = |index| -> Result<u32, String> {
        u32::try_from(row.try_get::<i64, _>(index).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    };
    Ok(CachedMessage {
        uid: integer(0)?,
        folder: row.try_get(1).map_err(|e| e.to_string())?,
        message_id: row.try_get(2).map_err(|e| e.to_string())?,
        in_reply_to: row.try_get(3).map_err(|e| e.to_string())?,
        references_header: row.try_get(4).map_err(|e| e.to_string())?,
        subject: row.try_get(5).map_err(|e| e.to_string())?,
        from_address: row.try_get(6).map_err(|e| e.to_string())?,
        from_name: row.try_get(7).map_err(|e| e.to_string())?,
        to_addresses: row.try_get(8).map_err(|e| e.to_string())?,
        cc_addresses: row.try_get(9).map_err(|e| e.to_string())?,
        date: row.try_get(10).map_err(|e| e.to_string())?,
        flags: row.try_get(11).map_err(|e| e.to_string())?,
        snippet: row.try_get(14).map_err(|e| e.to_string())?,
        reaction: row.try_get(15).map_err(|e| e.to_string())?,
        has_attachments: row.try_get::<i64, _>(12).map_err(|e| e.to_string())? != 0,
        size: integer(13)?,
    })
}

/// The SELECT column list used by all queries that return `CachedMessage`.
const MSG_SELECT_COLS: &str = "uid, folder, message_id, in_reply_to, references_header,
     subject, from_address, from_name, to_addresses, cc_addresses,
     date, flags, has_attachments, size, snippet, reaction";

#[allow(clippy::too_many_arguments)]
pub async fn upsert_message(
    db: &UserDb,
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
    reaction: Option<&str>,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("INSERT INTO messages
        (user_hash,uid,folder,message_id,in_reply_to,references_header,subject,from_address,
         from_name,to_addresses,cc_addresses,date,flags,size,has_attachments,snippet,date_epoch,reaction)
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
        ON CONFLICT(user_hash,uid,folder) DO UPDATE SET message_id=excluded.message_id,
         in_reply_to=excluded.in_reply_to,references_header=excluded.references_header,
         subject=excluded.subject,from_address=excluded.from_address,from_name=excluded.from_name,
         to_addresses=excluded.to_addresses,cc_addresses=excluded.cc_addresses,date=excluded.date,
         flags=excluded.flags,size=excluded.size,has_attachments=excluded.has_attachments,
         snippet=excluded.snippet,date_epoch=excluded.date_epoch,reaction=excluded.reaction")
        .bind(db.user_hash()).bind(i64::from(uid)).bind(folder).bind(message_id).bind(in_reply_to)
        .bind(references_header).bind(subject).bind(from_address).bind(from_name).bind(to_json)
        .bind(cc_json).bind(date).bind(flags_csv).bind(i64::from(size)).bind(i64::from(has_attachments))
        .bind(snippet).bind(parse_date_to_epoch(date)).bind(reaction)
        .execute(&mut *connection).await.map_err(|e| format!("Failed to upsert message: {e}"))?;
    Ok(())
}

/// Invalidate older network reads in the same transaction as the deletion.
pub async fn remove_after_mutation(db: &UserDb, folder: &str, uid: u32) -> Result<(), String> {
    let transaction = db.begin_write().await?;
    delete_message(&transaction, folder, uid).await?;
    super::header_sync::invalidate(&transaction).await?;
    super::folders::invalidate_folder_freshness(&transaction, folder).await?;
    transaction.commit().await
}

pub async fn delete_folder_messages(db: &UserDb, folder: &str) -> Result<usize, String> {
    let mut connection = db.acquire().await?;
    let deleted = sqlx::query("DELETE FROM messages WHERE user_hash=$1 AND folder=$2")
        .bind(db.user_hash())
        .bind(folder)
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
    usize::try_from(deleted).map_err(|e| e.to_string())
}

pub async fn max_uid(db: &UserDb, folder: &str) -> Result<u32, String> {
    let mut connection = db.acquire().await?;
    let row =
        sqlx::query("SELECT COALESCE(MAX(uid),0) FROM messages WHERE user_hash=$1 AND folder=$2")
            .bind(db.user_hash())
            .bind(folder)
            .fetch_one(&mut *connection)
            .await
            .map_err(|e| e.to_string())?;
    u32::try_from(row.try_get::<i64, _>(0).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
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

    let secs =
        days * 86400 + (dt.hour as i64) * 3600 + (dt.minute as i64) * 60 + (dt.second as i64);
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

/// Suggest distinct subjects with literal, case-insensitive matching.
pub async fn suggest_subjects(db: &UserDb, query: &str) -> Result<Vec<String>, String> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let sql=db.sql(
        "SELECT MIN(subject) FROM messages WHERE user_hash=$1 AND trim(subject)!='' AND instr(lower(subject),lower($2))>0 GROUP BY lower(subject) ORDER BY MAX(date_epoch) DESC,lower(subject) LIMIT 8",
        "SELECT MIN(subject) FROM messages WHERE user_hash=$1 AND trim(subject)!='' AND strpos(lower(subject),lower($2))>0 GROUP BY lower(subject) ORDER BY MAX(date_epoch) DESC,lower(subject) LIMIT 8");
    let mut connection = db.acquire().await?;
    sqlx::query(sql)
        .bind(db.user_hash())
        .bind(query)
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|row| row.try_get(0).map_err(|e| e.to_string()))
        .collect()
}

pub async fn get_messages(
    db: &UserDb,
    folder: &str,
    page: u32,
    per_page: u32,
) -> Result<Vec<CachedMessage>, String> {
    let offset = i64::try_from(u64::from(page) * u64::from(per_page))
        .map_err(|_| "Message page offset exceeds database range".to_string())?;
    let sql = format!(
        "SELECT {MSG_SELECT_COLS} FROM messages WHERE user_hash=$1 AND folder=$2 ORDER BY date_epoch DESC LIMIT $3 OFFSET $4"
    );
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(folder)
        .bind(i64::from(per_page))
        .bind(offset)
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(row_to_cached_message)
        .collect()
}

pub async fn count_messages(db: &UserDb, folder: &str) -> Result<u32, String> {
    let mut connection = db.acquire().await?;
    let row = sqlx::query("SELECT COUNT(*) FROM messages WHERE user_hash=$1 AND folder=$2")
        .bind(db.user_hash())
        .bind(folder)
        .fetch_one(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    u32::try_from(row.try_get::<i64, _>(0).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

pub async fn update_message_flags(
    db: &UserDb,
    folder: &str,
    uid: u32,
    flags_csv: &str,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE messages SET flags=$1 WHERE user_hash=$2 AND folder=$3 AND uid=$4")
        .bind(flags_csv)
        .bind(db.user_hash())
        .bind(folder)
        .bind(i64::from(uid))
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn cache_message_body(
    db: &UserDb,
    folder: &str,
    uid: u32,
    html: Option<&str>,
    text: Option<&str>,
    attachments_json: Option<&str>,
    raw_headers: Option<&str>,
    email_theme: Option<i32>,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE messages SET body_html=$1,body_text=$2,body_cached=1,attachments_json=$3,raw_headers=$4,email_theme=$5 WHERE user_hash=$6 AND folder=$7 AND uid=$8")
        .bind(html).bind(text).bind(attachments_json).bind(raw_headers).bind(email_theme.map(i64::from))
        .bind(db.user_hash()).bind(folder).bind(i64::from(uid))
        .execute(&mut *connection).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn update_email_theme(
    db: &UserDb,
    folder: &str,
    uid: u32,
    email_theme: i32,
) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("UPDATE messages SET email_theme=$1 WHERE user_hash=$2 AND folder=$3 AND uid=$4")
        .bind(i64::from(email_theme))
        .bind(db.user_hash())
        .bind(folder)
        .bind(i64::from(uid))
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Cached body data including attachment metadata and raw headers.
pub struct CachedBody {
    pub html: Option<String>,
    pub text: Option<String>,
    pub attachments_json: Option<String>,
    pub raw_headers: Option<String>,
    pub email_theme: Option<i32>,
}

pub async fn get_cached_body(
    db: &UserDb,
    folder: &str,
    uid: u32,
) -> Result<Option<CachedBody>, String> {
    let mut connection = db.acquire().await?;
    let row=sqlx::query("SELECT body_html,body_text,attachments_json,raw_headers,email_theme FROM messages WHERE user_hash=$1 AND folder=$2 AND uid=$3 AND body_cached=1")
        .bind(db.user_hash()).bind(folder).bind(i64::from(uid)).fetch_optional(&mut *connection).await.map_err(|e| e.to_string())?;
    row.map(|row| {
        Ok(CachedBody {
            html: row.try_get(0).map_err(|e| e.to_string())?,
            text: row.try_get(1).map_err(|e| e.to_string())?,
            attachments_json: row.try_get(2).map_err(|e| e.to_string())?,
            raw_headers: row.try_get(3).map_err(|e| e.to_string())?,
            email_theme: row
                .try_get::<Option<i64>, _>(4)
                .map_err(|e| e.to_string())?
                .map(i32::try_from)
                .transpose()
                .map_err(|e| e.to_string())?,
        })
    })
    .transpose()
}

pub async fn get_single_message(
    db: &UserDb,
    folder: &str,
    uid: u32,
) -> Result<Option<CachedMessage>, String> {
    let sql = format!(
        "SELECT {MSG_SELECT_COLS} FROM messages WHERE user_hash=$1 AND folder=$2 AND uid=$3"
    );
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(folder)
        .bind(i64::from(uid))
        .fetch_optional(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .map(row_to_cached_message)
        .transpose()
}

pub async fn delete_message(db: &UserDb, folder: &str, uid: u32) -> Result<(), String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM messages WHERE user_hash=$1 AND folder=$2 AND uid=$3")
        .bind(db.user_hash())
        .bind(folder)
        .bind(i64::from(uid))
        .execute(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_unindexed_messages(db: &UserDb, limit: u32) -> Result<Vec<(String, u32)>, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("SELECT folder,uid FROM messages WHERE user_hash=$1 AND body_cached=0 ORDER BY date_epoch DESC LIMIT $2")
        .bind(db.user_hash()).bind(i64::from(limit)).fetch_all(&mut *connection).await.map_err(|e| e.to_string())?
        .into_iter().map(|row| Ok((row.try_get(0).map_err(|e| e.to_string())?,
            u32::try_from(row.try_get::<i64,_>(1).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?))).collect()
}

pub async fn get_all_uids_and_flags(
    db: &UserDb,
    folder: &str,
) -> Result<Vec<(u32, String)>, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("SELECT uid,flags FROM messages WHERE user_hash=$1 AND folder=$2")
        .bind(db.user_hash())
        .bind(folder)
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|row| {
            Ok((
                u32::try_from(row.try_get::<i64, _>(0).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
                row.try_get(1).map_err(|e| e.to_string())?,
            ))
        })
        .collect()
}

pub async fn get_thread_messages(
    db: &UserDb,
    target_message_id: &str,
) -> Result<Vec<CachedMessage>, String> {
    let escaped = target_message_id
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    let pattern = format!("%{escaped}%");
    let sql=format!("SELECT {MSG_SELECT_COLS} FROM messages WHERE user_hash=$1 AND
        (message_id=$2 OR in_reply_to=$2 OR references_header LIKE $3 ESCAPE '\\') ORDER BY date_epoch ASC");
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(target_message_id)
        .bind(pattern)
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(row_to_cached_message)
        .collect()
}

/// Build a complete email thread by transitively walking the References chain.
///
/// Starting from `message_id` and the optional space-separated `references` string,
/// this function repeatedly queries related messages (via `get_thread_messages`) and
/// collects every message_id, in_reply_to, and references_header ID it encounters.
/// The process repeats until no new IDs are discovered (transitive closure).
///
/// The result is deduplicated by `(folder, uid)` and sorted by `date_epoch` ascending.
pub async fn get_full_thread(
    conn: &UserDb,
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
        let to_query: Vec<String> = known_ids.difference(&processed_ids).cloned().collect();

        if to_query.is_empty() {
            break;
        }

        for id in &to_query {
            processed_ids.insert(id.clone());

            let found = get_thread_messages(conn, id).await?;
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

/// Search the local cache on either backend. All filters remain tenant-scoped.
#[allow(clippy::too_many_arguments)]
pub async fn search_messages_sqlite(
    db: &UserDb,
    text: &str,
    folder: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    date_from: Option<i64>,
    date_to: Option<i64>,
    has_attachment: Option<bool>,
    limit: usize,
) -> Result<Vec<CachedMessage>, String> {
    let sql = format!(
        "SELECT {MSG_SELECT_COLS} FROM messages WHERE user_hash=$1
        AND LOWER(folder) NOT IN ('trash','spam','junk')
        AND ($2='' OR LOWER(subject) LIKE LOWER($3) OR LOWER(from_name) LIKE LOWER($3)
             OR LOWER(from_address) LIKE LOWER($3) OR LOWER(to_addresses) LIKE LOWER($3))
        AND (CAST($4 AS TEXT) IS NULL OR folder=$4)
        AND (CAST($5 AS TEXT) IS NULL OR LOWER(from_address)=LOWER($5))
        AND (CAST($6 AS TEXT) IS NULL OR LOWER(from_name) LIKE LOWER($6))
        AND (CAST($7 AS TEXT) IS NULL OR LOWER(to_addresses) LIKE LOWER($7))
        AND (CAST($8 AS BIGINT) IS NULL OR date_epoch >= $8)
        AND (CAST($9 AS BIGINT) IS NULL OR date_epoch <= $9)
        AND (CAST($10 AS BIGINT) IS NULL OR has_attachments=$10)
        ORDER BY date_epoch DESC LIMIT $11"
    );
    let from_address = from.filter(|value| value.contains('@'));
    let from_name = from
        .filter(|value| !value.contains('@'))
        .map(|value| format!("%{value}%"));
    let limit = i64::try_from(limit).map_err(|e| e.to_string())?;
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(text)
        .bind(format!("%{text}%"))
        .bind(folder)
        .bind(from_address)
        .bind(from_name)
        .bind(to.map(|value| format!("%{value}%")))
        .bind(date_from)
        .bind(date_to)
        .bind(has_attachment.map(i64::from))
        .bind(limit)
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(row_to_cached_message)
        .collect()
}

/// None distinguishes an empty cache from cached zero-byte messages.
pub async fn cached_storage_bytes(db: &UserDb) -> Result<Option<u64>, String> {
    let mut connection = db.acquire().await?;
    let row = sqlx::query("SELECT CAST(SUM(size) AS BIGINT) FROM messages WHERE user_hash=$1")
        .bind(db.user_hash())
        .fetch_one(&mut *connection)
        .await
        .map_err(|e| e.to_string())?;
    row.try_get::<Option<i64>, _>(0)
        .map_err(|e| e.to_string())?
        .map(u64::try_from)
        .transpose()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::folders::upsert_folder;
    use crate::db::pool::open_test_db;

    /// Helper: insert a folder so that the foreign key constraint is satisfied.
    async fn ensure_folder(conn: &UserDb, name: &str) {
        upsert_folder(conn, name, Some("/"), None, "", true, 0, 0, 1, 0)
            .await
            .unwrap();
    }

    /// Helper: insert a sample message.
    async fn insert_sample(conn: &UserDb, folder: &str, uid: u32, date: &str) {
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
            None,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn subject_suggestions_match_literal_text_deduplicate_and_limit() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;
        ensure_folder(&conn, "Archive").await;
        for uid in 1..=12 {
            insert_sample(&conn, "INBOX", uid, &format!("2024-01-{uid:02}T10:00:00Z")).await;
        }
        insert_sample(&conn, "Archive", 12, "2024-01-12T10:00:00Z").await;
        let suggestions = suggest_subjects(&conn, "  SUBJECT  ").await.unwrap();
        assert_eq!(suggestions.len(), 8);
        assert_eq!(suggestions[0], "Subject 12");
        assert_eq!(suggestions[7], "Subject 5");
        assert!(suggest_subjects(&conn, " ").await.unwrap().is_empty());
        assert!(suggest_subjects(&conn, "s").await.unwrap().is_empty());
        assert!(suggest_subjects(&conn, "%_").await.unwrap().is_empty());
        {
            let mut connection = conn.acquire().await.unwrap();
            sqlx::query("UPDATE messages SET subject='100%_done' WHERE user_hash=$1 AND uid=1")
                .bind(conn.user_hash())
                .execute(&mut *connection)
                .await
                .unwrap();
        }
        assert_eq!(
            suggest_subjects(&conn, "%_").await.unwrap(),
            vec!["100%_done"]
        );
        let other_account = open_test_db().await;
        assert!(
            suggest_subjects(&other_account, "Subject")
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn cached_storage_tracks_all_folders_and_deletions() {
        let conn = open_test_db().await;
        assert_eq!(cached_storage_bytes(&conn).await.unwrap(), None);
        ensure_folder(&conn, "INBOX").await;
        ensure_folder(&conn, "Archive").await;
        insert_sample(&conn, "INBOX", 1, "2024-01-01").await;
        insert_sample(&conn, "Archive", 1, "2024-01-01").await;
        assert_eq!(cached_storage_bytes(&conn).await.unwrap(), Some(2048));
        delete_message(&conn, "INBOX", 1).await.unwrap();
        assert_eq!(cached_storage_bytes(&conn).await.unwrap(), Some(1024));
    }

    #[tokio::test]
    async fn test_parse_date_to_epoch() {
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

    #[tokio::test]
    async fn test_upsert_and_get_messages() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;
        insert_sample(&conn, "INBOX", 2, "2024-01-02T10:00:00Z").await;
        insert_sample(&conn, "INBOX", 3, "2024-01-03T10:00:00Z").await;

        let msgs = get_messages(&conn, "INBOX", 0, 10).await.unwrap();
        assert_eq!(msgs.len(), 3);

        // Should be date DESC: uid 3, 2, 1.
        assert_eq!(msgs[0].uid, 3);
        assert_eq!(msgs[1].uid, 2);
        assert_eq!(msgs[2].uid, 1);
    }

    #[tokio::test]
    async fn test_pagination_no_overlap() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        for uid in 1..=5 {
            insert_sample(
                &conn,
                "INBOX",
                uid,
                &format!("2024-01-{:02}T10:00:00Z", uid),
            )
            .await;
        }

        let page0 = get_messages(&conn, "INBOX", 0, 2).await.unwrap();
        let page1 = get_messages(&conn, "INBOX", 1, 2).await.unwrap();
        let page2 = get_messages(&conn, "INBOX", 2, 2).await.unwrap();

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

    #[tokio::test]
    async fn test_count_messages() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        assert_eq!(count_messages(&conn, "INBOX").await.unwrap(), 0);

        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;
        insert_sample(&conn, "INBOX", 2, "2024-01-02T10:00:00Z").await;

        assert_eq!(count_messages(&conn, "INBOX").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_update_message_flags() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;

        update_message_flags(&conn, "INBOX", 1, "\\Seen,\\Flagged")
            .await
            .unwrap();

        let msgs = get_messages(&conn, "INBOX", 0, 10).await.unwrap();
        assert_eq!(msgs[0].flags, "\\Seen,\\Flagged");
    }

    #[tokio::test]
    async fn test_cache_and_get_body_initially_none() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;

        // Body should not be cached yet.
        let body = get_cached_body(&conn, "INBOX", 1).await.unwrap();
        assert!(body.is_none());
    }

    #[tokio::test]
    async fn test_cache_and_get_body_after_caching() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;

        cache_message_body(
            &conn, "INBOX", 1,
            Some("<h1>Hello</h1>"), Some("Hello"),
            Some(r#"[{"id":"0","filename":"test.pdf","content_type":"application/pdf","size":1024,"content_id":null}]"#),
            Some("From: alice@example.com"),
            Some(0),
        ).await
            .unwrap();

        let body = get_cached_body(&conn, "INBOX", 1).await.unwrap();
        assert!(body.is_some());
        let cached = body.unwrap();
        assert_eq!(cached.html.unwrap(), "<h1>Hello</h1>");
        assert_eq!(cached.text.unwrap(), "Hello");
        assert!(cached.attachments_json.unwrap().contains("test.pdf"));
        assert_eq!(cached.raw_headers.unwrap(), "From: alice@example.com");
        assert_eq!(cached.email_theme, Some(0));
    }

    #[tokio::test]
    async fn test_update_email_theme() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;

        update_email_theme(&conn, "INBOX", 1, 2).await.unwrap();

        let body = get_cached_body(&conn, "INBOX", 1).await.unwrap();
        assert!(body.is_none());

        cache_message_body(
            &conn,
            "INBOX",
            1,
            Some("<h1>Test</h1>"),
            Some("Test"),
            None,
            None,
            Some(0),
        )
        .await
        .unwrap();

        update_email_theme(&conn, "INBOX", 1, 1).await.unwrap();

        let cached = get_cached_body(&conn, "INBOX", 1).await.unwrap().unwrap();
        assert_eq!(cached.email_theme, Some(1));
    }

    #[tokio::test]
    async fn test_delete_message() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;
        insert_sample(&conn, "INBOX", 1, "2024-01-01T10:00:00Z").await;
        insert_sample(&conn, "INBOX", 2, "2024-01-02T10:00:00Z").await;

        delete_message(&conn, "INBOX", 1).await.unwrap();

        let msgs = get_messages(&conn, "INBOX", 0, 10).await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].uid, 2);
    }

    #[tokio::test]
    async fn test_get_thread_messages_by_message_id() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        // Original message.
        upsert_message(
            &conn,
            "INBOX",
            1,
            Some("<thread-1@example.com>"),
            None,
            None,
            "Hello",
            "alice@example.com",
            "Alice",
            "[]",
            "[]",
            "2024-01-01T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        // Reply referencing original via in_reply_to.
        upsert_message(
            &conn,
            "INBOX",
            2,
            Some("<reply-1@example.com>"),
            Some("<thread-1@example.com>"),
            None,
            "Re: Hello",
            "bob@example.com",
            "Bob",
            "[]",
            "[]",
            "2024-01-02T10:00:00Z",
            "",
            200,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        let thread = get_thread_messages(&conn, "<thread-1@example.com>")
            .await
            .unwrap();
        assert_eq!(thread.len(), 2);
        // ASC order: uid 1 first, uid 2 second.
        assert_eq!(thread[0].uid, 1);
        assert_eq!(thread[1].uid, 2);
    }

    #[tokio::test]
    async fn test_get_thread_messages_by_references_header() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        // Original message.
        upsert_message(
            &conn,
            "INBOX",
            1,
            Some("<orig@example.com>"),
            None,
            None,
            "Hello",
            "alice@example.com",
            "Alice",
            "[]",
            "[]",
            "2024-01-01T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        // A message that references the original only via references_header.
        upsert_message(
            &conn,
            "INBOX",
            3,
            Some("<deep-reply@example.com>"),
            Some("<mid@example.com>"),
            Some("<orig@example.com> <mid@example.com>"),
            "Re: Re: Hello",
            "carol@example.com",
            "Carol",
            "[]",
            "[]",
            "2024-01-03T10:00:00Z",
            "",
            300,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        let thread = get_thread_messages(&conn, "<orig@example.com>")
            .await
            .unwrap();
        assert_eq!(thread.len(), 2);
        assert_eq!(thread[0].uid, 1); // matched by message_id
        assert_eq!(thread[1].uid, 3); // matched by references_header LIKE
    }

    #[tokio::test]
    async fn test_get_full_thread_walks_references_chain() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        // Message 1: original
        upsert_message(
            &conn,
            "INBOX",
            1,
            Some("<a@ex>"),
            None,
            None,
            "Hello",
            "alice@ex",
            "Alice",
            "[]",
            "[]",
            "2024-01-01T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();
        // Message 2: reply to 1
        upsert_message(
            &conn,
            "INBOX",
            2,
            Some("<b@ex>"),
            Some("<a@ex>"),
            Some("<a@ex>"),
            "Re: Hello",
            "bob@ex",
            "Bob",
            "[]",
            "[]",
            "2024-01-02T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();
        // Message 3: reply to 2, references both
        upsert_message(
            &conn,
            "INBOX",
            3,
            Some("<c@ex>"),
            Some("<b@ex>"),
            Some("<a@ex> <b@ex>"),
            "Re: Re: Hello",
            "carol@ex",
            "Carol",
            "[]",
            "[]",
            "2024-01-03T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        // Starting from message 2, should find the entire thread (1, 2, 3).
        let thread = get_full_thread(&conn, "<b@ex>", Some("<a@ex>"))
            .await
            .unwrap();
        assert_eq!(thread.len(), 3);
        assert_eq!(thread[0].uid, 1);
        assert_eq!(thread[1].uid, 2);
        assert_eq!(thread[2].uid, 3);
    }

    #[tokio::test]
    async fn test_get_full_thread_from_leaf_message() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        upsert_message(
            &conn,
            "INBOX",
            1,
            Some("<root@ex>"),
            None,
            None,
            "Start",
            "alice@ex",
            "Alice",
            "[]",
            "[]",
            "2024-01-01T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();
        upsert_message(
            &conn,
            "INBOX",
            2,
            Some("<mid@ex>"),
            Some("<root@ex>"),
            Some("<root@ex>"),
            "Re: Start",
            "bob@ex",
            "Bob",
            "[]",
            "[]",
            "2024-01-02T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();
        upsert_message(
            &conn,
            "INBOX",
            3,
            Some("<leaf@ex>"),
            Some("<mid@ex>"),
            Some("<root@ex> <mid@ex>"),
            "Re: Re: Start",
            "carol@ex",
            "Carol",
            "[]",
            "[]",
            "2024-01-03T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        // Starting from the leaf message, should still find entire thread.
        let thread = get_full_thread(&conn, "<leaf@ex>", Some("<root@ex> <mid@ex>"))
            .await
            .unwrap();
        assert_eq!(thread.len(), 3);
    }

    #[tokio::test]
    async fn test_get_full_thread_single_message() {
        let conn = open_test_db().await;
        ensure_folder(&conn, "INBOX").await;

        upsert_message(
            &conn,
            "INBOX",
            1,
            Some("<solo@ex>"),
            None,
            None,
            "Solo",
            "alice@ex",
            "Alice",
            "[]",
            "[]",
            "2024-01-01T10:00:00Z",
            "",
            100,
            false,
            "",
            None,
        )
        .await
        .unwrap();

        let thread = get_full_thread(&conn, "<solo@ex>", None).await.unwrap();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].uid, 1);
    }

    #[tokio::test]
    async fn header_upserts_preserve_bodies_and_tags_and_full_uid_range() {
        let db = open_test_db().await;
        ensure_folder(&db, "INBOX").await;
        insert_sample(&db, "INBOX", u32::MAX, "2024-01-01").await;
        cache_message_body(
            &db,
            "INBOX",
            u32::MAX,
            None,
            Some("cached"),
            None,
            None,
            None,
        )
        .await
        .unwrap();
        {
            let mut connection = db.acquire().await.unwrap();
            sqlx::query("INSERT INTO tags (user_hash,id,name) VALUES ($1,'tag','Tag')")
                .bind(db.user_hash())
                .execute(&mut *connection)
                .await
                .unwrap();
            sqlx::query("INSERT INTO message_tags (user_hash,tag_id,message_uid,message_folder) VALUES ($1,'tag',$2,'INBOX')")
                .bind(db.user_hash()).bind(i64::from(u32::MAX)).execute(&mut *connection).await.unwrap();
        }
        insert_sample(&db, "INBOX", u32::MAX, "2024-01-02").await;
        ensure_folder(&db, "INBOX").await;
        assert_eq!(max_uid(&db, "INBOX").await.unwrap(), u32::MAX);
        assert_eq!(
            get_cached_body(&db, "INBOX", u32::MAX)
                .await
                .unwrap()
                .unwrap()
                .text
                .as_deref(),
            Some("cached")
        );
        let mut connection = db.acquire().await.unwrap();
        let row = sqlx::query("SELECT COUNT(*) FROM message_tags WHERE user_hash=$1")
            .bind(db.user_hash())
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(row.try_get::<i64, _>(0).unwrap(), 1);
    }
}
