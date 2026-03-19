use std::sync::Arc;

use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::session::SessionState;
use crate::config::AppConfig;
use crate::db;
use crate::error::AppError;
use crate::search::engine::{SearchEngine, SearchQuery};

// ---------------------------------------------------------------------------
// Search query parser
// ---------------------------------------------------------------------------

/// Intermediate result of parsing structured operators out of a query string.
struct ParsedQuery {
    text: String,
    subject_only: Option<String>,
    folder: Option<String>,
    from: Option<String>,
    to: Option<String>,
    date_from: Option<i64>,
    date_to: Option<i64>,
    has_attachment: Option<bool>,
}

/// Parse a search query string extracting structured operators like `from:`,
/// `to:`, `subject:`, `in:`/`folder:`, `date:`, `after:`, `before:`, and
/// `has:attachment`. Remaining text becomes the free-text query.
///
/// Supports quoted values: `from:"John Doe"`, `subject:"meeting notes"`.
fn parse_search_query(input: &str) -> ParsedQuery {
    let mut text_parts: Vec<String> = Vec::new();
    let mut subject_only: Option<String> = None;
    let mut folder: Option<String> = None;
    let mut from: Option<String> = None;
    let mut to: Option<String> = None;
    let mut date_from: Option<i64> = None;
    let mut date_to: Option<i64> = None;
    let mut has_attachment: Option<bool> = None;

    let tokens = tokenize_query(input);

    for token in tokens {
        if let Some((operator, value)) = split_operator(&token) {
            let op = operator.to_lowercase();
            match op.as_str() {
                "from" => from = Some(value),
                "to" => to = Some(value),
                "subject" => subject_only = Some(value),
                "in" | "folder" => folder = Some(value),
                "date" => {
                    if let Some((start, end)) = parse_date_range(&value) {
                        date_from = Some(start);
                        date_to = Some(end);
                    }
                }
                "after" => {
                    if let Some(epoch) = parse_date_start(&value) {
                        date_from = Some(epoch);
                    }
                }
                "before" => {
                    if let Some(epoch) = parse_date_start(&value) {
                        date_to = Some(epoch);
                    }
                }
                "has" if value.eq_ignore_ascii_case("attachment") => {
                    has_attachment = Some(true);
                }
                _ => {
                    // Unknown operator, treat as plain text
                    text_parts.push(token);
                }
            }
        } else {
            text_parts.push(token);
        }
    }

    ParsedQuery {
        text: text_parts.join(" "),
        subject_only,
        folder,
        from,
        to,
        date_from,
        date_to,
        has_attachment,
    }
}

