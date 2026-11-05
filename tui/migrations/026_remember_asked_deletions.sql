-- A batch of deletions the user has been asked about stays held until they answer. Without
-- this, anything that took some of it away would drop the rest under the limit, and it
-- would go ahead with the question never answered.
ALTER TABLE held_deletions ADD COLUMN asked INTEGER NOT NULL DEFAULT 0;

INSERT INTO schema_version (version, applied_at) VALUES (26, datetime('now'));
