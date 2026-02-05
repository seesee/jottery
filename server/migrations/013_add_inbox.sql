-- Add inbox support: token hash and quota columns on users, inbox_items table
ALTER TABLE users ADD COLUMN inbox_token_hash TEXT;
ALTER TABLE users ADD COLUMN inbox_max_items INTEGER DEFAULT 100;
ALTER TABLE users ADD COLUMN inbox_max_size_mb INTEGER DEFAULT 10;

CREATE TABLE inbox_items (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    source TEXT,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_inbox_items_user_id ON inbox_items(user_id);
CREATE INDEX idx_inbox_items_created_at ON inbox_items(created_at);
