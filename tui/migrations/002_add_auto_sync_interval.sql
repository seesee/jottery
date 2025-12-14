-- Add auto_sync_interval_minutes to settings table
ALTER TABLE settings ADD COLUMN auto_sync_interval_minutes INTEGER NOT NULL DEFAULT 5;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (2, datetime('now'));
