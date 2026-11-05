-- Notes now leave the bin by themselves after 30 days. The date on a note that came from
-- a server is the server's word, and it could say anything, so what's in the bin and has
-- been synced starts its 30 days now, on this device's clock. Those rows then go without a
-- tombstone: the server keeps its own 30 days for a soft-deleted note.
UPDATE notes
SET remote_deleted_at = strftime('%Y-%m-%dT%H:%M:%S+00:00', 'now')
WHERE deleted = 1 AND remote_deleted_at IS NULL AND synced_at IS NOT NULL;

INSERT INTO schema_version (version, applied_at) VALUES (24, datetime('now'));
