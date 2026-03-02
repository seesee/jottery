-- Add user_id to sync metadata for envelope encryption
-- The user_id (UUID) is used as the wrapping key salt for envelope encryption

ALTER TABLE sync_metadata ADD COLUMN user_id TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (17, datetime('now'));
