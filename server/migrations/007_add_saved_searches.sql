-- Add saved_searches table for storing user search queries
-- Allows users to save and sync frequently used search queries across devices

-- Create saved_searches table
CREATE TABLE saved_searches (
    id TEXT PRIMARY KEY NOT NULL,               -- UUID v4
    user_id TEXT NOT NULL,                      -- User ownership
    name TEXT NOT NULL,                         -- User-provided search name
    query TEXT NOT NULL,                        -- Search query string
    "order" INTEGER NOT NULL,                   -- Display order (quoted because ORDER is SQL keyword)
    created_at TEXT NOT NULL,                   -- ISO 8601
    modified_at TEXT NOT NULL,                  -- ISO 8601
    synced_at TEXT,                             -- ISO 8601
    deleted INTEGER DEFAULT 0 NOT NULL,         -- Soft delete flag (0/1)
    deleted_at TEXT,                            -- Deletion timestamp (ISO 8601)
    version INTEGER DEFAULT 1 NOT NULL,         -- Optimistic locking
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Indexes for efficient queries
CREATE INDEX idx_saved_searches_user_id ON saved_searches(user_id);
CREATE INDEX idx_saved_searches_deleted ON saved_searches(deleted);
CREATE INDEX idx_saved_searches_user_deleted ON saved_searches(user_id, deleted);
CREATE INDEX idx_saved_searches_modified ON saved_searches(modified_at);
CREATE INDEX idx_saved_searches_user_order ON saved_searches(user_id, "order");
