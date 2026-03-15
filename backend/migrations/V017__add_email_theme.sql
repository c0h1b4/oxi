-- Add email_theme column to messages table
-- NULL = not yet detected (only relevant when body_html exists)
-- 0 = Light, 1 = Dark, 2 = Transparent
ALTER TABLE messages ADD COLUMN email_theme INTEGER;