/// Tokenize a query string, respecting quoted values attached to operators.
/// For example: `from:"John Doe" hello world` yields
/// `["from:John Doe", "hello", "world"]`.
fn tokenize_query(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut current = String::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            chars.next();
        } else if ch == '"' {
            // Start of a quoted section
            chars.next(); // consume opening quote
            while let Some(&qch) = chars.peek() {
                if qch == '"' {
                    chars.next(); // consume closing quote
                    break;
                }
                current.push(qch);
                chars.next();
            }
        } else {
            current.push(ch);
            chars.next();
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Split a token into (operator, value) if it matches `operator:value` pattern.
fn split_operator(token: &str) -> Option<(String, String)> {
    if let Some(colon_pos) = token.find(':') {
        let op = &token[..colon_pos];
        let val = &token[colon_pos + 1..];
        // Only treat as operator if the operator part is alphabetic and value is non-empty
        if !op.is_empty() && op.chars().all(|c| c.is_ascii_alphabetic()) && !val.is_empty() {
            return Some((op.to_string(), val.to_string()));
        }
    }
    None
}

/// Parse a `YYYY-MM-DD` date string into a start-of-day and end-of-day epoch
/// (UTC) pair.
fn parse_date_range(date_str: &str) -> Option<(i64, i64)> {
    let start = parse_date_start(date_str)?;
    let end = start + 86_400 - 1; // end of day
    Some((start, end))
}

/// Parse a `YYYY-MM-DD` date string into a start-of-day epoch (UTC).
fn parse_date_start(date_str: &str) -> Option<i64> {
    let parts: Vec<&str> = date_str.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i64 = parts[0].parse().ok()?;
    let month: i64 = parts[1].parse().ok()?;
    let day: i64 = parts[2].parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Convert to Unix epoch using a simplified calculation (UTC).
    // Days from year 1970 to the given date.
    let mut total_days: i64 = 0;

    // Days from full years
    for y in 1970..year {
        total_days += if is_leap_year(y) { 366 } else { 365 };
    }

    // Days from full months in the target year
    let days_in_months = [31, 28 + i64::from(is_leap_year(year)), 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for days in days_in_months.iter().take((month - 1) as usize) {
        total_days += days;
    }

    total_days += day - 1;

    Some(total_days * 86_400)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

/// Query parameters for `GET /api/search`.
#[derive(Deserialize)]
pub struct SearchParams {
    /// The search text (required, must not be empty).
    pub q: Option<String>,
    /// Optional folder filter.
    pub folder: Option<String>,
    /// Optional from address filter.
    pub from: Option<String>,
    /// Optional to address filter.
    pub to: Option<String>,
    /// Optional date range start (Unix epoch seconds).
    pub date_from: Option<i64>,
    /// Optional date range end (Unix epoch seconds).
    pub date_to: Option<i64>,
    /// Optional attachment filter.
    pub has_attachment: Option<bool>,
    /// Maximum number of results (default 50).
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination (default 0).
    #[serde(default)]
    pub offset: usize,
    /// Sort order: "date_desc" (default) or "date_asc".
    #[serde(default = "default_sort")]
    pub sort: String,
}

fn default_sort() -> String {
    "date_desc".to_string()
}

fn default_limit() -> usize {
    10_000
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// Response envelope for `GET /api/search`.
#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total_count: usize,
    pub query: String,
}

/// A single search result item enriched with message metadata from SQLite.
#[derive(Serialize)]
pub struct SearchResultItem {
    pub uid: u32,
    pub folder: String,
    pub score: f32,
    pub subject: String,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: String,
    pub date: String,
    pub flags: String,
    pub has_attachments: bool,
    pub snippet: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// `GET /api/search?q=text&folder=INBOX&from=alice&date_from=...&date_to=...&has_attachment=true&limit=50&offset=0`
///
/// Searches the user's messages using both SQLite (for header fields) and
/// Tantivy (for body text). Results are merged and deduplicated.
pub async fn search_messages(
    Extension(session): Extension<SessionState>,
    Extension(config): Extension<Arc<AppConfig>>,
    Extension(search_engine): Extension<Arc<SearchEngine>>,
    Query(params): Query<SearchParams>,
) -> Result<Response, AppError> {
    // Validate that `q` is provided and non-empty.
    let query_text = params
        .q
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_string();

    if query_text.is_empty() {
        return Err(AppError::BadRequest(
            "Query parameter 'q' is required and must not be empty".to_string(),
        ));
    }

    let sort_order = params.sort.clone();

    // Parse structured operators from the query string.
    let parsed = parse_search_query(&query_text);

    let folder = params.folder.or(parsed.folder);
    let from = params.from.or(parsed.from);
    let to = params.to.or(parsed.to);
    let date_from = params.date_from.or(parsed.date_from);
    let date_to = params.date_to.or(parsed.date_to);
    let has_attachment = params.has_attachment.or(parsed.has_attachment);

    // Open the user's SQLite database.
    let conn = db::pool::open_user_db(&config.data_dir, &session.user_hash)
        .map_err(|e| AppError::InternalError(format!("Database error: {e}")))?;

    // Primary search: SQLite LIKE across all cached messages (subject, from, to).
    let sqlite_results = db::messages::search_messages_sqlite(
        &conn,
        &parsed.text,
        folder.as_deref(),
        from.as_deref(),
        to.as_deref(),
        date_from,
        date_to,
        has_attachment,
        params.limit,
    )
    .map_err(|e| AppError::InternalError(format!("Search error: {e}")))?;

    // Collect SQLite results as SearchResultItems.
    let mut seen = std::collections::HashSet::new();
    let mut results: Vec<SearchResultItem> = sqlite_results
        .into_iter()
        .map(|msg| {
            seen.insert((msg.folder.clone(), msg.uid));
            SearchResultItem {
                uid: msg.uid,
                folder: msg.folder,
                score: 1.0,
                subject: msg.subject,
                from_address: msg.from_address,
                from_name: msg.from_name,
                to_addresses: msg.to_addresses,
                date: msg.date,
                flags: msg.flags,
                has_attachments: msg.has_attachments,
                snippet: msg.snippet,
            }
        })
        .collect();

    // Secondary search: Tantivy for body text matches not found by SQLite.
    if !parsed.text.is_empty()
        && let Ok(user_index) = search_engine.open_user_index(&session.user_hash)
        && let Ok((tantivy_results, _)) = user_index.search(&SearchQuery {
            text: parsed.text,
            subject_only: parsed.subject_only,
            folder,
            from,
            to,
            date_from,
            date_to,
            has_attachment,
            limit: params.limit,
            offset: params.offset,
        })
    {
        for sr in &tantivy_results {
            if seen.contains(&(sr.folder.clone(), sr.uid)) {
                continue;
            }
            if let Ok(Some(msg)) = db::messages::get_single_message(&conn, &sr.folder, sr.uid) {
                seen.insert((msg.folder.clone(), msg.uid));
                results.push(SearchResultItem {
                    uid: msg.uid,
                    folder: msg.folder,
                    score: sr.score,
                    subject: msg.subject,
                    from_address: msg.from_address,
                    from_name: msg.from_name,
                    to_addresses: msg.to_addresses,
                    date: msg.date,
                    flags: msg.flags,
                    has_attachments: msg.has_attachments,
                    snippet: if sr.snippet.is_empty() {
                        msg.snippet
                    } else {
                        sr.snippet.clone()
                    },
                });
            }
        }
    }

    // Sort results by date.
    let sort_asc = sort_order == "date_asc";
    results.sort_by(|a, b| {
        let da = crate::db::messages::parse_date_to_epoch_public(&a.date);
        let db_val = crate::db::messages::parse_date_to_epoch_public(&b.date);
        if sort_asc { da.cmp(&db_val) } else { db_val.cmp(&da) }
    });

    Ok(Json(SearchResponse {
        total_count: results.len(),
        results,
        query: query_text,
    })
    .into_response())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text() {
        let p = parse_search_query("hello world");
        assert_eq!(p.text, "hello world");
        assert!(p.from.is_none());
        assert!(p.to.is_none());
        assert!(p.subject_only.is_none());
        assert!(p.folder.is_none());
    }

    #[test]
    fn parse_from_operator() {
        let p = parse_search_query("from:alice@example.com hello");
        assert_eq!(p.text, "hello");
        assert_eq!(p.from.as_deref(), Some("alice@example.com"));
    }

    #[test]
    fn parse_to_operator() {
        let p = parse_search_query("to:bob@example.com updates");
        assert_eq!(p.text, "updates");
        assert_eq!(p.to.as_deref(), Some("bob@example.com"));
    }

    #[test]
    fn parse_subject_operator() {
        let p = parse_search_query("subject:meeting notes");
        assert_eq!(p.subject_only.as_deref(), Some("meeting"));
        assert_eq!(p.text, "notes");
    }

    #[test]
    fn parse_subject_quoted() {
        let p = parse_search_query("subject:\"meeting notes\" extra");
        assert_eq!(p.subject_only.as_deref(), Some("meeting notes"));
        assert_eq!(p.text, "extra");
    }

    #[test]
    fn parse_folder_operators() {
        let p = parse_search_query("in:Sent hello");
        assert_eq!(p.folder.as_deref(), Some("Sent"));
        assert_eq!(p.text, "hello");

        let p2 = parse_search_query("folder:INBOX test");
        assert_eq!(p2.folder.as_deref(), Some("INBOX"));
    }

    #[test]
    fn parse_date_operator() {
        let p = parse_search_query("date:2024-01-15 news");
        assert_eq!(p.text, "news");
        assert!(p.date_from.is_some());
        assert!(p.date_to.is_some());
        // 2024-01-15 00:00:00 UTC = 1705276800
        assert_eq!(p.date_from, Some(1_705_276_800));
        assert_eq!(p.date_to, Some(1_705_276_800 + 86_399));
    }

    #[test]
    fn parse_after_before() {
        let p = parse_search_query("after:2024-01-01 before:2024-06-01 hello");
        assert_eq!(p.text, "hello");
        // 2024-01-01 = 1704067200
        assert_eq!(p.date_from, Some(1_704_067_200));
        // 2024-06-01 = 1717200000
        assert_eq!(p.date_to, Some(1_717_200_000));
    }

    #[test]
    fn parse_has_attachment() {
        let p = parse_search_query("has:attachment report");
        assert_eq!(p.text, "report");
        assert_eq!(p.has_attachment, Some(true));
    }

    #[test]
    fn parse_multiple_operators() {
        let p = parse_search_query("from:alice@test.com to:bob@test.com in:INBOX has:attachment meeting");
        assert_eq!(p.from.as_deref(), Some("alice@test.com"));
        assert_eq!(p.to.as_deref(), Some("bob@test.com"));
        assert_eq!(p.folder.as_deref(), Some("INBOX"));
        assert_eq!(p.has_attachment, Some(true));
        assert_eq!(p.text, "meeting");
    }

    #[test]
    fn parse_quoted_from() {
        let p = parse_search_query("from:\"John Doe\" hello");
        assert_eq!(p.from.as_deref(), Some("John Doe"));
        assert_eq!(p.text, "hello");
    }

    #[test]
    fn parse_unknown_operator_kept_as_text() {
        let p = parse_search_query("foo:bar hello");
        assert_eq!(p.text, "foo:bar hello");
    }

    #[test]
    fn parse_empty_value_not_operator() {
        // "from:" with no value should not be treated as an operator
        let p = parse_search_query("hello from:");
        assert_eq!(p.text, "hello from:");
        assert!(p.from.is_none());
    }

    #[test]
    fn date_parsing_basic() {
        // 2024-01-01 00:00:00 UTC
        assert_eq!(parse_date_start("2024-01-01"), Some(1_704_067_200));
        // Invalid date
        assert_eq!(parse_date_start("not-a-date"), None);
        assert_eq!(parse_date_start("2024-13-01"), None);
    }
}
