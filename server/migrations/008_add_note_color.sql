-- Add color field to notes and note_versions tables
-- This field stores the semantic color name for a note (e.g., 'red', 'blue')
-- Color names are user-configurable category labels

-- Add to notes table
ALTER TABLE notes ADD COLUMN color TEXT DEFAULT NULL;

-- Add to note_versions table
ALTER TABLE note_versions ADD COLUMN color TEXT DEFAULT NULL;
