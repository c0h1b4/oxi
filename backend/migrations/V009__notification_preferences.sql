CREATE TABLE IF NOT EXISTS notification_preferences (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 1,
    sound INTEGER NOT NULL DEFAULT 0,
    folders TEXT NOT NULL DEFAULT '["INBOX"]',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO notification_preferences (id) VALUES (1);
