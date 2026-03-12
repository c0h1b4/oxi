CREATE TABLE calendar_events (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    all_day INTEGER NOT NULL DEFAULT 0,
    recurrence_rule TEXT,
    attendees TEXT NOT NULL DEFAULT '[]',
    organizer TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'confirmed',
    source TEXT NOT NULL DEFAULT 'manual',
    source_uid TEXT,
    source_message_uid INTEGER,
    source_message_folder TEXT,
    meeting_url TEXT,
    color TEXT,
    reminder_minutes INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_calendar_events_time ON calendar_events(start_time, end_time);
CREATE INDEX idx_calendar_events_source_uid ON calendar_events(source_uid);

CREATE TABLE calendar_settings (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    default_view TEXT NOT NULL DEFAULT 'month',
    week_starts_on INTEGER NOT NULL DEFAULT 0,
    working_hours_start TEXT NOT NULL DEFAULT '09:00',
    working_hours_end TEXT NOT NULL DEFAULT '17:00',
    time_format TEXT NOT NULL DEFAULT '12h',
    zoom_link TEXT NOT NULL DEFAULT '',
    teams_link TEXT NOT NULL DEFAULT '',
    meet_link TEXT NOT NULL DEFAULT '',
    caldav_url TEXT NOT NULL DEFAULT '',
    caldav_username TEXT NOT NULL DEFAULT '',
    caldav_password TEXT NOT NULL DEFAULT '',
    caldav_enabled INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE meeting_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    url_template TEXT NOT NULL,
    icon TEXT NOT NULL DEFAULT '',
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
