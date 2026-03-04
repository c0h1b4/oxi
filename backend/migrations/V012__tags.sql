CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#6b7280',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS message_tags (
    tag_id TEXT NOT NULL,
    message_uid INTEGER NOT NULL,
    message_folder TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (tag_id, message_uid, message_folder),
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE,
    FOREIGN KEY (message_uid, message_folder) REFERENCES messages(uid, folder) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_message_tags_message ON message_tags(message_uid, message_folder);
CREATE INDEX IF NOT EXISTS idx_message_tags_tag ON message_tags(tag_id);
