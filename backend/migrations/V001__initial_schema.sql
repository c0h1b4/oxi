CREATE TABLE IF NOT EXISTS user_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO user_meta (key, value) VALUES ('schema_version', '1');
