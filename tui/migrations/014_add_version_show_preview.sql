-- Add show_preview field to note_versions table
-- This field indicates whether to show preview in version history
ALTER TABLE note_versions ADD COLUMN show_preview INTEGER;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (14, datetime('now'));
