-- Add show_preview field to notes and note_versions tables
-- This field tracks whether markdown/html preview is shown for a note

-- Add to notes table
ALTER TABLE notes ADD COLUMN show_preview INTEGER DEFAULT 0;

-- Add to note_versions table
ALTER TABLE note_versions ADD COLUMN show_preview INTEGER DEFAULT 0;
