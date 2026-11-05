-- Rebuild the sync tables. Fixes three problems at once so each table is only
-- rebuilt one time:
--
-- 1. Device deletion destroyed notes. notes.client_id and note_versions.client_id
--    were NOT NULL with ON DELETE CASCADE, and client_id records whichever device
--    first pushed the note. They become nullable with ON DELETE SET NULL.
-- 2. Ids were global, so one user could overwrite another's rows. Saved searches,
--    versions and attachment metadata are now keyed per user.
-- 3. An attachment was bound to one note. The note_attachments link table lets a
--    note reference any of its owner's attachments, and blobs outlive any one note.
--
-- It also adds what the pull cursor needs: a change_seq on every synced table,
-- taken from a per-user counter in sync_state, and a server-set time for
-- tombstones and saved searches. The counter is per user so that a cursor reveals
-- nothing about anyone else's activity.
--
-- THIS MUST RUN WITH FOREIGN KEYS OFF. DROP TABLE performs an implicit DELETE
-- that fires ON DELETE CASCADE, and sqlx turns foreign keys on. The usual
-- "-- no-transaction" marker doesn't help: the SQLite driver in sqlx 0.8.6
-- ignores it, always wraps a migration in a transaction, and PRAGMA foreign_keys
-- is a no-op inside one. db::run_migrations sets the pragma on the connection
-- before the migrator starts.
--
-- If this ever runs with foreign keys on against a database that holds data, the
-- row counts checked at the end won't match and the whole migration rolls back.
--
-- attachments_data, the only large table, isn't touched: attachments_meta keeps
-- its primary key, which now means "key of the blob", and the client's attachment
-- id moves to its own column.

CREATE TEMP TABLE migration_row_counts AS
SELECT
    (SELECT COUNT(*) FROM notes) AS notes,
    (SELECT COUNT(*) FROM note_versions) AS note_versions,
    (SELECT COUNT(*) FROM attachments_meta) AS attachments_meta,
    (SELECT COUNT(*) FROM attachments_data) AS attachments_data,
    (SELECT COUNT(*) FROM saved_searches) AS saved_searches,
    (SELECT COUNT(*) FROM note_deletions) AS note_deletions;

-- 1. One sequence across each user's synced rows, in server-time order, so that a
--    client moving from lastSyncAt to a cursor only has recent rows re-sent.
CREATE TEMP TABLE sequence_backfill AS
SELECT kind, owner, item,
       ROW_NUMBER() OVER (PARTITION BY owner ORDER BY changed_at, kind, item) AS seq
FROM (
    SELECT 'note' AS kind, user_id AS owner, id AS item, server_modified_at AS changed_at FROM notes
    UNION ALL
    SELECT 'version', user_id, version_key, server_synced_at FROM note_versions
    UNION ALL
    SELECT 'deletion', user_id, id, deleted_at FROM note_deletions
    UNION ALL
    SELECT 'search', user_id, id, COALESCE(synced_at, modified_at) FROM saved_searches
);
CREATE UNIQUE INDEX temp.idx_sequence_backfill ON sequence_backfill(kind, owner, item);

