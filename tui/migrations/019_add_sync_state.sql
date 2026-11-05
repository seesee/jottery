-- What the sync layer needs to know for each note, rather than work out from clocks.
--
-- needs_sync: this note has a local change the server hasn't accepted yet. Pushing
-- used to go by comparing modified_at with the last sync, which missed anything saved
-- with an older timestamp, and left no way to tell a pending edit from a synced note.
--
-- attachments_complete: this device can vouch for the note's attachment list. Older
-- builds could empty it by mistake, so existing notes start out unproven, and only a
-- list taken from the server or built here counts.
ALTER TABLE notes ADD COLUMN needs_sync INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN attachments_complete INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN show_preview INTEGER;

-- Versions and blobs the server has, so that a push only sends what's new
ALTER TABLE note_versions ADD COLUMN pushed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE attachments ADD COLUMN pushed INTEGER NOT NULL DEFAULT 0;

-- Opaque position in the server's change sequence
ALTER TABLE sync_metadata ADD COLUMN pull_cursor TEXT;

-- Whatever the old rule would have pushed next
UPDATE notes SET needs_sync = 1
WHERE modified_at > COALESCE((SELECT last_sync_at FROM sync_metadata WHERE id = 1), '');

CREATE INDEX idx_notes_needs_sync ON notes(needs_sync);

INSERT INTO schema_version (version, applied_at) VALUES (19, datetime('now'));
