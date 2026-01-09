-- Add conflict_data column for storing server version during conflicts
ALTER TABLE note_sync_metadata ADD COLUMN conflict_data TEXT;
