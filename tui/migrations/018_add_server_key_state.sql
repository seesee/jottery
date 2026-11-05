-- What this device knows about the server's wrapped copy of the master key.
--
-- server_key_seen is a hash of the blob this device last opened or uploaded. Being
-- set proves the blob isn't in the old account-password format, so if it later
-- stops opening, the password was changed on another device.
--
-- server_key_stale means this device changed the notes password and still owes the
-- server the re-wrapped key.
ALTER TABLE sync_metadata ADD COLUMN server_key_seen TEXT;
ALTER TABLE sync_metadata ADD COLUMN server_key_stale INTEGER NOT NULL DEFAULT 0;

INSERT INTO schema_version (version, applied_at) VALUES (18, datetime('now'));
