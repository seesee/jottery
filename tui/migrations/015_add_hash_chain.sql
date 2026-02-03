-- Migration 015: Add hash chain fields for git-like conflict detection
-- These fields enable detecting conflicts by hash divergence rather than timestamps

-- Add hash chain columns to notes table
ALTER TABLE notes ADD COLUMN content_hash TEXT;
ALTER TABLE notes ADD COLUMN parent_hash TEXT;
ALTER TABLE notes ADD COLUMN hash_chain TEXT;  -- JSON array of ancestor hashes

-- Add hash chain columns to note_versions table
ALTER TABLE note_versions ADD COLUMN content_hash TEXT;
ALTER TABLE note_versions ADD COLUMN parent_hash TEXT;

-- Create index for content_hash lookups
CREATE INDEX IF NOT EXISTS idx_notes_content_hash ON notes(content_hash) WHERE content_hash IS NOT NULL;

-- Record migration version
INSERT INTO schema_version (version, applied_at) VALUES (15, datetime('now'));
