-- Add note_versions table for version history syncing
-- Versions are synced across devices and stored on server

CREATE TABLE note_versions (
    version_key TEXT PRIMARY KEY NOT NULL,      -- `${note_id}:${version}`
    note_id TEXT NOT NULL,
    client_id TEXT NOT NULL,                    -- Client that created this version
    version INTEGER NOT NULL,
    created_at TEXT NOT NULL,                   -- ISO 8601 (when version captured)
    synced_at TEXT NOT NULL,                    -- ISO 8601 (when synced to server)
    server_synced_at TEXT NOT NULL,             -- ISO 8601 (server timestamp)
    content TEXT NOT NULL,                      -- Encrypted JSON string
    tags TEXT NOT NULL,                         -- Encrypted JSON array
    attachments TEXT NOT NULL,                  -- JSON array of attachment refs
    syntax_language TEXT,
    word_wrap INTEGER,
    reason TEXT NOT NULL,                       -- 'sync' or 'manual-sync'
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

-- Create indexes for efficient querying
CREATE INDEX idx_note_versions_note_id ON note_versions(note_id);
CREATE INDEX idx_note_versions_client_id ON note_versions(client_id);
CREATE INDEX idx_note_versions_created_at ON note_versions(created_at DESC);
CREATE INDEX idx_note_versions_server_synced_at ON note_versions(server_synced_at DESC);
CREATE UNIQUE INDEX idx_note_versions_composite ON note_versions(note_id, version);
