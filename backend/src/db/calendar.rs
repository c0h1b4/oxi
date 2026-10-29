use serde::{Deserialize, Serialize};
use sqlx::{Row, any::AnyRow};
use uuid::Uuid;

use super::pool::{UserDb, now};

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// A calendar event record, mirroring the `calendar_events` table.
#[derive(Debug, Clone, Serialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub start_time: String,
    pub end_time: String,
    pub all_day: bool,
    pub recurrence_rule: Option<String>,
    pub attendees: String,
    pub organizer: String,
    pub status: String,
    pub source: String,
    pub source_uid: Option<String>,
    pub source_message_uid: Option<i64>,
    pub source_message_folder: Option<String>,
    pub meeting_url: Option<String>,
    pub color: Option<String>,
    pub reminder_minutes: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Fields for creating a new calendar event.
#[derive(Debug, Deserialize)]
pub struct CreateEvent {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub location: String,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub all_day: bool,
    pub recurrence_rule: Option<String>,
    #[serde(default = "default_attendees")]
    pub attendees: String,
    #[serde(default)]
    pub organizer: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub source_uid: Option<String>,
    pub source_message_uid: Option<i64>,
    pub source_message_folder: Option<String>,
    pub meeting_url: Option<String>,
    pub color: Option<String>,
    pub reminder_minutes: Option<i64>,
}

fn default_attendees() -> String {
    "[]".to_string()
}

fn default_status() -> String {
    "confirmed".to_string()
}

fn default_source() -> String {
    "manual".to_string()
}

/// Fields for updating an existing calendar event. All optional.
#[derive(Debug, Deserialize)]
pub struct UpdateEvent {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub all_day: Option<bool>,
    pub recurrence_rule: Option<String>,
    pub attendees: Option<String>,
    pub organizer: Option<String>,
    pub status: Option<String>,
    pub meeting_url: Option<String>,
    pub color: Option<String>,
    pub reminder_minutes: Option<i64>,
}

/// Calendar settings record, mirroring the `calendar_settings` table.
#[derive(Debug, Clone, Serialize)]
pub struct CalendarSettings {
    pub default_view: String,
    pub week_starts_on: i64,
    pub working_hours_start: String,
    pub working_hours_end: String,
    pub time_format: String,
    pub zoom_link: String,
    pub teams_link: String,
    pub meet_link: String,
    pub caldav_url: String,
    pub caldav_username: String,
    pub caldav_enabled: bool,
    pub updated_at: String,
}

/// Fields for updating calendar settings. All optional.
#[derive(Debug, Deserialize)]
pub struct UpdateCalendarSettings {
    pub default_view: Option<String>,
    pub week_starts_on: Option<i64>,
    pub working_hours_start: Option<String>,
    pub working_hours_end: Option<String>,
    pub time_format: Option<String>,
    pub zoom_link: Option<String>,
    pub teams_link: Option<String>,
    pub meet_link: Option<String>,
    pub caldav_url: Option<String>,
    pub caldav_username: Option<String>,
    pub caldav_password: Option<String>,
    pub caldav_enabled: Option<bool>,
}

/// A meeting template record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingTemplate {
    pub id: i64,
    pub name: String,
    pub url_template: String,
    pub icon: String,
    pub is_default: bool,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

const EVENT_SELECT_COLS: &str = "id, title, description, location, start_time, end_time, \
    all_day, recurrence_rule, attendees, organizer, status, source, source_uid, \
    source_message_uid, source_message_folder, meeting_url, color, reminder_minutes, \
    created_at, updated_at";

fn row_to_event(row: &AnyRow) -> Result<CalendarEvent, sqlx::Error> {
    let all_day_int: i64 = row.try_get(6)?;
    Ok(CalendarEvent {
        id: row.try_get(0)?,
        title: row.try_get(1)?,
        description: row.try_get(2)?,
        location: row.try_get(3)?,
        start_time: row.try_get(4)?,
        end_time: row.try_get(5)?,
        all_day: all_day_int != 0,
        recurrence_rule: row.try_get(7)?,
        attendees: row.try_get(8)?,
        organizer: row.try_get(9)?,
        status: row.try_get(10)?,
        source: row.try_get(11)?,
        source_uid: row.try_get(12)?,
        source_message_uid: row.try_get(13)?,
        source_message_folder: row.try_get(14)?,
        meeting_url: row.try_get(15)?,
        color: row.try_get(16)?,
        reminder_minutes: row.try_get(17)?,
        created_at: row.try_get(18)?,
        updated_at: row.try_get(19)?,
    })
}

