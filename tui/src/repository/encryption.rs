/// Repository for encryption metadata
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

/// Encryption metadata stored in database
#[derive(Debug, Clone)]
pub struct EncryptionMetadata {
    pub salt: Vec<u8>,
    pub iterations: u32,
    pub algorithm: String,
}

/// Repository for managing encryption metadata
pub struct EncryptionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> EncryptionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get encryption metadata (if exists)
    pub fn get(&self) -> Result<Option<EncryptionMetadata>> {
        let result = self
            .conn
            .query_row(
                "SELECT salt, iterations, algorithm FROM encryption_metadata WHERE id = 1",
                [],
                |row| {
                    let salt_hex: String = row.get(0)?;
                    let salt = hex::decode(&salt_hex)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?;

                    Ok(EncryptionMetadata {
                        salt,
                        iterations: row.get(1)?,
                        algorithm: row.get(2)?,
                    })
                },
            );

        match result {
            Ok(metadata) => Ok(Some(metadata)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context("Failed to load encryption metadata"),
        }
    }

    /// Save encryption metadata (first-time setup)
    pub fn save(&self, salt: &[u8], iterations: u32) -> Result<()> {
        let salt_hex = hex::encode(salt);
        let created_at = Utc::now().to_rfc3339();

        self.conn
            .execute(
                "INSERT OR REPLACE INTO encryption_metadata (id, salt, iterations, created_at, algorithm)
                 VALUES (1, ?1, ?2, ?3, 'AES-256-GCM')",
                (salt_hex, iterations, created_at),
            )
            .context("Failed to save encryption metadata")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn test_encryption_metadata_roundtrip() {
        let db = Database::in_memory("test_password").unwrap();
        let repo = EncryptionRepository::new(db.connection());

        // Should be None initially
        assert!(repo.get().unwrap().is_none());

        // Save metadata
        let salt = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        repo.save(&salt, 256_000).unwrap();

        // Load and verify
        let metadata = repo.get().unwrap().unwrap();
        assert_eq!(metadata.salt, salt);
        assert_eq!(metadata.iterations, 256_000);
        assert_eq!(metadata.algorithm, "AES-256-GCM");
    }
}
