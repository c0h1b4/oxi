-- Add a uniform tenant column for the shared repository API. SQLite files remain per user.
ALTER TABLE user_meta ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_user_meta_tenant_key ON user_meta(user_hash, key);
ALTER TABLE folders ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_folders_tenant_key ON folders(user_hash, name);
ALTER TABLE messages ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_messages_tenant_key ON messages(user_hash, uid, folder);
ALTER TABLE drafts ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_drafts_tenant_key ON drafts(user_hash, id);
ALTER TABLE draft_attachments ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_draft_attachments_tenant_key ON draft_attachments(user_hash, id);
ALTER TABLE contacts ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_contacts_tenant_key ON contacts(user_hash, id);
ALTER TABLE identities ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_identities_tenant_key ON identities(user_hash, id);
ALTER TABLE notification_preferences ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_notification_preferences_tenant_key ON notification_preferences(user_hash, id);
ALTER TABLE contact_groups ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_contact_groups_tenant_key ON contact_groups(user_hash, id);
ALTER TABLE contact_group_members ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_contact_group_members_tenant_key ON contact_group_members(user_hash, group_id, contact_id);
ALTER TABLE tags ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_tags_tenant_key ON tags(user_hash, id);
ALTER TABLE message_tags ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_message_tags_tenant_key ON message_tags(user_hash, tag_id, message_uid, message_folder);
ALTER TABLE display_preferences ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_display_preferences_tenant_key ON display_preferences(user_hash, id);
ALTER TABLE calendar_events ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_calendar_events_tenant_key ON calendar_events(user_hash, id);
ALTER TABLE calendar_settings ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_calendar_settings_tenant_key ON calendar_settings(user_hash, id);
ALTER TABLE meeting_templates ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_meeting_templates_tenant_key ON meeting_templates(user_hash, id);
ALTER TABLE known_addresses ADD COLUMN user_hash TEXT NOT NULL DEFAULT '';
CREATE UNIQUE INDEX idx_known_addresses_tenant_key ON known_addresses(user_hash, email);
CREATE UNIQUE INDEX idx_contacts_tenant_email ON contacts(user_hash, email);
CREATE UNIQUE INDEX idx_tags_tenant_name ON tags(user_hash, name);
CREATE UNIQUE INDEX idx_contact_groups_tenant_name ON contact_groups(user_hash, name);
CREATE UNIQUE INDEX idx_notification_preferences_tenant_singleton ON notification_preferences(user_hash);
CREATE UNIQUE INDEX idx_display_preferences_tenant_singleton ON display_preferences(user_hash);
CREATE UNIQUE INDEX idx_calendar_settings_tenant_singleton ON calendar_settings(user_hash);

-- Store the full unsigned IMAP MODSEQ without SQLite floating-point coercion.
ALTER TABLE folders ADD COLUMN modseq_text TEXT NOT NULL DEFAULT '0';
-- The old client stored u64 values with `as i64`; recover their unsigned bits.
UPDATE folders SET modseq_text = CASE WHEN highest_modseq < 0
    THEN printf('%u', highest_modseq) ELSE CAST(highest_modseq AS TEXT) END;
ALTER TABLE folders DROP COLUMN highest_modseq;
ALTER TABLE folders RENAME COLUMN modseq_text TO highest_modseq;
