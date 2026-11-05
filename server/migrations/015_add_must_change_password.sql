-- Forces an account to set a new password before it can do anything else.
-- Set at startup for the seeded admin while it still has the published default
-- password (see src/bootstrap.rs).

ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0;
