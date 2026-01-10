-- Add conflict_data column for storing server version during conflicts
ALTER TABLE note_sync_metadata ADD COLUMN conflict_data TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (7, datetime('now'));
