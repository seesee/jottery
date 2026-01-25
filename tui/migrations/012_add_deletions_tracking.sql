-- Track permanently deleted notes for sync
-- When emptying trash (hard delete), record the deletion so it syncs to server/other devices

CREATE TABLE note_deletions (
    id TEXT PRIMARY KEY NOT NULL,           -- Note UUID that was deleted
    deleted_at TEXT NOT NULL,               -- ISO 8601 - when the deletion occurred
    synced INTEGER NOT NULL DEFAULT 0       -- 0 = pending, 1 = synced to server
);

CREATE INDEX idx_local_deletions_synced ON note_deletions(synced);

-- Record schema version
INSERT INTO schema_version (version, applied_at) VALUES (12, datetime('now'));
