-- Add envelope encryption support: server-escrowed wrapped master keys
-- Each user can store a wrapped (encrypted) copy of their master encryption key
-- on the server, enabling new device onboarding without credential blob transfer.
-- The wrapped blob is opaque to the server — only the client can unwrap it.

CREATE TABLE wrapped_keys (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wrapped_blob TEXT NOT NULL,
    kdf_version INTEGER NOT NULL DEFAULT 1,
    kdf_iterations INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
