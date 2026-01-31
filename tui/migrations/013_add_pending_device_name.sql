-- Add pending device name for clone-device call during credential import
ALTER TABLE sync_metadata ADD COLUMN pending_device_name TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (13, datetime('now'));
