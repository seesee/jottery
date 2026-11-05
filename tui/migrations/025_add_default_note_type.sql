-- What a new note starts as, on this device.
ALTER TABLE settings ADD COLUMN default_note_type TEXT NOT NULL DEFAULT 'markdown';

INSERT INTO schema_version (version, applied_at) VALUES (25, datetime('now'));
