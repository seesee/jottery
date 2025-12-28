-- Add user email to sync metadata for multi-user support
-- Required for new server authentication system (Phase 2)

ALTER TABLE sync_metadata ADD COLUMN user_email TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (6, datetime('now'));
