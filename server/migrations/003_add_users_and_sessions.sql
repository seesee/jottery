-- Add user authentication and sessions
-- Transforms device-only registration into multi-user system with approval workflow

-- 1. Create users table
CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,               -- UUID v4
    email TEXT UNIQUE NOT NULL,                 -- Email as username
    password_hash TEXT NOT NULL,                -- Argon2id hash
    created_at TEXT NOT NULL,                   -- ISO 8601
    approved INTEGER DEFAULT 0 NOT NULL,        -- Approval status (0/1)
    approved_at TEXT,                           -- When approved (ISO 8601)
    approved_by TEXT,                           -- Admin user ID
    is_active INTEGER DEFAULT 1 NOT NULL,       -- Soft deactivation (0/1)
    is_admin INTEGER DEFAULT 0 NOT NULL,        -- Admin role flag (0/1)
    storage_quota_mb INTEGER DEFAULT 1000,      -- Storage limit in MB
    last_login_at TEXT,                         -- Last successful auth (ISO 8601)
    FOREIGN KEY (approved_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_approved ON users(approved);
CREATE INDEX idx_users_is_admin ON users(is_admin);

-- 2. Create sessions table (for web dashboard)
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,               -- UUID v4
    user_id TEXT NOT NULL,                      -- Foreign key to users
    token_hash TEXT UNIQUE NOT NULL,            -- SHA-256 of session token
    created_at TEXT NOT NULL,                   -- ISO 8601
    expires_at TEXT NOT NULL,                   -- ISO 8601
    last_used_at TEXT NOT NULL,                 -- ISO 8601
    user_agent TEXT,                            -- Browser/client user agent
    ip_address TEXT,                            -- Client IP address
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- 3. Create default admin user for existing deployments
-- Password is 'changeme' - MUST be changed on first login
-- Hash: argon2id v=19, m=19456, t=2, p=1
INSERT INTO users (id, email, password_hash, created_at, approved, is_admin, is_active)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'admin@localhost',
    '$argon2id$v=19$m=19456,t=2,p=1$EVQp6JMJNY3JYI2sPBp58Q$ooV7ZZOE7PuFFrJA60P0cYLiQH3k7CdBMLaF5bakVUA',
    datetime('now'),
    1,  -- Pre-approved
    1,  -- Is admin
    1   -- Active
);

-- 4. Recreate clients table with user_id foreign key
CREATE TABLE clients_new (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,                      -- NEW: User ownership
    api_key TEXT UNIQUE NOT NULL,
    device_name TEXT NOT NULL,
    device_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_seen_at TEXT,
    is_active INTEGER DEFAULT 1,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_clients_new_api_key ON clients_new(api_key);
CREATE INDEX idx_clients_new_user_id ON clients_new(user_id);
CREATE INDEX idx_clients_new_is_active ON clients_new(is_active);

-- 5. Migrate existing clients to default admin user
INSERT INTO clients_new (id, user_id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
SELECT
    id,
    '00000000-0000-0000-0000-000000000001' as user_id,
    api_key,
    device_name,
    device_type,
    created_at,
    last_seen_at,
    is_active
FROM clients;

-- 6. Replace old clients table
DROP TABLE clients;
ALTER TABLE clients_new RENAME TO clients;

-- 7. Add user_id to notes table
ALTER TABLE notes ADD COLUMN user_id TEXT;

-- 8. Populate user_id in notes from clients
UPDATE notes
SET user_id = (
    SELECT user_id
    FROM clients
    WHERE clients.id = notes.client_id
);

-- 9. Create index for user-based note queries
CREATE INDEX idx_notes_user_id ON notes(user_id);

-- 10. Add user_id to note_versions table (if it exists)
ALTER TABLE note_versions ADD COLUMN user_id TEXT;

-- 11. Populate user_id in note_versions from clients
UPDATE note_versions
SET user_id = (
    SELECT user_id
    FROM clients
    WHERE clients.id = note_versions.client_id
);

-- 12. Create index for user-based version queries
CREATE INDEX idx_note_versions_user_id ON note_versions(user_id);
