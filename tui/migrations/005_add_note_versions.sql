-- Add note_versions table for version history
-- Versions are captured during sync events to maintain historical snapshots

CREATE TABLE IF NOT EXISTS note_versions (
    version_key TEXT PRIMARY KEY NOT NULL,
    note_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    synced_at TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT NOT NULL,
    attachments TEXT NOT NULL,
    syntax_language TEXT,
    word_wrap INTEGER,
    reason TEXT NOT NULL,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

-- Create indexes for efficient querying
CREATE INDEX idx_note_versions_note_id ON note_versions(note_id);
CREATE INDEX idx_note_versions_created_at ON note_versions(created_at DESC);
CREATE UNIQUE INDEX idx_note_versions_composite ON note_versions(note_id, version);

-- Record migration
INSERT INTO schema_version (version, applied_at) VALUES (5, datetime('now'));
