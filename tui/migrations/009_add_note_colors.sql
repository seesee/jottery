-- Add color field to notes table
-- This field stores the semantic color name for a note (e.g., 'red', 'blue')
ALTER TABLE notes ADD COLUMN color TEXT;

-- Add color_palette to settings table (stored as JSON)
-- Contains user-customized color palette with light/dark hex values
ALTER TABLE settings ADD COLUMN color_palette TEXT;

-- Add tag_colors to settings table (stored as JSON)
-- Maps tag names to color names globally
ALTER TABLE settings ADD COLUMN tag_colors TEXT;

-- Update schema version
INSERT INTO schema_version (version, applied_at) VALUES (9, datetime('now'));
