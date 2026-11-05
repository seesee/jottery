-- Older releases kept the remembered password here, encrypted with a constant from the
-- source, and later ones stopped reading it without ever clearing it. Nothing has used
-- this column since, so the value goes. Remember Password has its own storage now.
UPDATE settings SET stored_password = NULL;

INSERT INTO schema_version (version, applied_at) VALUES (23, datetime('now'));
