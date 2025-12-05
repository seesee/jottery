-- Initial schema for Jottery TUI
-- Matches web app IndexedDB structure

-- Notes table
-- Stores encrypted note content and metadata
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    synced_at TEXT,
    content TEXT NOT NULL,        -- Encrypted JSON string
    tags TEXT NOT NULL,            -- Encrypted JSON array
    attachments TEXT NOT NULL,     -- JSON array of attachment references
    pinned INTEGER NOT NULL DEFAULT 0,
    deleted INTEGER NOT NULL DEFAULT 0,
    deleted_at TEXT,
    sync_hash TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    word_wrap INTEGER NOT NULL DEFAULT 1,
    syntax_language TEXT NOT NULL DEFAULT 'plain'
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_notes_modified_at ON notes(modified_at DESC);
CREATE INDEX IF NOT EXISTS idx_notes_created_at ON notes(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_notes_deleted ON notes(deleted);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(pinned);
CREATE INDEX IF NOT EXISTS idx_notes_deleted_modified ON notes(deleted, modified_at DESC);

-- Attachments table
-- Stores encrypted attachment data
CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY NOT NULL,
    filename TEXT NOT NULL,        -- Encrypted filename
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    data BLOB NOT NULL,            -- Encrypted binary data
    thumbnail_data BLOB            -- Optional encrypted thumbnail
);

-- Settings table (single row)
-- Stores user application settings
CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    language TEXT NOT NULL DEFAULT 'en-GB',
    theme TEXT NOT NULL DEFAULT 'auto',
    sort_order TEXT NOT NULL DEFAULT 'recent',
    auto_lock_timeout INTEGER NOT NULL DEFAULT 15,
    sync_enabled INTEGER NOT NULL DEFAULT 0,
    sync_endpoint TEXT
);

-- Insert default settings
INSERT OR IGNORE INTO settings (id, language, theme, sort_order, auto_lock_timeout, sync_enabled)
VALUES (1, 'en-GB', 'auto', 'recent', 15, 0);

-- Encryption metadata table (single row)
-- Stores encryption setup information
CREATE TABLE IF NOT EXISTS encryption_metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    salt TEXT NOT NULL,
    iterations INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    algorithm TEXT NOT NULL DEFAULT 'AES-256-GCM'
);

-- Sync metadata table (single row)
-- Stores global sync configuration
CREATE TABLE IF NOT EXISTS sync_metadata (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_sync_at TEXT,
    last_push_at TEXT,
    last_pull_at TEXT,
    api_key TEXT,                  -- Encrypted API key (JSON)
    client_id TEXT,
    sync_enabled INTEGER NOT NULL DEFAULT 0,
    sync_endpoint TEXT,
    auto_sync_interval INTEGER DEFAULT 5
);

-- Note sync metadata table
-- Stores per-note sync tracking
CREATE TABLE IF NOT EXISTS note_sync_metadata (
    note_id TEXT PRIMARY KEY NOT NULL,
    synced_at TEXT NOT NULL,
    sync_hash TEXT NOT NULL,
    server_version INTEGER NOT NULL,
    last_sync_status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

-- Version tracking for migrations
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

INSERT INTO schema_version (version, applied_at) VALUES (1, datetime('now'));
