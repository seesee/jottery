-- The language setting never did anything, so en-GB here doesn't mean anyone chose it.
-- Now that it works, these keep following the system, as they always have.
UPDATE settings SET language = 'auto' WHERE language = 'en-GB';

INSERT INTO schema_version (version, applied_at) VALUES (21, datetime('now'));
