-- Nothing proves a deletion from the server came from one of the user's devices, so it
-- doesn't get to destroy anything here straight away. A note deleted elsewhere goes to
-- the bin, with this device's own time in remote_deleted_at, and stays there for a while.
ALTER TABLE notes ADD COLUMN remote_deleted_at TEXT;

-- Deletions from the server that haven't been acted on. A pull that deletes a lot waits
-- here for the user to say whether it's expected.
CREATE TABLE held_deletions (
    id TEXT PRIMARY KEY NOT NULL
);

INSERT INTO schema_version (version, applied_at) VALUES (20, datetime('now'));
