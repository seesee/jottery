#![allow(dead_code)]
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::warn;
use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::{CryptoService, EncryptedData};
use crate::models::{Attachment, Note};

/// The columns every note query reads, in the order `StoredNote::read` expects.
const NOTE_COLUMNS: &str = "id, created_at, modified_at, synced_at, content, tags, attachments,
    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version,
    word_wrap, syntax_language, color, content_hash, parent_hash, hash_chain";

/// Notes that were read, and how many rows couldn't be.
///
/// A row is unreadable when it's stored under another key or it's damaged. Callers
/// that show the list should say so, because the vault otherwise just looks short.
#[derive(Debug, Default)]
pub struct NoteList {
    pub notes: Vec<Note>,
    pub unreadable: usize,
}

/// A notes row as stored, before any decryption.
struct StoredNote {
    id: String,
    created_at: String,
    modified_at: String,
    synced_at: Option<String>,
    content: String,
    tags: String,
    attachments: String,
    pinned: bool,
    archived: bool,
    archived_at: Option<String>,
    locked: bool,
    locked_at: Option<String>,
    deleted: bool,
    deleted_at: Option<String>,
    sync_hash: Option<String>,
    version: i32,
    word_wrap: bool,
    syntax_language: String,
    color: Option<String>,
    content_hash: Option<String>,
    parent_hash: Option<String>,
    hash_chain: Option<String>,
}

