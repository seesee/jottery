-- Update syntax_language default to markdown
-- This matches the Note::new() default in code

-- Update all existing 'plain' notes to 'markdown'
-- This ensures existing notes benefit from markdown rendering
UPDATE notes
SET syntax_language = 'markdown'
WHERE syntax_language = 'plain';

-- Note: SQLite doesn't support ALTER COLUMN to change DEFAULT
-- The updated default will be enforced in future inserts via application code
-- New notes use Note::new() which defaults to Markdown

-- Record migration
INSERT INTO schema_version (version, applied_at) VALUES (4, datetime('now'));
