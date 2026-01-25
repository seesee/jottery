-- Add lock support for notes
-- Locked notes cannot be edited until unlocked

ALTER TABLE notes ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN locked_at TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (11, datetime('now'));
