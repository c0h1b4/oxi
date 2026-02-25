-- Folder cache: stores the IMAP folder hierarchy
CREATE TABLE IF NOT EXISTS folders (
    name          TEXT PRIMARY KEY,
    delimiter     TEXT,
    parent        TEXT,
    flags         TEXT NOT NULL DEFAULT '',
    is_subscribed INTEGER NOT NULL DEFAULT 1,
    total_count   INTEGER NOT NULL DEFAULT 0,
    unread_count  INTEGER NOT NULL DEFAULT 0,
    uid_validity  INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER NOT NULL DEFAULT 0,
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Message header cache: stores synced email headers per folder
CREATE TABLE IF NOT EXISTS messages (
    uid           INTEGER NOT NULL,
    folder        TEXT NOT NULL,
    message_id    TEXT,
    in_reply_to   TEXT,
    subject       TEXT NOT NULL DEFAULT '',
    from_address  TEXT NOT NULL DEFAULT '',
    from_name     TEXT NOT NULL DEFAULT '',
    to_addresses  TEXT NOT NULL DEFAULT '',
    cc_addresses  TEXT NOT NULL DEFAULT '',
    date          TEXT NOT NULL,
    flags         TEXT NOT NULL DEFAULT '',
    size          INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    snippet       TEXT NOT NULL DEFAULT '',
    body_cached   INTEGER NOT NULL DEFAULT 0,
    body_html     TEXT,
    body_text     TEXT,
    references_header TEXT,
    PRIMARY KEY (uid, folder),
    FOREIGN KEY (folder) REFERENCES folders(name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_messages_folder_date ON messages(folder, date DESC);
CREATE INDEX IF NOT EXISTS idx_messages_message_id ON messages(message_id);
CREATE INDEX IF NOT EXISTS idx_messages_in_reply_to ON messages(in_reply_to);
