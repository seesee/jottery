-- Fix notes table PRIMARY KEY to support multiple users with same note UUIDs
-- Each user can have notes with the same UUID (generated client-side)
-- The combination of (id, user_id) must be unique

-- 1. Recreate notes table with composite primary key
CREATE TABLE notes_new (
    id TEXT NOT NULL,                       -- Note UUID (unique per user)
    user_id TEXT NOT NULL,                  -- User who owns this note
    client_id TEXT NOT NULL,                -- Device that created/modified
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
    PRIMARY KEY (id, user_id),              -- Composite key: note UUID + user
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

-- 2. Migrate existing notes
INSERT INTO notes_new
SELECT
    id,
    user_id,
    client_id,
    created_at,
    modified_at,
    server_modified_at,
    content,
    tags,
    pinned,
    deleted,
    deleted_at,
    version,
    server_version,
    word_wrap,
    syntax_language
FROM notes;

-- 3. Replace old table
DROP TABLE notes;
ALTER TABLE notes_new RENAME TO notes;

-- 4. Recreate indexes
CREATE INDEX idx_notes_user_id ON notes(user_id);
CREATE INDEX idx_notes_client ON notes(client_id);
CREATE INDEX idx_notes_modified ON notes(modified_at);
CREATE INDEX idx_notes_server_modified ON notes(server_modified_at);
CREATE INDEX idx_notes_deleted ON notes(deleted);
CREATE INDEX idx_notes_user_modified ON notes(user_id, modified_at);

-- 5. Recreate attachments_meta table with proper foreign key
CREATE TABLE attachments_meta_new (
    id TEXT PRIMARY KEY,                    -- Attachment UUID
    note_id TEXT NOT NULL,                  -- Parent note UUID
    note_user_id TEXT NOT NULL,             -- Parent note's user (for FK)
    filename TEXT NOT NULL,                 -- Encrypted filename
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,                  -- Bytes
    created_at TEXT NOT NULL,               -- ISO 8601
    FOREIGN KEY (note_id, note_user_id) REFERENCES notes(id, user_id) ON DELETE CASCADE
);

-- 6. Migrate existing attachments
INSERT INTO attachments_meta_new (id, note_id, note_user_id, filename, mime_type, size, created_at)
SELECT
    am.id,
    am.note_id,
    n.user_id as note_user_id,
    am.filename,
    am.mime_type,
    am.size,
    am.created_at
FROM attachments_meta am
INNER JOIN notes n ON am.note_id = n.id;

-- 7. Replace old attachments_meta table
DROP TABLE attachments_meta;
ALTER TABLE attachments_meta_new RENAME TO attachments_meta;

-- 8. Recreate attachments_meta index
CREATE INDEX idx_attachments_note ON attachments_meta(note_id, note_user_id);

-- 9. Recreate note_versions table with composite foreign key
CREATE TABLE note_versions_new (
    version_key TEXT PRIMARY KEY NOT NULL,      -- "note_id:version"
    note_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
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
    FOREIGN KEY (note_id, user_id) REFERENCES notes(id, user_id) ON DELETE CASCADE,
    FOREIGN KEY (client_id) REFERENCES clients(id) ON DELETE CASCADE
);

-- 10. Migrate existing versions
-- First, update any NULL user_ids by looking up from notes table
UPDATE note_versions
SET user_id = (
    SELECT n.user_id
    FROM notes n
    WHERE n.id = note_versions.note_id
)
WHERE user_id IS NULL;

INSERT INTO note_versions_new
SELECT
    version_key,
    note_id,
    user_id,
    client_id,
    version,
    created_at,
    synced_at,
    server_synced_at,
    content,
    tags,
    attachments,
    syntax_language,
    word_wrap,
    reason
FROM note_versions;

-- 11. Replace old note_versions table
DROP TABLE note_versions;
ALTER TABLE note_versions_new RENAME TO note_versions;

-- 12. Recreate note_versions indexes
CREATE INDEX idx_note_versions_note_id ON note_versions(note_id, user_id);
CREATE INDEX idx_note_versions_user_id ON note_versions(user_id);
CREATE INDEX idx_note_versions_created_at ON note_versions(created_at);
