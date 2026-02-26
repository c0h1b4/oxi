-- Cache attachment metadata and raw headers alongside the message body.
ALTER TABLE messages ADD COLUMN attachments_json TEXT;
ALTER TABLE messages ADD COLUMN raw_headers TEXT;
