#![allow(dead_code)]
/// Repository for encryption metadata
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;

/// Encryption metadata stored in database
#[derive(Debug, Clone)]
pub struct EncryptionMetadata {
    // Legacy fields (pre-envelope)
    pub salt: Option<Vec<u8>>,
    pub iterations: Option<u32>,
    pub algorithm: String,

    // Envelope encryption fields (post-migration)
    pub envelope_version: Option<u32>,
    pub device_salt: Option<String>,          // Base64-encoded per-device salt
    pub local_wrapped_master: Option<String>, // JSON stringified EncryptedData
    pub wrapping_kdf_version: Option<u32>,    // 1=PBKDF2, 2=Argon2id (future)
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
                "SELECT salt, iterations, algorithm, envelope_version, device_salt, local_wrapped_master, wrapping_kdf_version FROM encryption_metadata WHERE id = 1",
                [],
                |row| {
                    let salt_hex: Option<String> = row.get(0)?;
                    let salt = salt_hex.and_then(|s| {
                        if s.is_empty() { None } else { hex::decode(&s).ok() }
                    });

                    Ok(EncryptionMetadata {
                        salt,
                        iterations: row.get(1)?,
                        algorithm: row.get::<_, Option<String>>(2)?.unwrap_or_else(|| "AES-256-GCM".to_string()),
                        envelope_version: row.get(3)?,
                        device_salt: row.get(4)?,
                        local_wrapped_master: row.get(5)?,
                        wrapping_kdf_version: row.get(6)?,
                    })
                },
            );

        match result {
            Ok(metadata) => Ok(Some(metadata)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e).context("Failed to load encryption metadata"),
        }
    }

    /// Save legacy encryption metadata (first-time setup, pre-envelope)
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

    /// Save envelope encryption metadata (replaces legacy fields)
    pub fn save_envelope(
        &self,
        envelope_version: u32,
        device_salt: &str,
        local_wrapped_master: &str,
        wrapping_kdf_version: u32,
    ) -> Result<()> {
        let created_at = Utc::now().to_rfc3339();

        self.conn
            .execute(
                "INSERT OR REPLACE INTO encryption_metadata (id, salt, iterations, created_at, algorithm, envelope_version, device_salt, local_wrapped_master, wrapping_kdf_version)
                 VALUES (1, NULL, NULL, ?1, 'AES-256-GCM', ?2, ?3, ?4, ?5)",
                rusqlite::params![created_at, envelope_version, device_salt, local_wrapped_master, wrapping_kdf_version],
            )
            .context("Failed to save envelope encryption metadata")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Get test password from environment variable or use default.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        std::env::var("JOTTERY_TEST_PASSWORD")
            .unwrap_or_else(|_| "test_password_placeholder".to_string())
    }

    #[test]
    fn test_encryption_metadata_roundtrip() {
        let db = Database::in_memory(&test_password()).unwrap();
        let repo = EncryptionRepository::new(db.connection());

        // Should be None initially
        assert!(repo.get().unwrap().is_none());

        // Save legacy metadata
        let salt = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        repo.save(&salt, 256_000).unwrap();

        // Load and verify legacy
        let metadata = repo.get().unwrap().unwrap();
        assert_eq!(metadata.salt, Some(salt));
        assert_eq!(metadata.iterations, Some(256_000));
        assert_eq!(metadata.algorithm, "AES-256-GCM");
        assert!(metadata.envelope_version.is_none());
    }

    #[test]
    fn test_envelope_metadata_roundtrip() {
        let db = Database::in_memory(&test_password()).unwrap();
        let repo = EncryptionRepository::new(db.connection());

        // Save envelope metadata
        repo.save_envelope(1, "device_salt_base64", "{\"ciphertext\":\"...\",\"iv\":\"...\"}", 1).unwrap();

        // Load and verify
        let metadata = repo.get().unwrap().unwrap();
        assert!(metadata.salt.is_none());
        assert!(metadata.iterations.is_none());
        assert_eq!(metadata.envelope_version, Some(1));
        assert_eq!(metadata.device_salt.as_deref(), Some("device_salt_base64"));
        assert!(metadata.local_wrapped_master.is_some());
        assert_eq!(metadata.wrapping_kdf_version, Some(1));
    }
}
