use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEvent> {
    let all_day_int: i32 = row.get(6)?;
    Ok(CalendarEvent {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        location: row.get(3)?,
        start_time: row.get(4)?,
        end_time: row.get(5)?,
        all_day: all_day_int != 0,
        recurrence_rule: row.get(7)?,
        attendees: row.get(8)?,
        organizer: row.get(9)?,
        status: row.get(10)?,
        source: row.get(11)?,
        source_uid: row.get(12)?,
        source_message_uid: row.get(13)?,
        source_message_folder: row.get(14)?,
        meeting_url: row.get(15)?,
        color: row.get(16)?,
        reminder_minutes: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

fn row_to_settings(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarSettings> {
    let caldav_enabled_int: i32 = row.get(10)?;
    Ok(CalendarSettings {
        default_view: row.get(0)?,
        week_starts_on: row.get(1)?,
        working_hours_start: row.get(2)?,
        working_hours_end: row.get(3)?,
        time_format: row.get(4)?,
        zoom_link: row.get(5)?,
        teams_link: row.get(6)?,
        meet_link: row.get(7)?,
        caldav_url: row.get(8)?,
        caldav_username: row.get(9)?,
        caldav_enabled: caldav_enabled_int != 0,
        updated_at: row.get(11)?,
    })
}

fn row_to_template(row: &rusqlite::Row<'_>) -> rusqlite::Result<MeetingTemplate> {
    let is_default_int: i32 = row.get(4)?;
    Ok(MeetingTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        url_template: row.get(2)?,
        icon: row.get(3)?,
        is_default: is_default_int != 0,
        created_at: row.get(5)?,
    })
}

// ---------------------------------------------------------------------------
// Public API — Events
// ---------------------------------------------------------------------------

/// List events in the given time range (inclusive start, inclusive end).
pub fn list_events(
    conn: &Connection,
    start: &str,
    end: &str,
) -> Result<Vec<CalendarEvent>, String> {
    let sql = format!(
        "SELECT {EVENT_SELECT_COLS} FROM calendar_events
         WHERE end_time >= ?1 AND start_time <= ?2
         ORDER BY start_time ASC"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare list_events: {e}"))?;
    let rows = stmt
        .query_map(params![start, end], row_to_event)
        .map_err(|e| format!("Failed to query events: {e}"))?;

    let mut events = Vec::new();
    for row in rows {
        events.push(row.map_err(|e| format!("Failed to read event row: {e}"))?);
    }
    Ok(events)
}

/// Get a single event by ID.
pub fn get_event(conn: &Connection, id: &str) -> Result<Option<CalendarEvent>, String> {
    let sql = format!("SELECT {EVENT_SELECT_COLS} FROM calendar_events WHERE id = ?1");
    let result = conn.query_row(&sql, params![id], row_to_event);

    match result {
        Ok(e) => Ok(Some(e)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to get event: {e}")),
    }
}

/// Create a new calendar event. Generates a UUID for the ID.
pub fn create_event(conn: &Connection, data: &CreateEvent) -> Result<CalendarEvent, String> {
    let id = Uuid::new_v4().to_string();
    let now: String = conn
        .query_row("SELECT datetime('now')", [], |row| row.get(0))
        .map_err(|e| format!("Failed to get current time: {e}"))?;

    conn.execute(
        "INSERT INTO calendar_events
            (id, title, description, location, start_time, end_time, all_day,
             recurrence_rule, attendees, organizer, status, source, source_uid,
             source_message_uid, source_message_folder, meeting_url, color,
             reminder_minutes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            id,
            data.title,
            data.description,
            data.location,
            data.start_time,
            data.end_time,
            data.all_day as i32,
            data.recurrence_rule,
            data.attendees,
            data.organizer,
            data.status,
            data.source,
            data.source_uid,
            data.source_message_uid,
            data.source_message_folder,
            data.meeting_url,
            data.color,
            data.reminder_minutes,
            now,
            now,
        ],
    )
    .map_err(|e| format!("Failed to create event: {e}"))?;

    get_event(conn, &id)?.ok_or_else(|| "Event not found after insert".to_string())
}

/// Update an existing calendar event. Returns the updated event, or None if not found.
pub fn update_event(
    conn: &Connection,
    id: &str,
    data: &UpdateEvent,
) -> Result<Option<CalendarEvent>, String> {
    // Build SET clauses dynamically for non-None fields.
    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    macro_rules! maybe_set {
        ($field:ident, $col:expr) => {
            if let Some(ref v) = data.$field {
                sets.push(format!("{} = ?{idx}", $col));
                values.push(Box::new(v.clone()));
                idx += 1;
            }
        };
    }

    maybe_set!(title, "title");
    maybe_set!(description, "description");
    maybe_set!(location, "location");
    maybe_set!(start_time, "start_time");
    maybe_set!(end_time, "end_time");
    maybe_set!(attendees, "attendees");
    maybe_set!(organizer, "organizer");
    maybe_set!(status, "status");
    maybe_set!(meeting_url, "meeting_url");
    maybe_set!(color, "color");
    maybe_set!(recurrence_rule, "recurrence_rule");

    if let Some(v) = data.all_day {
        sets.push(format!("all_day = ?{idx}"));
        values.push(Box::new(v as i32));
        idx += 1;
    }

    if let Some(v) = data.reminder_minutes {
        sets.push(format!("reminder_minutes = ?{idx}"));
        values.push(Box::new(v));
        idx += 1;
    }

    if sets.is_empty() {
        return get_event(conn, id);
    }

    sets.push("updated_at = datetime('now')".to_string());

    let sql = format!(
        "UPDATE calendar_events SET {} WHERE id = ?{idx}",
        sets.join(", ")
    );
    values.push(Box::new(id.to_string()));

    let params_refs: Vec<&dyn rusqlite::types::ToSql> = values.iter().map(|b| b.as_ref()).collect();
    let updated = conn
        .execute(&sql, params_refs.as_slice())
        .map_err(|e| format!("Failed to update event: {e}"))?;

    if updated == 0 {
        return Ok(None);
    }

    get_event(conn, id)
}

/// Delete an event by ID. Returns true if a row was deleted.
pub fn delete_event(conn: &Connection, id: &str) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM calendar_events WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete event: {e}"))?;
    Ok(deleted > 0)
}

/// Find an event by its source_uid (for deduplication during ICS import).
pub fn find_event_by_source_uid(
    conn: &Connection,
    source_uid: &str,
) -> Result<Option<CalendarEvent>, String> {
    let sql = format!(
        "SELECT {EVENT_SELECT_COLS} FROM calendar_events WHERE source_uid = ?1"
    );
    let result = conn.query_row(&sql, params![source_uid], row_to_event);

    match result {
        Ok(e) => Ok(Some(e)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to find event by source_uid: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Public API — Settings
// ---------------------------------------------------------------------------

const SETTINGS_SELECT_COLS: &str = "default_view, week_starts_on, working_hours_start, \
    working_hours_end, time_format, zoom_link, teams_link, meet_link, caldav_url, \
    caldav_username, caldav_enabled, updated_at";

/// Get calendar settings, inserting a default row if none exists.
pub fn get_calendar_settings(conn: &Connection) -> Result<CalendarSettings, String> {
    // Ensure the singleton row exists.
    conn.execute(
        "INSERT OR IGNORE INTO calendar_settings (id) VALUES (1)",
        [],
    )
    .map_err(|e| format!("Failed to ensure calendar_settings row: {e}"))?;

    let sql = format!(
        "SELECT {SETTINGS_SELECT_COLS} FROM calendar_settings WHERE id = 1"
    );
    conn.query_row(&sql, [], row_to_settings)
        .map_err(|e| format!("Failed to get calendar settings: {e}"))
}

/// Update calendar settings. Returns the updated settings.
pub fn update_calendar_settings(
    conn: &Connection,
    data: &UpdateCalendarSettings,
) -> Result<CalendarSettings, String> {
    // Ensure the singleton row exists.
    conn.execute(
        "INSERT OR IGNORE INTO calendar_settings (id) VALUES (1)",
        [],
    )
    .map_err(|e| format!("Failed to ensure calendar_settings row: {e}"))?;

    let mut sets = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1u32;

    macro_rules! maybe_set {
        ($field:ident, $col:expr) => {
            if let Some(ref v) = data.$field {
                sets.push(format!("{} = ?{idx}", $col));
                values.push(Box::new(v.clone()));
                idx += 1;
            }
        };
    }

    maybe_set!(default_view, "default_view");
    maybe_set!(working_hours_start, "working_hours_start");
    maybe_set!(working_hours_end, "working_hours_end");
    maybe_set!(time_format, "time_format");
    maybe_set!(zoom_link, "zoom_link");
    maybe_set!(teams_link, "teams_link");
    maybe_set!(meet_link, "meet_link");
    maybe_set!(caldav_url, "caldav_url");
    maybe_set!(caldav_username, "caldav_username");
    maybe_set!(caldav_password, "caldav_password");

    if let Some(v) = data.week_starts_on {
        sets.push(format!("week_starts_on = ?{idx}"));
        values.push(Box::new(v));
        idx += 1;
    }

    if let Some(v) = data.caldav_enabled {
        sets.push(format!("caldav_enabled = ?{idx}"));
        values.push(Box::new(v as i32));
        #[allow(unused_assignments)]
        {
            idx += 1;
        }
    }

    if !sets.is_empty() {
        sets.push("updated_at = datetime('now')".to_string());
        let sql = format!(
            "UPDATE calendar_settings SET {} WHERE id = 1",
            sets.join(", ")
        );
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|b| b.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(|e| format!("Failed to update calendar settings: {e}"))?;
    }

    get_calendar_settings(conn)
}

// ---------------------------------------------------------------------------
// Public API — Meeting Templates
// ---------------------------------------------------------------------------

/// List all meeting templates.
pub fn list_meeting_templates(conn: &Connection) -> Result<Vec<MeetingTemplate>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, url_template, icon, is_default, created_at
             FROM meeting_templates ORDER BY is_default DESC, name ASC",
        )
        .map_err(|e| format!("Failed to prepare list_meeting_templates: {e}"))?;
    let rows = stmt
        .query_map([], row_to_template)
        .map_err(|e| format!("Failed to query meeting templates: {e}"))?;

    let mut templates = Vec::new();
    for row in rows {
        templates.push(row.map_err(|e| format!("Failed to read template row: {e}"))?);
    }
    Ok(templates)
}

/// Create a new meeting template.
pub fn create_meeting_template(
    conn: &Connection,
    name: &str,
    url_template: &str,
    icon: &str,
) -> Result<MeetingTemplate, String> {
    conn.execute(
        "INSERT INTO meeting_templates (name, url_template, icon) VALUES (?1, ?2, ?3)",
        params![name, url_template, icon],
    )
    .map_err(|e| format!("Failed to create meeting template: {e}"))?;

    let id = conn.last_insert_rowid();
    let sql = "SELECT id, name, url_template, icon, is_default, created_at
               FROM meeting_templates WHERE id = ?1";
    conn.query_row(sql, params![id], row_to_template)
        .map_err(|e| format!("Failed to read created template: {e}"))
}

/// Delete a meeting template by ID. Returns true if a row was deleted.
pub fn delete_meeting_template(conn: &Connection, id: i64) -> Result<bool, String> {
    let deleted = conn
        .execute("DELETE FROM meeting_templates WHERE id = ?1", params![id])
        .map_err(|e| format!("Failed to delete meeting template: {e}"))?;
    Ok(deleted > 0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::open_test_db;

    #[test]
    fn test_create_and_get_event() {
        let conn = open_test_db();
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

        let event = create_event(&conn, &data).unwrap();
        assert_eq!(event.title, "Team Standup");
        assert_eq!(event.location, "Room A");
        assert!(!event.all_day);

        let fetched = get_event(&conn, &event.id).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().title, "Team Standup");
    }

    #[test]
    fn test_list_events_range() {
        let conn = open_test_db();

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

        create_event(&conn, &early).unwrap();
        create_event(&conn, &late).unwrap();

        let march = list_events(&conn, "2025-03-01T00:00:00Z", "2025-03-31T23:59:59Z").unwrap();
        assert_eq!(march.len(), 1);
        assert_eq!(march[0].title, "Early");

        let all = list_events(&conn, "2025-01-01T00:00:00Z", "2025-12-31T23:59:59Z").unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_update_event() {
        let conn = open_test_db();
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

        let event = create_event(&conn, &data).unwrap();
        let update = UpdateEvent {
            title: Some("Updated".to_string()),
            description: None,
            location: None,
            start_time: None,
            end_time: None,
            all_day: None,
            recurrence_rule: None,
            attendees: None,
            organizer: None,
            status: None,
            meeting_url: None,
            color: None,
            reminder_minutes: None,
        };

        let updated = update_event(&conn, &event.id, &update).unwrap();
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().title, "Updated");
    }

    #[test]
    fn test_delete_event() {
        let conn = open_test_db();
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

        let event = create_event(&conn, &data).unwrap();
        assert!(delete_event(&conn, &event.id).unwrap());
        assert!(!delete_event(&conn, &event.id).unwrap());
        assert!(get_event(&conn, &event.id).unwrap().is_none());
    }

    #[test]
    fn test_find_event_by_source_uid() {
        let conn = open_test_db();
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

        create_event(&conn, &data).unwrap();

        let found = find_event_by_source_uid(&conn, "unique-uid-123@example.com").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Imported");

        let missing = find_event_by_source_uid(&conn, "nonexistent@example.com").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_calendar_settings() {
        let conn = open_test_db();

        let settings = get_calendar_settings(&conn).unwrap();
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

        let updated = update_calendar_settings(&conn, &update).unwrap();
        assert_eq!(updated.default_view, "week");
        assert_eq!(updated.week_starts_on, 1);
        assert_eq!(updated.time_format, "24h");
    }

    #[test]
    fn test_meeting_templates() {
        let conn = open_test_db();

        let template =
            create_meeting_template(&conn, "Zoom", "https://zoom.us/j/{id}", "video").unwrap();
        assert_eq!(template.name, "Zoom");

        let all = list_meeting_templates(&conn).unwrap();
        assert_eq!(all.len(), 1);

        assert!(delete_meeting_template(&conn, template.id).unwrap());
        assert!(!delete_meeting_template(&conn, template.id).unwrap());

        let all = list_meeting_templates(&conn).unwrap();
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
