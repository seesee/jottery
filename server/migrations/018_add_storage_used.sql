-- What each user holds on the server, in bytes, so that a push can be checked
-- against the quota without adding up every note and version. It starts at zero
-- and housekeeping fills it in within moments of the server starting.

ALTER TABLE users ADD COLUMN storage_used_bytes INTEGER NOT NULL DEFAULT 0;
