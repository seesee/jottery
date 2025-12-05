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
        let is_new = !path.exists();

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

        // Run migrations if this is a new database
        if is_new {
            info!("New database detected, running migrations...");
            db.run_migrations()?;
        } else {
            debug!("Existing database opened successfully");
        }

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

        // Read and execute the migration SQL
        let migration_sql = include_str!("../migrations/001_initial.sql");

        self.conn
            .execute_batch(migration_sql)
            .context("Failed to run migrations")?;

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
        assert_eq!(db.schema_version().unwrap(), 1);
        assert_eq!(db.count_notes(false).unwrap(), 0);
    }

    #[test]
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
            assert_eq!(db.schema_version().unwrap(), 1);
        }

        // Reopen and verify
        {
            let db = Database::open(&db_path, "password").unwrap();
            assert_eq!(db.schema_version().unwrap(), 1);
        }
    }
}
