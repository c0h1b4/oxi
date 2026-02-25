-- Separate timestamp for when a folder's messages were last synced from IMAP,
-- distinct from `updated_at` which tracks when the folder list was last synced.
ALTER TABLE folders ADD COLUMN messages_updated_at TEXT;
