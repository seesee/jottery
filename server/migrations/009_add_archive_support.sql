-- Migration 009: Add archive support to notes table
-- Allows notes to be archived without deletion

-- Add archived and archived_at columns to notes table
ALTER TABLE notes ADD COLUMN archived BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN archived_at TEXT;

-- Create index for efficient archived queries
CREATE INDEX IF NOT EXISTS idx_notes_archived ON notes(user_id, archived);
CREATE INDEX IF NOT EXISTS idx_notes_archived_modified ON notes(user_id, archived, modified_at);
