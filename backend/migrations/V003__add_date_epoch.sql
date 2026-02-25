-- Add a sortable integer timestamp column for proper date ordering.
ALTER TABLE messages ADD COLUMN date_epoch INTEGER NOT NULL DEFAULT 0;

-- Rebuild index to use the new column for sorting.
DROP INDEX IF EXISTS idx_messages_folder_date;
CREATE INDEX idx_messages_folder_date ON messages(folder, date_epoch DESC);
