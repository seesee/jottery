-- Add remember_password and stored_password to settings table
ALTER TABLE settings ADD COLUMN remember_password INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN stored_password TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (3, datetime('now'));
