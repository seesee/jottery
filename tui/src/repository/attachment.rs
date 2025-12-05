use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::{CryptoService, EncryptedData};

/// Repository for attachment operations
pub struct AttachmentRepository<'a> {
    conn: &'a Connection,
    crypto: CryptoService,
}

impl<'a> AttachmentRepository<'a> {
    /// Create a new attachment repository
    pub fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            crypto: CryptoService::new(),
        }
    }

    /// Store an attachment (encrypted)
    pub fn store(
        &self,
        id: &str,
        filename: &str,
        mime_type: &str,
        size: i64,
        data: &[u8],
        key: &[u8; 32],
    ) -> Result<()> {
        // Encrypt filename and data
        let encrypted_filename = self.crypto.encrypt_text(filename, key)?;
        let encrypted_data = self.crypto.encrypt_binary(data, key)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO attachments (id, filename, mime_type, size, data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                serde_json::to_string(&encrypted_filename)?,
                mime_type,
                size,
                serde_json::to_string(&encrypted_data)?,
            ],
        )?;

        Ok(())
    }

    /// Get an attachment (decrypted)
    pub fn get(&self, id: &str, key: &[u8; 32]) -> Result<Option<(String, String, i64, Vec<u8>)>> {
        let result = self.conn
            .query_row(
                "SELECT filename, mime_type, size, data FROM attachments WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;

        match result {
            Some((filename_json, mime_type, size, data_json)) => {
                let encrypted_filename: EncryptedData = serde_json::from_str(&filename_json)?;
                let encrypted_data: EncryptedData = serde_json::from_str(&data_json)?;

                let filename = self.crypto.decrypt_text(&encrypted_filename, key)?;
                let data = self.crypto.decrypt_binary(&encrypted_data, key)?;

                Ok(Some((filename, mime_type, size, data)))
            }
            None => Ok(None),
        }
    }

    /// Delete an attachment
    pub fn delete(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM attachments WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Get attachment size
    pub fn get_size(&self, id: &str) -> Result<Option<i64>> {
        let size = self.conn
            .query_row(
                "SELECT size FROM attachments WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(size)
    }

    /// Count all attachments
    pub fn count(&self) -> Result<i64> {
        let count: i64 = self.conn
            .query_row("SELECT COUNT(*) FROM attachments", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get total size of all attachments
    pub fn total_size(&self) -> Result<i64> {
        let size: i64 = self.conn
            .query_row("SELECT COALESCE(SUM(size), 0) FROM attachments", [], |row| {
                row.get(0)
            })?;
        Ok(size)
    }
}
