-- Add pending registration email for tracking registrations awaiting approval
ALTER TABLE sync_metadata ADD COLUMN pending_registration_email TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (8, datetime('now'));
