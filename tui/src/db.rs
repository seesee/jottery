#![allow(dead_code)]
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;
use tracing::{debug, info};

/// Database manager for Jottery TUI
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a new encrypted database
    ///
    /// # Arguments
    /// * `path` - Path to the database file
    /// * `password` - Password for SQLCipher encryption (derived from user password)
    pub fn open<P: AsRef<Path>>(path: P, password: &str) -> Result<Self> {
        let path = path.as_ref();

        info!("Opening database at: {}", path.display());

        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .context("Failed to open database")?;

        // Enable SQLCipher encryption
        conn.pragma_update(None, "key", password)
            .context("Failed to set database encryption key")?;

        // Configure SQLCipher settings (matching web app security level)
        // Use SQLCipher 4.x defaults for compatibility
        conn.pragma_update(None, "cipher_page_size", 4096)?;
        conn.pragma_update(None, "kdf_iter", 256000)?; // PBKDF2 iterations

        // Verify the database is accessible (this will fail if wrong password)
        conn.pragma_query(None, "user_version", |_| Ok(()))
            .context("Failed to access database (wrong password or corrupted database)")?;

        debug!("SQLCipher encryption enabled");

        let mut db = Self { conn };

        // Always run migrations (will skip already-applied migrations)
        db.run_migrations()?;

        Ok(db)
    }

    /// Create an in-memory database (for testing)
    pub fn in_memory(password: &str) -> Result<Self> {
        info!("Creating in-memory database");

        let conn = Connection::open_in_memory()
            .context("Failed to create in-memory database")?;

        conn.pragma_update(None, "key", password)?;

        let mut db = Self { conn };
        db.run_migrations()?;

        Ok(db)
    }

    /// Run database migrations
    fn run_migrations(&mut self) -> Result<()> {
        info!("Running database migrations...");

        // Get current schema version (0 if table doesn't exist)
        let current_version: i32 = self.conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| row.get(0))
            .unwrap_or(0);

        info!("Current schema version: {}", current_version);

        // List of all migrations
        let migrations = vec![
            (1, include_str!("../migrations/001_initial.sql")),
            (2, include_str!("../migrations/002_add_auto_sync_interval.sql")),
            (3, include_str!("../migrations/003_add_remember_password.sql")),
            (4, include_str!("../migrations/004_update_syntax_default.sql")),
            (5, include_str!("../migrations/005_add_note_versions.sql")),
            (6, include_str!("../migrations/006_add_user_email.sql")),
            (7, include_str!("../migrations/007_add_conflict_data.sql")),
            (8, include_str!("../migrations/008_add_pending_registration.sql")),
            (9, include_str!("../migrations/009_add_note_colors.sql")),
            (10, include_str!("../migrations/010_add_archive_support.sql")),
            (11, include_str!("../migrations/011_add_lock_support.sql")),
            (12, include_str!("../migrations/012_add_deletions_tracking.sql")),
            (13, include_str!("../migrations/013_add_pending_device_name.sql")),
        ];

        // Run pending migrations
        for (version, sql) in migrations {
            if version > current_version {
                info!("Applying migration {}", version);

                // Special handling for migration 11: check if columns already exist
                // (from a previous incomplete migration that didn't record schema version)
                if version == 11 {
                    let has_locked_column: bool = self.conn
                        .query_row(
                            "SELECT COUNT(*) > 0 FROM pragma_table_info('notes') WHERE name = 'locked'",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap_or(false);

                    if has_locked_column {
                        info!("Migration 11: locked column already exists, recording schema version only");
                        self.conn
                            .execute(
                                "INSERT INTO schema_version (version, applied_at) VALUES (11, datetime('now'))",
                                [],
                            )
                            .context("Failed to record migration 11 version")?;
                        continue;
                    }
                }

                self.conn
                    .execute_batch(sql)
                    .context(format!("Failed to apply migration {}", version))?;
                info!("Migration {} applied successfully", version);
            }
        }

        info!("Migrations completed successfully");
        Ok(())
    }

    /// Get the current schema version
    pub fn schema_version(&self) -> Result<i32> {
        let version: i32 = self
            .conn
            .query_row(
                "SELECT MAX(version) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .context("Failed to get schema version")?;

        Ok(version)
    }

    /// Check if encryption metadata exists (i.e., if setup has been completed)
    pub fn is_initialized(&self) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM encryption_metadata WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .context("Failed to check initialization status")?;

        Ok(count > 0)
    }

    /// Get the inner connection (for repository use)
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get a mutable reference to the inner connection
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Close the database connection
    pub fn close(self) -> Result<()> {
        debug!("Closing database connection");
        self.conn.close().map_err(|(_, err)| err)?;
        Ok(())
    }

    /// Count total notes (excluding deleted)
    pub fn count_notes(&self, include_deleted: bool) -> Result<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) FROM notes"
        } else {
            "SELECT COUNT(*) FROM notes WHERE deleted = 0"
        };

        let count: i64 = self
            .conn
            .query_row(query, [], |row| row.get(0))
            .context("Failed to count notes")?;

        Ok(count)
    }

    /// Count attachments
    pub fn count_attachments(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM attachments", [], |row| row.get(0))
            .context("Failed to count attachments")?;

        Ok(count)
    }

    /// Vacuum the database to reclaim space
    pub fn vacuum(&mut self) -> Result<()> {
        info!("Vacuuming database...");
        self.conn
            .execute("VACUUM", [])
            .context("Failed to vacuum database")?;
        info!("Database vacuumed successfully");
        Ok(())
    }

    /// Get database file size in bytes
    pub fn file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
        let metadata = std::fs::metadata(path.as_ref())
            .context("Failed to get database file metadata")?;
        Ok(metadata.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database() {
        let db = Database::in_memory("test_password").unwrap();
        assert!(db.is_initialized().unwrap() == false);
        assert_eq!(db.schema_version().unwrap(), 12);
        assert_eq!(db.count_notes(false).unwrap(), 0);
    }

    #[test]
    #[cfg(feature = "sqlcipher")]
    fn test_wrong_password() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create database with password
        {
            let _db = Database::open(&db_path, "correct_password").unwrap();
        }

        // Try to open with wrong password
        let result = Database::open(&db_path, "wrong_password");
        assert!(result.is_err());
    }

    #[test]
    fn test_database_persistence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Create and close database
        {
            let db = Database::open(&db_path, "password").unwrap();
            assert_eq!(db.schema_version().unwrap(), 12);
        }

        // Reopen and verify
        {
            let db = Database::open(&db_path, "password").unwrap();
            assert_eq!(db.schema_version().unwrap(), 12);
        }
    }
}
