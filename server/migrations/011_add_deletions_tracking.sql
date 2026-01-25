-- Track permanently deleted notes for sync
-- When a client empties their trash (hard deletes), we need to tell other clients
-- to remove those notes too

CREATE TABLE note_deletions (
    id TEXT NOT NULL,                       -- Note UUID that was deleted
    user_id TEXT NOT NULL,                  -- Owner user
    deleted_at TEXT NOT NULL,               -- ISO 8601 - when the deletion occurred
    synced_from_client_id TEXT,             -- Which client initiated the deletion
    expires_at TEXT NOT NULL,               -- ISO 8601 - when to purge this record
    PRIMARY KEY (id, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_deletions_user ON note_deletions(user_id);
CREATE INDEX idx_deletions_deleted_at ON note_deletions(deleted_at);
CREATE INDEX idx_deletions_expires ON note_deletions(expires_at);