fn row_to_settings(row: &AnyRow) -> Result<CalendarSettings, sqlx::Error> {
    let caldav_enabled_int: i64 = row.try_get(10)?;
    Ok(CalendarSettings {
        default_view: row.try_get(0)?,
        week_starts_on: row.try_get(1)?,
        working_hours_start: row.try_get(2)?,
        working_hours_end: row.try_get(3)?,
        time_format: row.try_get(4)?,
        zoom_link: row.try_get(5)?,
        teams_link: row.try_get(6)?,
        meet_link: row.try_get(7)?,
        caldav_url: row.try_get(8)?,
        caldav_username: row.try_get(9)?,
        caldav_enabled: caldav_enabled_int != 0,
        updated_at: row.try_get(11)?,
    })
}

fn row_to_template(row: &AnyRow) -> Result<MeetingTemplate, sqlx::Error> {
    let is_default_int: i64 = row.try_get(4)?;
    Ok(MeetingTemplate {
        id: row.try_get(0)?,
        name: row.try_get(1)?,
        url_template: row.try_get(2)?,
        icon: row.try_get(3)?,
        is_default: is_default_int != 0,
        created_at: row.try_get(5)?,
    })
}

/// List events overlapping the inclusive time range.
pub async fn list_events(
    db: &UserDb,
    start: &str,
    end: &str,
) -> Result<Vec<CalendarEvent>, String> {
    let sql = format!(
        "SELECT {EVENT_SELECT_COLS} FROM calendar_events
        WHERE user_hash = $1 AND end_time >= $2 AND start_time <= $3 ORDER BY start_time ASC"
    );
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(start)
        .bind(end)
        .fetch_all(&mut *connection)
        .await
        .map_err(|e| format!("Failed to query events: {e}"))?;
    rows.iter()
        .map(row_to_event)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read event row: {e}"))
}

pub async fn get_event(db: &UserDb, id: &str) -> Result<Option<CalendarEvent>, String> {
    let sql =
        format!("SELECT {EVENT_SELECT_COLS} FROM calendar_events WHERE user_hash = $1 AND id = $2");
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(id)
        .fetch_optional(&mut *connection)
        .await
        .and_then(|row| row.as_ref().map(row_to_event).transpose())
        .map_err(|e| format!("Failed to get event: {e}"))
}

