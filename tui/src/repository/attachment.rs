#![allow(dead_code)]
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::{CryptoService, EncryptedData};

/// Attachment data: (filename, mime_type, size, data)
pub type AttachmentData = (String, String, i64, Vec<u8>);

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
    pub fn get(&self, id: &str, key: &[u8; 32]) -> Result<Option<AttachmentData>> {
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

    /// Note that the server has this attachment's file, so a push needn't send it.
    pub fn mark_pushed(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE attachments SET pushed = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Whether the server is known to have this attachment's file
    pub fn is_pushed(&self, id: &str) -> Result<bool> {
        let pushed = self.conn
            .query_row("SELECT pushed FROM attachments WHERE id = ?1", params![id], |row| row.get::<_, i32>(0))
            .optional()?;
        Ok(pushed.is_some_and(|pushed| pushed != 0))
    }

    /// Every attachment's id
    pub fn list_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM attachments")?;
        let ids = stmt.query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    }

    /// Attachments the notes here refer to whose files this device doesn't hold.
    ///
    /// The list reads the notes' own attachment column, which isn't encrypted here. A
    /// note in the bin counts, as it can be restored.
    pub fn list_missing_blobs(&self) -> Result<Vec<crate::models::Attachment>> {
        let held: std::collections::HashSet<String> = self.list_ids()?.into_iter().collect();

        let mut stmt = self.conn.prepare("SELECT attachments FROM notes WHERE attachments != '[]'")?;
        let lists = stmt.query_map([], |row| row.get::<_, String>(0))?;

        let mut seen = std::collections::HashSet::new();
        let mut missing = Vec::new();
        for list in lists {
            let attachments: Vec<crate::models::Attachment> = serde_json::from_str(&list?).unwrap_or_default();
            for attachment in attachments {
                if !held.contains(&attachment.id) && seen.insert(attachment.id.clone()) {
                    missing.push(attachment);
                }
            }
        }

        Ok(missing)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Get test password from environment variable or use default.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        static PASSWORD: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        PASSWORD
            .get_or_init(|| {
                std::env::var("JOTTERY_TEST_PASSWORD")
                    .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
            })
            .clone()
    }

    fn setup_test_db() -> (Database, [u8; 32]) {
        let password = test_password();
        let db = Database::in_memory(&password).unwrap();
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key(&password, &salt, 100_000).unwrap();
        (db, key)
    }

    #[test]
    fn test_store_and_get_attachment() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let id = "test-attachment-id";
        let filename = "test_file.txt";
        let mime_type = "text/plain";
        let data = b"Hello, World!";

        // Store attachment
        repo.store(id, filename, mime_type, data.len() as i64, data, &key).unwrap();

        // Retrieve attachment
        let result = repo.get(id, &key).unwrap();
        assert!(result.is_some());

        let (retrieved_filename, retrieved_mime, retrieved_size, retrieved_data) = result.unwrap();
        assert_eq!(retrieved_filename, filename);
        assert_eq!(retrieved_mime, mime_type);
        assert_eq!(retrieved_size, data.len() as i64);
        assert_eq!(retrieved_data, data);
    }

    #[test]
    fn test_get_nonexistent_attachment() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let result = repo.get("nonexistent", &key).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_attachment() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let id = "test-id";
        repo.store(id, "file.txt", "text/plain", 5, b"hello", &key).unwrap();

        // Verify it exists
        assert!(repo.get(id, &key).unwrap().is_some());

        // Delete it
        repo.delete(id).unwrap();

        // Verify it's gone
        assert!(repo.get(id, &key).unwrap().is_none());
    }

    #[test]
    fn test_get_size() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let id = "test-id";
        let data = b"test data with some length";

        repo.store(id, "file.txt", "text/plain", data.len() as i64, data, &key).unwrap();

        let size = repo.get_size(id).unwrap();
        assert_eq!(size, Some(data.len() as i64));
    }

    #[test]
    fn test_get_size_nonexistent() {
        let (db, _key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let size = repo.get_size("nonexistent").unwrap();
        assert_eq!(size, None);
    }

    #[test]
    fn test_count() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        assert_eq!(repo.count().unwrap(), 0);

        repo.store("id1", "file1.txt", "text/plain", 5, b"hello", &key).unwrap();
        assert_eq!(repo.count().unwrap(), 1);

        repo.store("id2", "file2.txt", "text/plain", 5, b"world", &key).unwrap();
        assert_eq!(repo.count().unwrap(), 2);

        repo.delete("id1").unwrap();
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn test_total_size() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        assert_eq!(repo.total_size().unwrap(), 0);

        repo.store("id1", "file1.txt", "text/plain", 100, b"a", &key).unwrap();
        repo.store("id2", "file2.txt", "text/plain", 200, b"b", &key).unwrap();

        assert_eq!(repo.total_size().unwrap(), 300);
    }

    #[test]
    fn test_store_binary_data() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        // Test with binary data (not valid UTF-8)
        let binary_data: Vec<u8> = (0..=255).collect();
        let id = "binary-attachment";

        repo.store(id, "binary.bin", "application/octet-stream", binary_data.len() as i64, &binary_data, &key).unwrap();

        let (_, _, _, retrieved) = repo.get(id, &key).unwrap().unwrap();
        assert_eq!(retrieved, binary_data);
    }

    #[test]
    fn test_store_replaces_existing() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let id = "same-id";

        repo.store(id, "original.txt", "text/plain", 8, b"original", &key).unwrap();
        repo.store(id, "updated.txt", "text/plain", 7, b"updated", &key).unwrap();

        let (filename, _, _, data) = repo.get(id, &key).unwrap().unwrap();
        assert_eq!(filename, "updated.txt");
        assert_eq!(data, b"updated");

        // Should still be only one attachment
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn test_filename_with_special_chars() {
        let (db, key) = setup_test_db();
        let repo = AttachmentRepository::new(db.connection());

        let filename = "file with spaces & special chars!@#$%.txt";
        repo.store("id", filename, "text/plain", 4, b"test", &key).unwrap();

        let (retrieved_filename, _, _, _) = repo.get("id", &key).unwrap().unwrap();
        assert_eq!(retrieved_filename, filename);
    }
}
