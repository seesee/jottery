-- Searches saved on any device. name and query hold ciphertext, in the form every
-- client sends. A deleted row stays, so an older copy from the server can't bring it back.
CREATE TABLE IF NOT EXISTS saved_searches (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    query TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    synced_at TEXT,
    deleted INTEGER NOT NULL DEFAULT 0,
    deleted_at TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    needs_sync INTEGER NOT NULL DEFAULT 0
);

INSERT INTO schema_version (version, applied_at) VALUES (22, datetime('now'));