pub async fn create_event(db: &UserDb, data: &CreateEvent) -> Result<CalendarEvent, String> {
    let id = Uuid::new_v4().to_string();
    let timestamp = now();
    let mut connection = db.acquire().await?;
    sqlx::query("INSERT INTO calendar_events
        (user_hash, id, title, description, location, start_time, end_time, all_day,
         recurrence_rule, attendees, organizer, status, source, source_uid, source_message_uid,
         source_message_folder, meeting_url, color, reminder_minutes, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $20)")
        .bind(db.user_hash()).bind(&id).bind(&data.title).bind(&data.description).bind(&data.location)
        .bind(&data.start_time).bind(&data.end_time).bind(i64::from(data.all_day))
        .bind(&data.recurrence_rule).bind(&data.attendees).bind(&data.organizer).bind(&data.status)
        .bind(&data.source).bind(&data.source_uid).bind(data.source_message_uid)
        .bind(&data.source_message_folder).bind(&data.meeting_url).bind(&data.color)
        .bind(data.reminder_minutes).bind(timestamp).execute(&mut *connection).await
        .map_err(|e| format!("Failed to create event: {e}"))?;
    drop(connection);
    get_event(db, &id)
        .await?
        .ok_or_else(|| "Event not found after insert".to_string())
}

pub async fn update_event(
    db: &UserDb,
    id: &str,
    data: &UpdateEvent,
) -> Result<Option<CalendarEvent>, String> {
    if data.title.is_none()
        && data.description.is_none()
        && data.location.is_none()
        && data.start_time.is_none()
        && data.end_time.is_none()
        && data.all_day.is_none()
        && data.recurrence_rule.is_none()
        && data.attendees.is_none()
        && data.organizer.is_none()
        && data.status.is_none()
        && data.meeting_url.is_none()
        && data.color.is_none()
        && data.reminder_minutes.is_none()
    {
        return get_event(db, id).await;
    }
    let mut connection = db.acquire().await?;
    let updated = sqlx::query(
        "UPDATE calendar_events SET
        title = COALESCE($1, title), description = COALESCE($2, description),
        location = COALESCE($3, location), start_time = COALESCE($4, start_time),
        end_time = COALESCE($5, end_time), all_day = COALESCE($6, all_day),
        recurrence_rule = COALESCE($7, recurrence_rule), attendees = COALESCE($8, attendees),
        organizer = COALESCE($9, organizer), status = COALESCE($10, status),
        meeting_url = COALESCE($11, meeting_url), color = COALESCE($12, color),
        reminder_minutes = COALESCE($13, reminder_minutes), updated_at = $14
        WHERE user_hash = $15 AND id = $16",
    )
    .bind(&data.title)
    .bind(&data.description)
    .bind(&data.location)
    .bind(&data.start_time)
    .bind(&data.end_time)
    .bind(data.all_day.map(i64::from))
    .bind(&data.recurrence_rule)
    .bind(&data.attendees)
    .bind(&data.organizer)
    .bind(&data.status)
    .bind(&data.meeting_url)
    .bind(&data.color)
    .bind(data.reminder_minutes)
    .bind(now())
    .bind(db.user_hash())
    .bind(id)
    .execute(&mut *connection)
    .await
    .map_err(|e| format!("Failed to update event: {e}"))?;
    drop(connection);
    if updated.rows_affected() == 0 {
        return Ok(None);
    }
    get_event(db, id).await
}

pub async fn delete_event(db: &UserDb, id: &str) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM calendar_events WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(|e| format!("Failed to delete event: {e}"))
}

pub async fn find_event_by_source_uid(
    db: &UserDb,
    source_uid: &str,
) -> Result<Option<CalendarEvent>, String> {
    let sql = format!(
        "SELECT {EVENT_SELECT_COLS} FROM calendar_events WHERE user_hash = $1 AND source_uid = $2"
    );
    let mut connection = db.acquire().await?;
    sqlx::query(&sql)
        .bind(db.user_hash())
        .bind(source_uid)
        .fetch_optional(&mut *connection)
        .await
        .and_then(|row| row.as_ref().map(row_to_event).transpose())
        .map_err(|e| format!("Failed to find event by source_uid: {e}"))
}

const SETTINGS_SELECT_COLS: &str = "default_view, week_starts_on, working_hours_start, \
    working_hours_end, time_format, zoom_link, teams_link, meet_link, caldav_url, \
    caldav_username, caldav_enabled, updated_at";

/// Get calendar settings, inserting a default row for this user if needed.
pub async fn get_calendar_settings(db: &UserDb) -> Result<CalendarSettings, String> {
    let mut connection = db.acquire().await?;
    let insert = db.sql(
        "INSERT INTO calendar_settings (user_hash, id) VALUES ($1, 1) ON CONFLICT(user_hash) DO NOTHING",
        "INSERT INTO calendar_settings (user_hash) VALUES ($1) ON CONFLICT(user_hash) DO NOTHING",
    );
    sqlx::query(insert)
        .bind(db.user_hash())
        .execute(&mut *connection)
        .await
        .map_err(|e| format!("Failed to ensure calendar_settings row: {e}"))?;
    let sql = format!("SELECT {SETTINGS_SELECT_COLS} FROM calendar_settings WHERE user_hash = $1");
    let row = sqlx::query(&sql)
        .bind(db.user_hash())
        .fetch_one(&mut *connection)
        .await
        .map_err(|e| format!("Failed to get calendar settings: {e}"))?;
    row_to_settings(&row).map_err(|e| format!("Failed to read calendar settings: {e}"))
}

