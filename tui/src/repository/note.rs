#![allow(dead_code)]
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

        // Serialize hash chain (if present)
        let hash_chain_json = note.hash_chain.as_ref()
            .map(|c| serde_json::to_string(c))
            .transpose()
            .context("Failed to serialize hash chain")?;

        self.conn.execute(
            "INSERT INTO notes (
                id, created_at, modified_at, synced_at, content, tags, attachments,
                pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color,
                content_hash, parent_hash, hash_chain
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                &note.id,
                note.created_at.to_rfc3339(),
                note.modified_at.to_rfc3339(),
                note.synced_at.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&encrypted_content)?,
                serde_json::to_string(&encrypted_tags)?,
                attachments_json,
                note.pinned as i32,
                note.archived as i32,
                note.archived_at.map(|dt| dt.to_rfc3339()),
                note.locked as i32,
                note.locked_at.map(|dt| dt.to_rfc3339()),
                note.deleted as i32,
                note.deleted_at.map(|dt| dt.to_rfc3339()),
                &note.sync_hash,
                note.version,
                note.word_wrap as i32,
                note.syntax_language.to_string(),
                &note.color,
                &note.content_hash,
                &note.parent_hash,
                hash_chain_json,
            ],
        )?;

        Ok(())
    }

    /// Get a note by ID (decrypted)
    pub fn get(&self, id: &str, key: &[u8; 32]) -> Result<Option<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color,
                    content_hash, parent_hash, hash_chain
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
                    row.get::<_, i32>(8)?,         // archived
                    row.get::<_, Option<String>>(9)?, // archived_at
                    row.get::<_, i32>(10)?,        // locked
                    row.get::<_, Option<String>>(11)?, // locked_at
                    row.get::<_, i32>(12)?,        // deleted
                    row.get::<_, Option<String>>(13)?, // deleted_at
                    row.get::<_, Option<String>>(14)?, // sync_hash
                    row.get::<_, i32>(15)?,        // version
                    row.get::<_, i32>(16)?,        // word_wrap
                    row.get::<_, String>(17)?,     // syntax_language
                    row.get::<_, Option<String>>(18)?, // color
                    row.get::<_, Option<String>>(19)?, // content_hash
                    row.get::<_, Option<String>>(20)?, // parent_hash
                    row.get::<_, Option<String>>(21)?, // hash_chain (JSON)
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
                archived,
                archived_at,
                locked,
                locked_at,
                deleted,
                deleted_at,
                sync_hash,
                version,
                word_wrap,
                syntax_language,
                color,
                content_hash,
                parent_hash,
                hash_chain_json,
            )) => {
                // Decrypt content and tags
                let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
                let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

                let content = self.crypto.decrypt_text(&encrypted_content, key)?;
                let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;

                // Deserialize attachments
                let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

                // Deserialize hash chain
                let hash_chain: Option<Vec<String>> = hash_chain_json
                    .map(|j| serde_json::from_str(&j))
                    .transpose()
                    .context("Failed to deserialize hash chain")?;

                Ok(Some(Note {
                    id,
                    created_at: created_at.parse()?,
                    modified_at: modified_at.parse()?,
                    synced_at: synced_at.map(|s| s.parse()).transpose()?,
                    content,
                    tags,
                    attachments,
                    pinned: pinned != 0,
                    archived: archived != 0,
                    archived_at: archived_at.map(|s| s.parse()).transpose()?,
                    locked: locked != 0,
                    locked_at: locked_at.map(|s| s.parse()).transpose()?,
                    deleted: deleted != 0,
                    deleted_at: deleted_at.map(|s| s.parse()).transpose()?,
                    sync_hash,
                    version,
                    word_wrap: word_wrap != 0,
                    syntax_language: syntax_language.parse().unwrap_or_default(),
                    color,
                    content_hash,
                    parent_hash,
                    hash_chain,
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

        // Serialize hash chain (if present)
        let hash_chain_json = note.hash_chain.as_ref()
            .map(|c| serde_json::to_string(c))
            .transpose()
            .context("Failed to serialize hash chain")?;

        self.conn.execute(
            "UPDATE notes SET
                modified_at = ?1, synced_at = ?2, content = ?3, tags = ?4, attachments = ?5,
                pinned = ?6, archived = ?7, archived_at = ?8, locked = ?9, locked_at = ?10,
                deleted = ?11, deleted_at = ?12, sync_hash = ?13, version = ?14,
                word_wrap = ?15, syntax_language = ?16, color = ?17,
                content_hash = ?18, parent_hash = ?19, hash_chain = ?20
             WHERE id = ?21",
            params![
                note.modified_at.to_rfc3339(),
                note.synced_at.map(|dt| dt.to_rfc3339()),
                serde_json::to_string(&encrypted_content)?,
                serde_json::to_string(&encrypted_tags)?,
                attachments_json,
                note.pinned as i32,
                note.archived as i32,
                note.archived_at.map(|dt| dt.to_rfc3339()),
                note.locked as i32,
                note.locked_at.map(|dt| dt.to_rfc3339()),
                note.deleted as i32,
                note.deleted_at.map(|dt| dt.to_rfc3339()),
                &note.sync_hash,
                note.version,
                note.word_wrap as i32,
                note.syntax_language.to_string(),
                &note.color,
                &note.content_hash,
                &note.parent_hash,
                hash_chain_json,
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

    /// Hard delete a note (records deletion for sync)
    pub fn hard_delete(&self, id: &str) -> Result<()> {
        let now = Utc::now();

        // Record the deletion for sync before removing the note
        self.conn.execute(
            "INSERT OR REPLACE INTO note_deletions (id, deleted_at, synced) VALUES (?1, ?2, 0)",
            params![id, now.to_rfc3339()],
        )?;

        // Now delete the note
        self.conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// List all notes (excluding deleted and archived by default)
    pub fn list(&self, include_deleted: bool, key: &[u8; 32]) -> Result<Vec<Note>> {
        let query = if include_deleted {
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color,
                    content_hash, parent_hash, hash_chain
             FROM notes ORDER BY modified_at DESC"
        } else {
            "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color,
                    content_hash, parent_hash, hash_chain
             FROM notes WHERE deleted = 0 AND archived = 0 ORDER BY modified_at DESC"
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
                row.get::<_, i32>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, i32>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, i32>(15)?,
                row.get::<_, i32>(16)?,
                row.get::<_, String>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<String>>(19)?,
                row.get::<_, Option<String>>(20)?,
                row.get::<_, Option<String>>(21)?,
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
                archived,
                archived_at,
                locked,
                locked_at,
                deleted,
                deleted_at,
                sync_hash,
                version,
                word_wrap,
                syntax_language,
                color,
                content_hash,
                parent_hash,
                hash_chain_json,
            ) = row?;

            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;
            let hash_chain: Option<Vec<String>> = hash_chain_json
                .map(|j| serde_json::from_str(&j))
                .transpose()
                .unwrap_or(None);

            notes.push(Note {
                id,
                created_at: created_at.parse()?,
                modified_at: modified_at.parse()?,
                synced_at: synced_at.map(|s| s.parse()).transpose()?,
                content,
                tags,
                attachments,
                pinned: pinned != 0,
                archived: archived != 0,
                archived_at: archived_at.map(|s| s.parse()).transpose()?,
                locked: locked != 0,
                locked_at: locked_at.map(|s| s.parse()).transpose()?,
                deleted: deleted != 0,
                deleted_at: deleted_at.map(|s| s.parse()).transpose()?,
                sync_hash,
                version,
                word_wrap: word_wrap != 0,
                syntax_language: syntax_language.parse().unwrap_or_default(),
                color,
                content_hash,
                parent_hash,
                hash_chain,
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
                    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color,
                    content_hash, parent_hash, hash_chain
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
                row.get::<_, i32>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, i32>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, i32>(15)?,
                row.get::<_, i32>(16)?,
                row.get::<_, String>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<String>>(19)?,
                row.get::<_, Option<String>>(20)?,
                row.get::<_, Option<String>>(21)?,
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
                archived,
                archived_at,
                locked,
                locked_at,
                deleted,
                deleted_at,
                sync_hash,
                version,
                word_wrap,
                syntax_language,
                color,
                content_hash,
                parent_hash,
                hash_chain_json,
            ) = row?;

            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;
            let hash_chain: Option<Vec<String>> = hash_chain_json
                .map(|j| serde_json::from_str(&j))
                .transpose()
                .unwrap_or(None);

            notes.push(Note {
                id,
                created_at: created_at.parse()?,
                modified_at: modified_at.parse()?,
                synced_at: synced_at.map(|s| s.parse()).transpose()?,
                content,
                tags,
                attachments,
                pinned: pinned != 0,
                archived: archived != 0,
                archived_at: archived_at.map(|s| s.parse()).transpose()?,
                locked: locked != 0,
                locked_at: locked_at.map(|s| s.parse()).transpose()?,
                deleted: deleted != 0,
                deleted_at: deleted_at.map(|s| s.parse()).transpose()?,
                sync_hash,
                version,
                word_wrap: word_wrap != 0,
                syntax_language: syntax_language.parse().unwrap_or_default(),
                color,
                content_hash,
                parent_hash,
                hash_chain,
            });
        }

        Ok(notes)
    }

    /// Count notes (excluding deleted and archived by default)
    pub fn count(&self, include_deleted: bool) -> Result<i64> {
        let query = if include_deleted {
            "SELECT COUNT(*) FROM notes"
        } else {
            "SELECT COUNT(*) FROM notes WHERE deleted = 0 AND archived = 0"
        };

        let count: i64 = self.conn.query_row(query, [], |row| row.get(0))?;
        Ok(count)
    }

    /// Get only deleted notes (for recycle bin)
    pub fn get_deleted(&self, key: &[u8; 32]) -> Result<Vec<Note>> {
        let query = "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color
             FROM notes WHERE deleted = 1 ORDER BY deleted_at DESC";

        let mut stmt = self.conn.prepare(query)?;
        let mut notes = Vec::new();

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,   // id
                row.get::<_, String>(1)?,   // created_at
                row.get::<_, String>(2)?,   // modified_at
                row.get::<_, Option<String>>(3)?,   // synced_at
                row.get::<_, String>(4)?,   // content
                row.get::<_, String>(5)?,   // tags
                row.get::<_, String>(6)?,   // attachments
                row.get::<_, bool>(7)?,     // pinned
                row.get::<_, bool>(8)?,     // archived
                row.get::<_, Option<String>>(9)?,   // archived_at
                row.get::<_, bool>(10)?,    // locked
                row.get::<_, Option<String>>(11)?,  // locked_at
                row.get::<_, bool>(12)?,    // deleted
                row.get::<_, Option<String>>(13)?,  // deleted_at
                row.get::<_, Option<String>>(14)?,  // sync_hash
                row.get::<_, i32>(15)?,     // version
                row.get::<_, bool>(16)?,    // word_wrap
                row.get::<_, String>(17)?,  // syntax_language
                row.get::<_, Option<String>>(18)?,  // color
            ))
        })?;

        for row_result in rows {
            let (id, created_at, modified_at, synced_at, content_json, tags_json, attachments_json,
                 pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color) = row_result?;

            // Decrypt content and tags
            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

            notes.push(Note {
                id,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .unwrap()
                    .with_timezone(&Utc),
                modified_at: DateTime::parse_from_rfc3339(&modified_at)
                    .unwrap()
                    .with_timezone(&Utc),
                synced_at: synced_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                content,
                tags,
                attachments,
                pinned,
                archived,
                archived_at: archived_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                locked,
                locked_at: locked_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                deleted,
                deleted_at: deleted_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                sync_hash,
                version,
                word_wrap,
                syntax_language: syntax_language.parse().unwrap_or_default(),
                color,
                // Hash chain fields not loaded in deleted view
                content_hash: None,
                parent_hash: None,
                hash_chain: None,
            });
        }

        Ok(notes)
    }

    /// Get only archived notes
    pub fn get_archived(&self, key: &[u8; 32]) -> Result<Vec<Note>> {
        let query = "SELECT id, created_at, modified_at, synced_at, content, tags, attachments,
                    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color
             FROM notes WHERE archived = 1 AND deleted = 0 ORDER BY archived_at DESC";

        let mut stmt = self.conn.prepare(query)?;
        let mut notes = Vec::new();

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,   // id
                row.get::<_, String>(1)?,   // created_at
                row.get::<_, String>(2)?,   // modified_at
                row.get::<_, Option<String>>(3)?,   // synced_at
                row.get::<_, String>(4)?,   // content
                row.get::<_, String>(5)?,   // tags
                row.get::<_, String>(6)?,   // attachments
                row.get::<_, bool>(7)?,     // pinned
                row.get::<_, bool>(8)?,     // archived
                row.get::<_, Option<String>>(9)?,   // archived_at
                row.get::<_, bool>(10)?,    // locked
                row.get::<_, Option<String>>(11)?,  // locked_at
                row.get::<_, bool>(12)?,    // deleted
                row.get::<_, Option<String>>(13)?,  // deleted_at
                row.get::<_, Option<String>>(14)?,  // sync_hash
                row.get::<_, i32>(15)?,     // version
                row.get::<_, bool>(16)?,    // word_wrap
                row.get::<_, String>(17)?,  // syntax_language
                row.get::<_, Option<String>>(18)?,  // color
            ))
        })?;

        for row_result in rows {
            let (id, created_at, modified_at, synced_at, content_json, tags_json, attachments_json,
                 pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version, word_wrap, syntax_language, color) = row_result?;

            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

            notes.push(Note {
                id,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .unwrap()
                    .with_timezone(&Utc),
                modified_at: DateTime::parse_from_rfc3339(&modified_at)
                    .unwrap()
                    .with_timezone(&Utc),
                synced_at: synced_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                content,
                tags,
                attachments,
                pinned,
                archived,
                archived_at: archived_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                locked,
                locked_at: locked_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                deleted,
                deleted_at: deleted_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                sync_hash,
                version,
                word_wrap,
                syntax_language: syntax_language.parse().unwrap_or_default(),
                color,
                // Hash chain fields not loaded in get_archived (use dedicated sync queries)
                content_hash: None,
                parent_hash: None,
                hash_chain: None,
            });
        }

        Ok(notes)
    }

    /// Archive a note
    pub fn archive(&self, id: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE notes SET archived = 1, archived_at = ?1, modified_at = ?2 WHERE id = ?3",
            params![now.to_rfc3339(), now.to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Unarchive a note
    pub fn unarchive(&self, id: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE notes SET archived = 0, archived_at = NULL, modified_at = ?1 WHERE id = ?2",
            params![now.to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Permanently delete all notes in recycle bin (records deletions for sync)
    pub fn empty_trash(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();

        // First, record all deletions for sync
        self.conn.execute(
            "INSERT OR REPLACE INTO note_deletions (id, deleted_at, synced)
             SELECT id, ?1, 0 FROM notes WHERE deleted = 1",
            params![now],
        )?;

        // Then delete the notes
        let count = self
            .conn
            .execute("DELETE FROM notes WHERE deleted = 1", [])?;
        Ok(count)
    }

    /// Get pending deletions that need to be synced to server
    pub fn get_pending_deletions(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, deleted_at FROM note_deletions WHERE synced = 0"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })?;

        let mut deletions = Vec::new();
        for row in rows {
            deletions.push(row?);
        }

        Ok(deletions)
    }

    /// Mark deletions as synced after successful push
    pub fn mark_deletions_synced(&self, ids: &[String]) -> Result<()> {
        for id in ids {
            self.conn.execute(
                "UPDATE note_deletions SET synced = 1 WHERE id = ?1",
                params![id],
            )?;
        }
        Ok(())
    }

    /// Apply a remote deletion (from server pull)
    /// This hard-deletes the note locally without recording it for sync
    /// (since we received it from the server, it's already synced)
    pub fn apply_remote_deletion(&self, id: &str) -> Result<bool> {
        // Check if note exists
        let exists: bool = self.conn.query_row(
            "SELECT 1 FROM notes WHERE id = ?1",
            params![id],
            |_| Ok(true),
        ).unwrap_or(false);

        if exists {
            // Delete the note without recording in note_deletions
            // (the deletion came from server, no need to sync back)
            self.conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
            Ok(true)
        } else {
            // Note doesn't exist locally - might have been already deleted
            Ok(false)
        }
    }

    /// Clean up old synced deletions (older than 30 days)
    pub fn cleanup_old_deletions(&self) -> Result<usize> {
        let thirty_days_ago = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        let count = self.conn.execute(
            "DELETE FROM note_deletions WHERE synced = 1 AND deleted_at < ?1",
            params![thirty_days_ago],
        )?;
        Ok(count)
    }
}
