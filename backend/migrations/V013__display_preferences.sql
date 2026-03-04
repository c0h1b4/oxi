CREATE TABLE IF NOT EXISTS display_preferences (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    density TEXT NOT NULL DEFAULT 'comfortable',
    theme TEXT NOT NULL DEFAULT 'system',
    language TEXT NOT NULL DEFAULT 'en',
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
