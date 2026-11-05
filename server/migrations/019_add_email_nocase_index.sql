-- Addresses are matched without regard to case from now on, and this keeps that
-- lookup fast. It isn't a UNIQUE index: an existing install can already hold two
-- accounts that differ only by case, and a migration that failed on them would stop
-- the server starting. Registration refuses new ones, and startup logs any that exist.

CREATE INDEX idx_users_email_nocase ON users(email COLLATE NOCASE);
