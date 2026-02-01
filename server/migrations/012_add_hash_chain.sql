-- Add hash chain fields for git-like conflict detection
-- These fields enable detecting true conflicts vs fast-forwards

-- Add hash chain columns to notes table
ALTER TABLE notes ADD COLUMN content_hash TEXT;      -- SHA-256 of content + tags + attachments + change fields
ALTER TABLE notes ADD COLUMN parent_hash TEXT;       -- Hash of previous version (NULL for new notes)
ALTER TABLE notes ADD COLUMN hash_chain TEXT;        -- JSON array of ancestor hashes (max 50)

-- Add hash columns to note_versions table for historical lookup
ALTER TABLE note_versions ADD COLUMN content_hash TEXT;
ALTER TABLE note_versions ADD COLUMN parent_hash TEXT;

-- Index for looking up notes by content hash (useful for finding ancestors)
CREATE INDEX idx_notes_content_hash ON notes(content_hash) WHERE content_hash IS NOT NULL;