CREATE TABLE sync_state (
    user_id TEXT PRIMARY KEY NOT NULL,
    last_seq INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
INSERT INTO sync_state (user_id, last_seq)
SELECT owner, MAX(seq) FROM sequence_backfill GROUP BY owner;

-- 2. note_versions: per-user key, device reference survives device deletion
CREATE TABLE note_versions_new (
    version_key TEXT NOT NULL,                  -- "note_id:version"
    note_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    client_id TEXT,                             -- NULL once the device is deleted
    version INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    synced_at TEXT NOT NULL,                    -- ISO 8601 (when synced to server)
    server_synced_at TEXT NOT NULL,             -- Server timestamp for sync tracking
    content TEXT NOT NULL,
    tags TEXT NOT NULL,
    attachments TEXT NOT NULL,
    syntax_language TEXT,
    word_wrap INTEGER,
    reason TEXT NOT NULL,                       -- 'sync' or 'manual-sync'
    show_preview INTEGER DEFAULT 0,
    color TEXT DEFAULT NULL,
    content_hash TEXT,
    parent_hash TEXT,
    change_seq INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, version_key),
    FOREIGN KEY (note_id, user_id) REFERENCES notes(id, user_id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE SET NULL
);

INSERT INTO note_versions_new (
    version_key, note_id, user_id, client_id, version, created_at, synced_at,
    server_synced_at, content, tags, attachments, syntax_language, word_wrap, reason,
    show_preview, color, content_hash, parent_hash, change_seq
)
SELECT
    v.version_key, v.note_id, v.user_id, v.client_id, v.version, v.created_at, v.synced_at,
    v.server_synced_at, v.content, v.tags, v.attachments, v.syntax_language, v.word_wrap, v.reason,
    v.show_preview, v.color, v.content_hash, v.parent_hash, s.seq
FROM note_versions v
JOIN sequence_backfill s ON s.kind = 'version' AND s.owner = v.user_id AND s.item = v.version_key;

DROP TABLE note_versions;
ALTER TABLE note_versions_new RENAME TO note_versions;

CREATE INDEX idx_note_versions_note_id ON note_versions(note_id, user_id);
CREATE INDEX idx_note_versions_user_id ON note_versions(user_id);
CREATE INDEX idx_note_versions_created_at ON note_versions(created_at);
CREATE INDEX idx_note_versions_client_id ON note_versions(client_id);
CREATE INDEX idx_note_versions_user_seq ON note_versions(user_id, change_seq);
CREATE INDEX idx_note_versions_user_synced ON note_versions(user_id, server_synced_at);

-- 3. attachments_meta: id stays the key of the blob in attachments_data, so that
--    table needs no change. New rows get a server-generated id. The client's id
--    is attachment_id, unique per user. Copied in rowid order because pull
--    returns attachments in that order.
CREATE TABLE attachments_meta_new (
    id TEXT PRIMARY KEY NOT NULL,               -- Key of the blob in attachments_data
    attachment_id TEXT NOT NULL,                -- Attachment UUID from the client
    user_id TEXT NOT NULL,
    filename TEXT NOT NULL,                     -- Encrypted filename
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,                      -- Bytes
    created_at TEXT NOT NULL,                   -- ISO 8601
    unreferenced_at TEXT,                       -- When no note linked it any more
    UNIQUE (user_id, attachment_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO attachments_meta_new (id, attachment_id, user_id, filename, mime_type, size, created_at)
SELECT id, id, note_user_id, filename, mime_type, size, created_at
FROM attachments_meta
ORDER BY rowid;

CREATE TEMP TABLE attachment_links AS
SELECT m.note_user_id AS user_id, m.note_id, m.id AS attachment_id
FROM attachments_meta m
JOIN notes n ON n.id = m.note_id AND n.user_id = m.note_user_id;

DROP TABLE attachments_meta;
ALTER TABLE attachments_meta_new RENAME TO attachments_meta;

CREATE INDEX idx_attachments_unreferenced ON attachments_meta(unreferenced_at)
    WHERE unreferenced_at IS NOT NULL;

-- 4. note_attachments: which notes reference which attachments
CREATE TABLE note_attachments (
    user_id TEXT NOT NULL,
    note_id TEXT NOT NULL,
    attachment_id TEXT NOT NULL,
    PRIMARY KEY (user_id, note_id, attachment_id),
    FOREIGN KEY (note_id, user_id) REFERENCES notes(id, user_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id, attachment_id) REFERENCES attachments_meta(user_id, attachment_id) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX idx_note_attachments_attachment ON note_attachments(user_id, attachment_id);

INSERT INTO note_attachments (user_id, note_id, attachment_id)
SELECT user_id, note_id, attachment_id FROM attachment_links;

-- 5. notes: device reference survives device deletion
CREATE TABLE notes_new (
    id TEXT NOT NULL,                       -- Note UUID (unique per user)
    user_id TEXT NOT NULL,                  -- User who owns this note
    client_id TEXT,                         -- First device to push; NULL once it's deleted
    created_at TEXT NOT NULL,               -- ISO 8601 (from client)
    modified_at TEXT NOT NULL,              -- ISO 8601 (from client)
    server_modified_at TEXT NOT NULL,       -- ISO 8601 (server timestamp)
    content TEXT NOT NULL,                  -- Encrypted JSON string
    tags TEXT NOT NULL,                     -- Encrypted JSON array (as string)
    pinned INTEGER NOT NULL DEFAULT 0,      -- 0 or 1
    deleted INTEGER NOT NULL DEFAULT 0,     -- 0 or 1 (soft delete)
    deleted_at TEXT,                        -- ISO 8601
    version INTEGER NOT NULL DEFAULT 1,     -- Client version number
    server_version INTEGER NOT NULL DEFAULT 1, -- Server version (increment)
    word_wrap INTEGER DEFAULT 1,            -- 0 or 1
    syntax_language TEXT DEFAULT 'plain',   -- Language identifier
    show_preview INTEGER DEFAULT 0,
    color TEXT DEFAULT NULL,
    archived BOOLEAN NOT NULL DEFAULT 0,
    archived_at TEXT,
    locked INTEGER NOT NULL DEFAULT 0,
    locked_at TEXT,
    content_hash TEXT,
    parent_hash TEXT,
    hash_chain TEXT,
    change_seq INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (id, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE SET NULL
);

INSERT INTO notes_new (
    id, user_id, client_id, created_at, modified_at, server_modified_at, content, tags,
    pinned, deleted, deleted_at, version, server_version, word_wrap, syntax_language,
    show_preview, color, archived, archived_at, locked, locked_at,
    content_hash, parent_hash, hash_chain, change_seq
)
SELECT
    n.id, n.user_id, n.client_id, n.created_at, n.modified_at, n.server_modified_at, n.content, n.tags,
    n.pinned, n.deleted, n.deleted_at, n.version, n.server_version, n.word_wrap, n.syntax_language,
    n.show_preview, n.color, n.archived, n.archived_at, n.locked, n.locked_at,
    n.content_hash, n.parent_hash, n.hash_chain, s.seq
FROM notes n
JOIN sequence_backfill s ON s.kind = 'note' AND s.owner = n.user_id AND s.item = n.id;

DROP TABLE notes;
ALTER TABLE notes_new RENAME TO notes;

CREATE INDEX idx_notes_user_id ON notes(user_id);
CREATE INDEX idx_notes_client ON notes(client_id);
CREATE INDEX idx_notes_modified ON notes(modified_at);
CREATE INDEX idx_notes_server_modified ON notes(server_modified_at);
CREATE INDEX idx_notes_deleted ON notes(deleted);
CREATE INDEX idx_notes_user_modified ON notes(user_id, modified_at);
CREATE INDEX idx_notes_archived ON notes(user_id, archived);
CREATE INDEX idx_notes_archived_modified ON notes(user_id, archived, modified_at);
CREATE INDEX idx_notes_content_hash ON notes(content_hash) WHERE content_hash IS NOT NULL;
CREATE INDEX idx_notes_user_seq ON notes(user_id, change_seq);
CREATE INDEX idx_notes_user_server_modified ON notes(user_id, server_modified_at);

-- 6. saved_searches: per-user key. synced_at is the server's clock, so it becomes
--    the legacy pull filter. Rows that predate it fall back to the client's time.
CREATE TABLE saved_searches_new (
    id TEXT NOT NULL,                           -- UUID v4
    user_id TEXT NOT NULL,                      -- User ownership
    name TEXT NOT NULL,                         -- User-provided search name
    query TEXT NOT NULL,                        -- Search query string
    "order" INTEGER NOT NULL,                   -- Display order (quoted because ORDER is SQL keyword)
    created_at TEXT NOT NULL,                   -- ISO 8601
    modified_at TEXT NOT NULL,                  -- ISO 8601
    synced_at TEXT,                             -- ISO 8601, set by the server
    deleted INTEGER DEFAULT 0 NOT NULL,         -- Soft delete flag (0/1)
    deleted_at TEXT,                            -- Deletion timestamp (ISO 8601)
    version INTEGER DEFAULT 1 NOT NULL,         -- Optimistic locking
    change_seq INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (id, user_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO saved_searches_new (
    id, user_id, name, query, "order", created_at, modified_at, synced_at,
    deleted, deleted_at, version, change_seq
)
SELECT
    q.id, q.user_id, q.name, q.query, q."order", q.created_at, q.modified_at,
    COALESCE(q.synced_at, q.modified_at), q.deleted, q.deleted_at, q.version, s.seq
FROM saved_searches q
JOIN sequence_backfill s ON s.kind = 'search' AND s.owner = q.user_id AND s.item = q.id;

DROP TABLE saved_searches;
ALTER TABLE saved_searches_new RENAME TO saved_searches;

CREATE INDEX idx_saved_searches_user_id ON saved_searches(user_id);
CREATE INDEX idx_saved_searches_deleted ON saved_searches(deleted);
CREATE INDEX idx_saved_searches_user_deleted ON saved_searches(user_id, deleted);
CREATE INDEX idx_saved_searches_modified ON saved_searches(modified_at);
CREATE INDEX idx_saved_searches_user_order ON saved_searches(user_id, "order");
CREATE INDEX idx_saved_searches_user_seq ON saved_searches(user_id, change_seq);
CREATE INDEX idx_saved_searches_user_synced ON saved_searches(user_id, synced_at);

-- 7. note_deletions: the server's receipt time, kept apart from the client's
--    deleted_at, which is only for comparing an edit against a deletion.
ALTER TABLE note_deletions ADD COLUMN received_at TEXT NOT NULL DEFAULT '';
ALTER TABLE note_deletions ADD COLUMN change_seq INTEGER NOT NULL DEFAULT 0;

UPDATE note_deletions
SET received_at = deleted_at,
    change_seq = (
        SELECT seq FROM sequence_backfill s
        WHERE s.kind = 'deletion' AND s.owner = note_deletions.user_id AND s.item = note_deletions.id
    );

CREATE INDEX idx_deletions_user_seq ON note_deletions(user_id, change_seq);
CREATE INDEX idx_deletions_user_received ON note_deletions(user_id, received_at);

-- 8. Every row must have survived. A failure here rolls the whole migration back.
CREATE TEMP TABLE migration_guard (
    rows_preserved INTEGER NOT NULL CHECK (rows_preserved = 1)
);

INSERT INTO migration_guard (rows_preserved)
SELECT
    c.notes = (SELECT COUNT(*) FROM notes)
    AND c.note_versions = (SELECT COUNT(*) FROM note_versions)
    AND c.attachments_meta = (SELECT COUNT(*) FROM attachments_meta)
    AND c.attachments_data = (SELECT COUNT(*) FROM attachments_data)
    AND c.saved_searches = (SELECT COUNT(*) FROM saved_searches)
    AND c.note_deletions = (SELECT COUNT(*) FROM note_deletions)
FROM migration_row_counts c;

DROP TABLE migration_guard;
DROP TABLE migration_row_counts;
DROP TABLE attachment_links;
DROP TABLE sequence_backfill;
