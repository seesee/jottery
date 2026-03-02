-- Add envelope encryption fields to encryption_metadata
-- These enable password change without re-encrypting notes

-- Make legacy fields nullable (envelope mode doesn't use them)
-- SQLite doesn't support ALTER COLUMN, but NULL values work with NOT NULL columns
-- when inserted via INSERT OR REPLACE, so we just add the new columns

ALTER TABLE encryption_metadata ADD COLUMN envelope_version INTEGER;
ALTER TABLE encryption_metadata ADD COLUMN device_salt TEXT;
ALTER TABLE encryption_metadata ADD COLUMN local_wrapped_master TEXT;
ALTER TABLE encryption_metadata ADD COLUMN wrapping_kdf_version INTEGER;
