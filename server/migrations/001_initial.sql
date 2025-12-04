-- Server database schema for jottery sync
-- Initial migration: Create all tables and indexes

-- API clients (devices)
CREATE TABLE clients (
    id TEXT PRIMARY KEY,                    -- UUID v4
    api_key TEXT NOT NULL UNIQUE,           -- SHA-256 hashed API key
    device_name TEXT NOT NULL,
    device_type TEXT NOT NULL,              -- 'web', 'cli'
    created_at TEXT NOT NULL,               -- ISO 8601
    last_seen_at TEXT NOT NULL,             -- ISO 8601
    is_active INTEGER NOT NULL DEFAULT 1    -- 0 or 1
);

CREATE INDEX idx_clients_api_key ON clients(api_key);
CREATE INDEX idx_clients_active ON clients(is_active);

-- Notes (encrypted payloads)
CREATE TABLE notes (
    id TEXT PRIMARY KEY,                    -- Note UUID
    client_id TEXT NOT NULL,                -- Owner client
    created_at TEXT NOT NULL,               -- ISO 8601 (from client)
    modified_at TEXT NOT NULL,              -- ISO 8601 (from client)
    server_modified_at TEXT NOT NULL,       -- ISO 8601 (server timestamp)
    content TEXT NOT NULL,                  -- Encrypted JSON string
    tags TEXT NOT NULL,                     -- Encrypted JSON array (as string)
    pinned INTEGER NOT NULL DEFAULT 0,      -- 0 or 1
    deleted INTEGER NOT NULL DEFAULT 0,     -- 0 or 1 (soft delete)
    deleted_at TEXT,                        -- ISO 8601
    version INTEGER NOT NULL DEFAULT 1,     -- Client version number
    server_version INTEGER NOT NULL DEFAULT 1, -- Server version (increment)
    word_wrap INTEGER DEFAULT 1,            -- 0 or 1
    syntax_language TEXT DEFAULT 'plain',   -- Language identifier
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_notes_client ON notes(client_id);
CREATE INDEX idx_notes_modified ON notes(modified_at);
CREATE INDEX idx_notes_server_modified ON notes(server_modified_at);
CREATE INDEX idx_notes_deleted ON notes(deleted);
CREATE INDEX idx_notes_client_modified ON notes(client_id, modified_at);

-- Attachments metadata
CREATE TABLE attachments_meta (
    id TEXT PRIMARY KEY,                    -- Attachment UUID
    note_id TEXT NOT NULL,                  -- Parent note
    filename TEXT NOT NULL,                 -- Encrypted filename
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,                  -- Bytes
    created_at TEXT NOT NULL,               -- ISO 8601
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

CREATE INDEX idx_attachments_note ON attachments_meta(note_id);

-- Attachment blobs (encrypted data)
CREATE TABLE attachments_data (
    id TEXT PRIMARY KEY,                    -- Same as attachments_meta.id
    data BLOB NOT NULL,                     -- Base64-encoded encrypted blob
    created_at TEXT NOT NULL,               -- ISO 8601
    FOREIGN KEY (id) REFERENCES attachments_meta(id) ON DELETE CASCADE
);

-- Sync operations audit trail (optional, for debugging)
CREATE TABLE sync_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,           -- 'push', 'pull'
    note_count INTEGER NOT NULL DEFAULT 0,
    attachment_count INTEGER NOT NULL DEFAULT 0,
    timestamp TEXT NOT NULL,                -- ISO 8601
    success INTEGER NOT NULL DEFAULT 1,     -- 0 or 1
    error_message TEXT,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

CREATE INDEX idx_sync_ops_client ON sync_operations(client_id);
CREATE INDEX idx_sync_ops_timestamp ON sync_operations(timestamp);
