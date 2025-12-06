use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::{CryptoService, EncryptedData};
use crate::models::{Attachment, Note};

/// Repository for note operations
pub struct NoteRepository<'a> {
    conn: &'a Connection,
    crypto: CryptoService,
}

impl<'a> NoteRepository<'a> {
    /// Create a new note repository
    pub fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            crypto: CryptoService::new(),
        }
    }

    /// Create a new note (encrypted)
    pub fn create(&self, note: &Note, key: &[u8; 32]) -> Result<()> {
        // Encrypt content and tags
        let encrypted_content = self.crypto.encrypt_text(&note.content, key)?;
        let encrypted_tags = self.crypto.encrypt_json(&note.tags, key)?;

        // Serialize attachments
        let attachments_json = serde_json::to_string(&note.attachments)
            .context("Failed to serialize attachments")?;

        self.conn.execute(
            "INSERT INTO notes (
                id, created_at, modified_at, synced_at, content, tags, attachments,
                pinned, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &note.id,
                note.created_at.to_rfc3339(),
                note.modified_at.to_rfc3339(),
                note.synced_at.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&encrypted_content)?,
                serde_json::to_string(&encrypted_tags)?,
                attachments_json,
                note.pinned as i32,
                note.deleted as i32,
                note.deleted_at.map(|dt| dt.to_rfc3339()),
                &note.sync_hash,
                note.version,
                note.word_wrap as i32,
                note.syntax_language.to_string(),
            ],
        )?;

        Ok(())
    }

    /// Get a note by ID (decrypted)
    pub fn get(&self, id: &str, key: &[u8; 32]) -> Result<Option<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language
             FROM notes WHERE id = ?1"
        )?;

        let note = stmt
            .query_row(params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,      // id
                    row.get::<_, String>(1)?,      // created_at
                    row.get::<_, String>(2)?,      // modified_at
                    row.get::<_, Option<String>>(3)?, // synced_at
                    row.get::<_, String>(4)?,      // content (encrypted)
                    row.get::<_, String>(5)?,      // tags (encrypted)
                    row.get::<_, String>(6)?,      // attachments
                    row.get::<_, i32>(7)?,         // pinned
                    row.get::<_, i32>(8)?,         // deleted
                    row.get::<_, Option<String>>(9)?, // deleted_at
                    row.get::<_, Option<String>>(10)?, // sync_hash
                    row.get::<_, i32>(11)?,        // version
                    row.get::<_, i32>(12)?,        // word_wrap
                    row.get::<_, String>(13)?,     // syntax_language
                ))
            })
            .optional()?;

        match note {
            Some((
                id,
                created_at,
                modified_at,
                synced_at,
                content_json,
                tags_json,
                attachments_json,
                pinned,
                deleted,
                deleted_at,
                sync_hash,
                version,
                word_wrap,
                syntax_language,
            )) => {
                // Decrypt content and tags
                let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
                let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

                let content = self.crypto.decrypt_text(&encrypted_content, key)?;
                let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;

                // Deserialize attachments
                let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

                Ok(Some(Note {
                    id,
                    created_at: created_at.parse()?,
                    modified_at: modified_at.parse()?,
                    synced_at: synced_at.map(|s| s.parse()).transpose()?,
                    content,
                    tags,
                    attachments,
                    pinned: pinned != 0,
                    deleted: deleted != 0,
                    deleted_at: deleted_at.map(|s| s.parse()).transpose()?,
                    sync_hash,
                    version,
                    word_wrap: word_wrap != 0,
                    syntax_language: syntax_language.parse().unwrap_or_default(),
                }))
            }
            None => Ok(None),
        }
    }

    /// Update a note
    pub fn update(&self, note: &Note, key: &[u8; 32]) -> Result<()> {
        // Encrypt content and tags
        let encrypted_content = self.crypto.encrypt_text(&note.content, key)?;
        let encrypted_tags = self.crypto.encrypt_json(&note.tags, key)?;

        let attachments_json = serde_json::to_string(&note.attachments)?;

        self.conn.execute(
            "UPDATE notes SET
                modified_at = ?1, synced_at = ?2, content = ?3, tags = ?4, attachments = ?5,
                pinned = ?6, deleted = ?7, deleted_at = ?8, sync_hash = ?9, version = ?10,
                word_wrap = ?11, syntax_language = ?12
             WHERE id = ?13",
            params![
                note.modified_at.to_rfc3339(),
                note.synced_at.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&encrypted_content)?,
                serde_json::to_string(&encrypted_tags)?,
                attachments_json,
                note.pinned as i32,
                note.deleted as i32,
                note.deleted_at.map(|dt| dt.to_rfc3339()),
                &note.sync_hash,
                note.version,
                note.word_wrap as i32,
                note.syntax_language.to_string(),
                &note.id,
            ],
        )?;

        Ok(())
    }

    /// Delete a note (soft delete)
    pub fn delete(&self, id: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE notes SET deleted = 1, deleted_at = ?1, modified_at = ?2 WHERE id = ?3",
            params![now.to_rfc3339(), now.to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Hard delete a note
    pub fn hard_delete(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// List all notes (excluding deleted by default)
    pub fn list(&self, include_deleted: bool, key: &[u8; 32]) -> Result<Vec<Note>> {
        let query = if include_deleted {
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language
             FROM notes ORDER BY modified_at DESC"
        } else {
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language
             FROM notes WHERE deleted = 0 ORDER BY modified_at DESC"
        };

        let mut stmt = self.conn.prepare(query)?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i32>(7)?,
                row.get::<_, i32>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, i32>(11)?,
                row.get::<_, i32>(12)?,
                row.get::<_, String>(13)?,
            ))
        })?;

        let mut notes = Vec::new();
        for row in rows {
            let (
                id,
                created_at,
                modified_at,
                synced_at,
                content_json,
                tags_json,
                attachments_json,
                pinned,
                deleted,
                deleted_at,
                sync_hash,
                version,
                word_wrap,
                syntax_language,
            ) = row?;

            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

            notes.push(Note {
                id,
                created_at: created_at.parse()?,
                modified_at: modified_at.parse()?,
                synced_at: synced_at.map(|s| s.parse()).transpose()?,
                content,
                tags,
                attachments,
                pinned: pinned != 0,
                deleted: deleted != 0,
                deleted_at: deleted_at.map(|s| s.parse()).transpose()?,
                sync_hash,
                version,
                word_wrap: word_wrap != 0,
                syntax_language: syntax_language.parse().unwrap_or_default(),
            });
        }

        Ok(notes)
    }

    /// Get notes modified after a specific timestamp (for sync)
    pub fn get_modified_after(
        &self,
        timestamp: DateTime<Utc>,
        key: &[u8; 32],
    ) -> Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language
             FROM notes WHERE modified_at > ?1 ORDER BY modified_at DESC"
        )?;

        let rows = stmt.query_map(params![timestamp.to_rfc3339()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i32>(7)?,
                row.get::<_, i32>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, i32>(11)?,
                row.get::<_, i32>(12)?,
                row.get::<_, String>(13)?,
            ))
        })?;

        let mut notes = Vec::new();
        for row in rows {
            let (
                id,
                created_at,
                modified_at,
                synced_at,
                content_json,
                tags_json,
                attachments_json,
                pinned,
                deleted,
                deleted_at,
                sync_hash,
                version,
                word_wrap,
                syntax_language,
            ) = row?;

            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

            notes.push(Note {
                id,
                created_at: created_at.parse()?,
                modified_at: modified_at.parse()?,
                synced_at: synced_at.map(|s| s.parse()).transpose()?,
                content,
                tags,
                attachments,
                pinned: pinned != 0,
                deleted: deleted != 0,
                deleted_at: deleted_at.map(|s| s.parse()).transpose()?,
                sync_hash,
                version,
                word_wrap: word_wrap != 0,
                syntax_language: syntax_language.parse().unwrap_or_default(),
            });
        }

        Ok(notes)
    }

    /// Count notes
    pub fn count(&self, include_deleted: bool) -> Result<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) FROM notes"
        } else {
            "SELECT COUNT(*) FROM notes WHERE deleted = 0"
        };

        let count: i64 = self.conn.query_row(query, [], |row| row.get(0))?;
        Ok(count)
    }
}