pub async fn update_calendar_settings(
    db: &UserDb,
    data: &UpdateCalendarSettings,
) -> Result<CalendarSettings, String> {
    get_calendar_settings(db).await?;
    let changed = data.default_view.is_some()
        || data.week_starts_on.is_some()
        || data.working_hours_start.is_some()
        || data.working_hours_end.is_some()
        || data.time_format.is_some()
        || data.zoom_link.is_some()
        || data.teams_link.is_some()
        || data.meet_link.is_some()
        || data.caldav_url.is_some()
        || data.caldav_username.is_some()
        || data.caldav_password.is_some()
        || data.caldav_enabled.is_some();
    if changed {
        let mut connection = db.acquire().await?;
        sqlx::query("UPDATE calendar_settings SET
            default_view = COALESCE($1, default_view), week_starts_on = COALESCE($2, week_starts_on),
            working_hours_start = COALESCE($3, working_hours_start), working_hours_end = COALESCE($4, working_hours_end),
            time_format = COALESCE($5, time_format), zoom_link = COALESCE($6, zoom_link),
            teams_link = COALESCE($7, teams_link), meet_link = COALESCE($8, meet_link),
            caldav_url = COALESCE($9, caldav_url), caldav_username = COALESCE($10, caldav_username),
            caldav_password = COALESCE($11, caldav_password), caldav_enabled = COALESCE($12, caldav_enabled),
            updated_at = $13 WHERE user_hash = $14")
            .bind(&data.default_view).bind(data.week_starts_on).bind(&data.working_hours_start)
            .bind(&data.working_hours_end).bind(&data.time_format).bind(&data.zoom_link)
            .bind(&data.teams_link).bind(&data.meet_link).bind(&data.caldav_url).bind(&data.caldav_username)
            .bind(&data.caldav_password).bind(data.caldav_enabled.map(i64::from)).bind(now()).bind(db.user_hash())
            .execute(&mut *connection).await.map_err(|e| format!("Failed to update calendar settings: {e}"))?;
        drop(connection);
    }
    get_calendar_settings(db).await
}

pub async fn list_meeting_templates(db: &UserDb) -> Result<Vec<MeetingTemplate>, String> {
    let mut connection = db.acquire().await?;
    let rows = sqlx::query(
        "SELECT id, name, url_template, icon, is_default, created_at
        FROM meeting_templates WHERE user_hash = $1 ORDER BY is_default DESC, name ASC",
    )
    .bind(db.user_hash())
    .fetch_all(&mut *connection)
    .await
    .map_err(|e| format!("Failed to query meeting templates: {e}"))?;
    rows.iter()
        .map(row_to_template)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to read template row: {e}"))
}

pub async fn create_meeting_template(
    db: &UserDb,
    name: &str,
    url_template: &str,
    icon: &str,
) -> Result<MeetingTemplate, String> {
    let mut connection = db.acquire().await?;
    let row = sqlx::query(
        "INSERT INTO meeting_templates (user_hash, name, url_template, icon, created_at)
        VALUES ($1, $2, $3, $4, $5) RETURNING id, name, url_template, icon, is_default, created_at",
    )
    .bind(db.user_hash())
    .bind(name)
    .bind(url_template)
    .bind(icon)
    .bind(now())
    .fetch_one(&mut *connection)
    .await
    .map_err(|e| format!("Failed to create meeting template: {e}"))?;
    row_to_template(&row).map_err(|e| format!("Failed to read created template: {e}"))
}

pub async fn delete_meeting_template(db: &UserDb, id: i64) -> Result<bool, String> {
    let mut connection = db.acquire().await?;
    sqlx::query("DELETE FROM meeting_templates WHERE user_hash = $1 AND id = $2")
        .bind(db.user_hash())
        .bind(id)
        .execute(&mut *connection)
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(|e| format!("Failed to delete meeting template: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[tokio::test]
    async fn test_create_and_get_event() {
        let conn = open_test_db().await;
        let data = CreateEvent {
            title: "Team Standup".to_string(),
            description: "Daily standup meeting".to_string(),
            location: "Room A".to_string(),
            start_time: "2025-03-06T09:00:00Z".to_string(),
            end_time: "2025-03-06T09:30:00Z".to_string(),
            all_day: false,
            recurrence_rule: None,
            attendees: "[]".to_string(),
            organizer: "alice@example.com".to_string(),
            status: "confirmed".to_string(),
            source: "manual".to_string(),
            source_uid: None,
            source_message_uid: None,
            source_message_folder: None,
            meeting_url: Some("https://zoom.us/j/123".to_string()),
            color: Some("blue".to_string()),
            reminder_minutes: Some(15),
        };

        let event = create_event(&conn, &data).await.unwrap();
        assert_eq!(event.title, "Team Standup");
        assert_eq!(event.location, "Room A");
        assert!(!event.all_day);

        let fetched = get_event(&conn, &event.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().title, "Team Standup");
    }

    #[tokio::test]
    async fn test_list_events_range() {
        let conn = open_test_db().await;

        let early = CreateEvent {
            title: "Early".to_string(),
            description: String::new(),
            location: String::new(),
            start_time: "2025-03-01T09:00:00Z".to_string(),
            end_time: "2025-03-01T10:00:00Z".to_string(),
            all_day: false,
            recurrence_rule: None,
            attendees: "[]".to_string(),
            organizer: String::new(),
            status: "confirmed".to_string(),
            source: "manual".to_string(),
            source_uid: None,
            source_message_uid: None,
            source_message_folder: None,
            meeting_url: None,
            color: None,
            reminder_minutes: None,
        };

        let late = CreateEvent {
            title: "Late".to_string(),
            start_time: "2025-04-01T09:00:00Z".to_string(),
            end_time: "2025-04-01T10:00:00Z".to_string(),
            ..early.clone_fields()
        };

        create_event(&conn, &early).await.unwrap();
        create_event(&conn, &late).await.unwrap();

        let march = list_events(&conn, "2025-03-01T00:00:00Z", "2025-03-31T23:59:59Z")
            .await
            .unwrap();
        assert_eq!(march.len(), 1);
        assert_eq!(march[0].title, "Early");

        let all = list_events(&conn, "2025-01-01T00:00:00Z", "2025-12-31T23:59:59Z")
            .await
            .unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_event() {
        let db = open_test_db().await;
        let conn = db.begin_write().await.unwrap();
        let data = CreateEvent {
            title: "Original".to_string(),
            description: String::new(),
            location: String::new(),
            start_time: "2025-03-06T09:00:00Z".to_string(),
            end_time: "2025-03-06T10:00:00Z".to_string(),
            all_day: false,
            recurrence_rule: None,
            attendees: "[]".to_string(),
            organizer: String::new(),
            status: "confirmed".to_string(),
            source: "manual".to_string(),
            source_uid: None,
            source_message_uid: None,
            source_message_folder: None,
            meeting_url: None,
            color: None,
            reminder_minutes: None,
        };

        let event = create_event(&conn, &data).await.unwrap();
        let update = UpdateEvent {
            title: Some("Updated".to_string()),
            description: None,
            location: None,
            start_time: None,
            end_time: None,
            all_day: Some(true),
            recurrence_rule: None,
            attendees: None,
            organizer: None,
            status: None,
            meeting_url: None,
            color: None,
            reminder_minutes: Some(30),
        };

        let updated = update_event(&conn, &event.id, &update)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.title, "Updated");
        assert!(updated.all_day);
        assert_eq!(updated.reminder_minutes, Some(30));
        assert_eq!(updated.start_time, data.start_time);
        conn.commit().await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_event() {
        let conn = open_test_db().await;
        let data = CreateEvent {
            title: "To Delete".to_string(),
            description: String::new(),
            location: String::new(),
            start_time: "2025-03-06T09:00:00Z".to_string(),
            end_time: "2025-03-06T10:00:00Z".to_string(),
            all_day: false,
            recurrence_rule: None,
            attendees: "[]".to_string(),
            organizer: String::new(),
            status: "confirmed".to_string(),
            source: "manual".to_string(),
            source_uid: None,
            source_message_uid: None,
            source_message_folder: None,
            meeting_url: None,
            color: None,
            reminder_minutes: None,
        };

        let event = create_event(&conn, &data).await.unwrap();
        assert!(delete_event(&conn, &event.id).await.unwrap());
        assert!(!delete_event(&conn, &event.id).await.unwrap());
        assert!(get_event(&conn, &event.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_event_by_source_uid() {
        let conn = open_test_db().await;
        let data = CreateEvent {
            title: "Imported".to_string(),
            description: String::new(),
            location: String::new(),
            start_time: "2025-03-06T09:00:00Z".to_string(),
            end_time: "2025-03-06T10:00:00Z".to_string(),
            all_day: false,
            recurrence_rule: None,
            attendees: "[]".to_string(),
            organizer: String::new(),
            status: "confirmed".to_string(),
            source: "ics".to_string(),
            source_uid: Some("unique-uid-123@example.com".to_string()),
            source_message_uid: None,
            source_message_folder: None,
            meeting_url: None,
            color: None,
            reminder_minutes: None,
        };

        create_event(&conn, &data).await.unwrap();

        let found = find_event_by_source_uid(&conn, "unique-uid-123@example.com")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Imported");

        let missing = find_event_by_source_uid(&conn, "nonexistent@example.com")
            .await
            .unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_calendar_settings() {
        let db = open_test_db().await;
        let conn = db.begin_write().await.unwrap();

        let settings = get_calendar_settings(&conn).await.unwrap();
        assert_eq!(settings.default_view, "month");
        assert_eq!(settings.week_starts_on, 0);
        assert_eq!(settings.time_format, "12h");

        let update = UpdateCalendarSettings {
            default_view: Some("week".to_string()),
            week_starts_on: Some(1),
            working_hours_start: None,
            working_hours_end: None,
            time_format: Some("24h".to_string()),
            zoom_link: None,
            teams_link: None,
            meet_link: None,
            caldav_url: None,
            caldav_username: None,
            caldav_password: None,
            caldav_enabled: None,
        };

        let updated = update_calendar_settings(&conn, &update).await.unwrap();
        assert_eq!(updated.default_view, "week");
        assert_eq!(updated.week_starts_on, 1);
        assert_eq!(updated.time_format, "24h");
        conn.commit().await.unwrap();
    }

    #[tokio::test]
    async fn test_meeting_templates() {
        let conn = open_test_db().await;

        let template = create_meeting_template(&conn, "Zoom", "https://zoom.us/j/{id}", "video")
            .await
            .unwrap();
        assert_eq!(template.name, "Zoom");

        let all = list_meeting_templates(&conn).await.unwrap();
        assert_eq!(all.len(), 1);

        assert!(delete_meeting_template(&conn, template.id).await.unwrap());
        assert!(!delete_meeting_template(&conn, template.id).await.unwrap());

        let all = list_meeting_templates(&conn).await.unwrap();
        assert_eq!(all.len(), 0);
    }

    // Helper to create CreateEvent variants without full Clone impl
    impl CreateEvent {
        fn clone_fields(&self) -> Self {
            CreateEvent {
                title: String::new(),
                description: self.description.clone(),
                location: self.location.clone(),
                start_time: String::new(),
                end_time: String::new(),
                all_day: self.all_day,
                recurrence_rule: self.recurrence_rule.clone(),
                attendees: self.attendees.clone(),
                organizer: self.organizer.clone(),
                status: self.status.clone(),
                source: self.source.clone(),
                source_uid: self.source_uid.clone(),
                source_message_uid: self.source_message_uid,
                source_message_folder: self.source_message_folder.clone(),
                meeting_url: self.meeting_url.clone(),
                color: self.color.clone(),
                reminder_minutes: self.reminder_minutes,
            }
        }
    }
}