impl StoredNote {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            created_at: row.get(1)?,
            modified_at: row.get(2)?,
            synced_at: row.get(3)?,
            content: row.get(4)?,
            tags: row.get(5)?,
            attachments: row.get(6)?,
            pinned: row.get::<_, i32>(7)? != 0,
            archived: row.get::<_, i32>(8)? != 0,
            archived_at: row.get(9)?,
            locked: row.get::<_, i32>(10)? != 0,
            locked_at: row.get(11)?,
            deleted: row.get::<_, i32>(12)? != 0,
            deleted_at: row.get(13)?,
            sync_hash: row.get(14)?,
            version: row.get(15)?,
            word_wrap: row.get::<_, i32>(16)? != 0,
            syntax_language: row.get(17)?,
            color: row.get(18)?,
            content_hash: row.get(19)?,
            parent_hash: row.get(20)?,
            hash_chain: row.get(21)?,
        })
    }

    fn decrypt(self, crypto: &CryptoService, key: &[u8; 32]) -> Result<Note> {
        let encrypted_content: EncryptedData = serde_json::from_str(&self.content)?;
        let encrypted_tags: EncryptedData = serde_json::from_str(&self.tags)?;

        // A stored timestamp that won't parse loses that timestamp, not the note.
        let optional_time = |text: Option<String>| text.and_then(|t| t.parse().ok());

        Ok(Note {
            content: crypto.decrypt_text(&encrypted_content, key)?,
            tags: crypto.decrypt_json(&encrypted_tags, key)?,
            attachments: serde_json::from_str::<Vec<Attachment>>(&self.attachments)?,
            created_at: self.created_at.parse()?,
            modified_at: self.modified_at.parse()?,
            synced_at: optional_time(self.synced_at),
            archived_at: optional_time(self.archived_at),
            locked_at: optional_time(self.locked_at),
            deleted_at: optional_time(self.deleted_at),
            hash_chain: self.hash_chain.and_then(|json| serde_json::from_str(&json).ok()),
            syntax_language: self.syntax_language.parse().unwrap_or_default(),
            id: self.id,
            pinned: self.pinned,
            archived: self.archived,
            locked: self.locked,
            deleted: self.deleted,
            sync_hash: self.sync_hash,
            version: self.version,
            word_wrap: self.word_wrap,
            color: self.color,
            content_hash: self.content_hash,
            parent_hash: self.parent_hash,
        })
    }
}

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
            .map(serde_json::to_string)
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
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1");
        let stored = self.conn
            .query_row(&sql, params![id], StoredNote::read)
            .optional()?;

        stored.map(|note| note.decrypt(&self.crypto, key)).transpose()
    }

    /// Run a note query, keeping every row that can be read and counting the rest.
    fn query_notes(&self, filter: &str, params: impl rusqlite::Params, key: &[u8; 32]) -> Result<NoteList> {
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes {filter}");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params, StoredNote::read)?;

        let mut list = NoteList::default();
        for row in rows {
            let stored = match row {
                Ok(stored) => stored,
                Err(e) => {
                    warn!("[NoteRepository] Skipping row with read error: {e}");
                    list.unreadable += 1;
                    continue;
                }
            };

            let note_id = stored.id.clone();
            match stored.decrypt(&self.crypto, key) {
                Ok(note) => list.notes.push(note),
                Err(e) => {
                    warn!("[NoteRepository] Skipping undecryptable note {note_id}: {e}");
                    list.unreadable += 1;
                }
            }
        }
        if list.unreadable > 0 {
            warn!("[NoteRepository] {} notes could not be read", list.unreadable);
        }

        Ok(list)
    }

    /// Update a note
    pub fn update(&self, note: &Note, key: &[u8; 32]) -> Result<()> {
        // Encrypt content and tags
        let encrypted_content = self.crypto.encrypt_text(&note.content, key)?;
        let encrypted_tags = self.crypto.encrypt_json(&note.tags, key)?;

        let attachments_json = serde_json::to_string(&note.attachments)?;

        // Serialize hash chain (if present)
        let hash_chain_json = note.hash_chain.as_ref()
            .map(serde_json::to_string)
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
        let filter = if include_deleted {
            "ORDER BY modified_at DESC"
        } else {
            "WHERE deleted = 0 AND archived = 0 ORDER BY modified_at DESC"
        };
        Ok(self.query_notes(filter, [], key)?.notes)
    }

    /// Every note that isn't deleted, archived ones included, and a count of rows
    /// that couldn't be read.
    pub fn list_not_deleted(&self, key: &[u8; 32]) -> Result<NoteList> {
        self.query_notes("WHERE deleted = 0 ORDER BY modified_at DESC", [], key)
    }

    /// Every note in the vault, deleted or not, and a count of rows that couldn't
    /// be read. Whatever copies the whole vault needs that count.
    pub fn list_everything(&self, key: &[u8; 32]) -> Result<NoteList> {
        self.query_notes("ORDER BY modified_at DESC", [], key)
    }

    /// Get notes modified after a specific timestamp (for sync)
    pub fn get_modified_after(
        &self,
        timestamp: DateTime<Utc>,
        key: &[u8; 32],
    ) -> Result<Vec<Note>> {
        let list = self.query_notes(
            "WHERE modified_at > ?1 ORDER BY modified_at DESC",
            params![timestamp.to_rfc3339()],
            key,
        )?;
        Ok(list.notes)
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
        Ok(self.query_notes("WHERE deleted = 1 ORDER BY deleted_at DESC", [], key)?.notes)
    }

    /// Get only archived notes
    pub fn get_archived(&self, key: &[u8; 32]) -> Result<Vec<Note>> {
        Ok(self.query_notes("WHERE archived = 1 AND deleted = 0 ORDER BY archived_at DESC", [], key)?.notes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::SyntaxLanguage;

    /// Get test password from environment variable or use default.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        std::env::var("JOTTERY_TEST_PASSWORD")
            .unwrap_or_else(|_| "test_password_placeholder".to_string())
    }

    /// A row whose timestamps can't be parsed, as a damaged database could hold.
    fn damage_timestamps(db: &Database, id: &str) {
        db.connection()
            .execute("UPDATE notes SET created_at = 'not a date', modified_at = 'nor this' WHERE id = ?1", params![id])
            .unwrap();
    }

    #[test]
    fn a_stored_date_that_will_not_parse_costs_that_row_and_nothing_else() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut bad = Note::new("Bad row".to_string());
        bad.deleted = true;
        let mut good = Note::new("Good row".to_string());
        good.deleted = true;
        repo.create(&bad, &key).unwrap();
        repo.create(&good, &key).unwrap();
        damage_timestamps(&db, &bad.id);

        let deleted = repo.get_deleted(&key).unwrap();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].content, "Good row");
    }

    #[test]
    fn archived_notes_survive_a_damaged_neighbour_too() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut bad = Note::new("Bad row".to_string());
        bad.archived = true;
        repo.create(&bad, &key).unwrap();
        damage_timestamps(&db, &bad.id);

        assert!(repo.get_archived(&key).unwrap().is_empty());
    }

    #[test]
    fn the_bin_keeps_a_notes_hash_chain() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Hashed".to_string());
        note.deleted = true;
        note.content_hash = Some("abc".to_string());
        note.hash_chain = Some(vec!["abc".to_string()]);
        repo.create(&note, &key).unwrap();

        // Restoring from the bin writes back what was read here.
        let read = repo.get_deleted(&key).unwrap().remove(0);
        assert_eq!(read.content_hash.as_deref(), Some("abc"));
        assert_eq!(read.hash_chain, Some(vec!["abc".to_string()]));
    }

    #[test]
    fn rows_under_another_key_are_counted() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let other_key = CryptoService::new().generate_master_key();

        repo.create(&Note::new("Mine".to_string()), &key).unwrap();
        repo.create(&Note::new("Not mine".to_string()), &other_key).unwrap();

        let list = repo.list_everything(&key).unwrap();
        assert_eq!(list.notes.len(), 1);
        assert_eq!(list.unreadable, 1);
    }

    fn setup_test_db() -> (Database, [u8; 32]) {
        let password = test_password();
        let db = Database::in_memory(&password).unwrap();
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key(&password, &salt, 100_000).unwrap();
        (db, key)
    }

    // ===== CRUD Operations =====

    #[test]
    fn test_create_note() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test content".to_string());
        let result = repo.create(&note, &key);

        assert!(result.is_ok());
    }

    #[test]
    fn test_get_note() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test content".to_string());
        repo.create(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, note.id);
        assert_eq!(retrieved.content, "Test content");
    }

    #[test]
    fn test_get_nonexistent_note() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let retrieved = repo.get("nonexistent-id", &key).unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_update_note() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Original content".to_string());
        repo.create(&note, &key).unwrap();

        note.content = "Updated content".to_string();
        repo.update(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert_eq!(retrieved.content, "Updated content");
    }

    #[test]
    fn test_delete_note_soft() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        repo.delete(&note.id).unwrap();

        // Note should still exist but be marked deleted
        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(retrieved.deleted);
        assert!(retrieved.deleted_at.is_some());
    }

    #[test]
    fn test_restore_note_via_update() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        repo.delete(&note.id).unwrap();

        // Restore by updating with deleted=false
        note.deleted = false;
        note.deleted_at = None;
        repo.update(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(!retrieved.deleted);
        assert!(retrieved.deleted_at.is_none());
    }

    #[test]
    fn test_hard_delete() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        repo.hard_delete(&note.id).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap();
        assert!(retrieved.is_none());
    }

    // ===== List Operations =====

    #[test]
    fn test_list_excludes_deleted() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();
        repo.delete(&note2.id).unwrap();

        let notes = repo.list(false, &key).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, note1.id);
    }

    #[test]
    fn test_list_includes_deleted() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();
        repo.delete(&note2.id).unwrap();

        let notes = repo.list(true, &key).unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn test_get_deleted_notes() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();
        repo.delete(&note2.id).unwrap();

        let deleted = repo.get_deleted(&key).unwrap();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].id, note2.id);
    }

    // ===== Pin Operations (via update) =====

    #[test]
    fn test_pin_via_update() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        assert!(!repo.get(&note.id, &key).unwrap().unwrap().pinned);

        // Pin via update
        note.pinned = true;
        repo.update(&note, &key).unwrap();
        assert!(repo.get(&note.id, &key).unwrap().unwrap().pinned);

        // Unpin via update
        note.pinned = false;
        repo.update(&note, &key).unwrap();
        assert!(!repo.get(&note.id, &key).unwrap().unwrap().pinned);
    }

    // ===== Archive Operations =====

    #[test]
    fn test_archive_unarchive() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        assert!(!repo.get(&note.id, &key).unwrap().unwrap().archived);

        repo.archive(&note.id).unwrap();
        let archived = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(archived.archived);
        assert!(archived.archived_at.is_some());

        repo.unarchive(&note.id).unwrap();
        let unarchived = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(!unarchived.archived);
        assert!(unarchived.archived_at.is_none());
    }

    #[test]
    fn test_get_archived() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();
        repo.archive(&note2.id).unwrap();

        let archived = repo.get_archived(&key).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, note2.id);
    }

    // ===== Note with Tags =====

    #[test]
    fn test_note_with_tags() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Test".to_string());
        note.tags = vec!["work".to_string(), "important".to_string()];
        repo.create(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert_eq!(retrieved.tags.len(), 2);
        assert!(retrieved.tags.contains(&"work".to_string()));
        assert!(retrieved.tags.contains(&"important".to_string()));
    }

    // ===== Syntax Language =====

    #[test]
    fn test_note_with_syntax_language() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("let x = 1;".to_string());
        note.syntax_language = SyntaxLanguage::Javascript;
        repo.create(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert_eq!(retrieved.syntax_language, SyntaxLanguage::Javascript);
    }

    // ===== Counting =====

    #[test]
    fn test_count_notes() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        assert_eq!(repo.count(false).unwrap(), 0);

        repo.create(&Note::new("1".to_string()), &key).unwrap();
        repo.create(&Note::new("2".to_string()), &key).unwrap();

        assert_eq!(repo.count(false).unwrap(), 2);
    }

    #[test]
    fn test_count_excludes_deleted() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();
        repo.delete(&note.id).unwrap();

        assert_eq!(repo.count(false).unwrap(), 0);
        assert_eq!(repo.count(true).unwrap(), 1);
    }

    // ===== Modified After (for sync) =====

    #[test]
    fn test_get_modified_after() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        // Boundaries come from the note's own timestamp, not a second
        // Utc::now(): on macOS the clock ticks in microseconds, so a
        // `before = Utc::now()` taken just ahead of Note::new() was often
        // equal to modified_at, and the strict comparison then (correctly)
        // left the note out, failing the test intermittently.

        // A note modified after the timestamp is returned...
        let just_before = note.modified_at - chrono::Duration::microseconds(1);
        let modified = repo.get_modified_after(just_before, &key).unwrap();
        assert_eq!(modified.len(), 1);

        // ...but not one modified exactly at it: the comparison is strict.
        let modified = repo.get_modified_after(note.modified_at, &key).unwrap();
        assert_eq!(modified.len(), 0);

        // Notes modified before a future timestamp are not returned
        let far_future = Utc::now() + chrono::Duration::hours(1);
        let modified = repo.get_modified_after(far_future, &key).unwrap();
        assert_eq!(modified.len(), 0);
    }

    // ===== Empty Trash =====

    #[test]
    fn test_empty_trash() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());
        let note3 = Note::new("Note 3".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();
        repo.create(&note3, &key).unwrap();

        repo.delete(&note1.id).unwrap();
        repo.delete(&note2.id).unwrap();

        // Empty trash should remove deleted notes and return count
        let deleted_count = repo.empty_trash().unwrap();
        assert_eq!(deleted_count, 2);

        // Deleted notes should be gone
        assert!(repo.get(&note1.id, &key).unwrap().is_none());
        assert!(repo.get(&note2.id, &key).unwrap().is_none());

        // Non-deleted note should remain
        assert!(repo.get(&note3.id, &key).unwrap().is_some());
    }

    // ===== Note Color =====

    #[test]
    fn test_note_with_color() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Test".to_string());
        note.color = Some("red".to_string());
        repo.create(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert_eq!(retrieved.color, Some("red".to_string()));
    }

    // ===== Word Wrap =====

    #[test]
    fn test_note_word_wrap() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Test".to_string());
        note.word_wrap = false;
        repo.create(&note, &key).unwrap();

        let retrieved = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(!retrieved.word_wrap);
    }

    // ===== Sync Operations =====

    #[test]
    fn test_apply_remote_deletion() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        // Apply remote deletion
        let deleted = repo.apply_remote_deletion(&note.id).unwrap();
        assert!(deleted);

        // Note should be completely gone
        assert!(repo.get(&note.id, &key).unwrap().is_none());
    }

    #[test]
    fn test_apply_remote_deletion_nonexistent() {
        let (db, _key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        // Should return false for nonexistent note
        let deleted = repo.apply_remote_deletion("nonexistent").unwrap();
        assert!(!deleted);
    }

    // ===== Locked Notes (via update) =====

    #[test]
    fn test_lock_unlock_note_via_update() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let mut note = Note::new("Secret".to_string());
        repo.create(&note, &key).unwrap();

        // Lock the note via update
        note.locked = true;
        note.locked_at = Some(Utc::now());
        repo.update(&note, &key).unwrap();

        let locked = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(locked.locked);
        assert!(locked.locked_at.is_some());

        // Unlock the note via update
        note.locked = false;
        note.locked_at = None;
        repo.update(&note, &key).unwrap();

        let unlocked = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(!unlocked.locked);
        assert!(unlocked.locked_at.is_none());
    }
}
