-- Add per-user maximum upload size setting
-- Default: 5MB (matches current hardcoded limit)

ALTER TABLE users ADD COLUMN max_upload_size_mb INTEGER DEFAULT 5 NOT NULL;
