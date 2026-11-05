#![allow(dead_code)]
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::warn;
use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::{update_hash_chain, CryptoService, EncryptedData};
use crate::models::{Attachment, Note, SyntaxLanguage};

/// The columns every note query reads, in the order `StoredNote::read` expects.
const NOTE_COLUMNS: &str = "id, created_at, modified_at, synced_at, content, tags, attachments,
    pinned, archived, archived_at, locked, locked_at, deleted, deleted_at, sync_hash, version,
    word_wrap, syntax_language, color, content_hash, parent_hash, hash_chain,
    show_preview, needs_sync, attachments_complete";

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
    show_preview: Option<bool>,
    needs_sync: bool,
    attachments_complete: bool,
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
            show_preview: row.get::<_, Option<i32>>(22)?.map(|value| value != 0),
            needs_sync: row.get::<_, i32>(23)? != 0,
            attachments_complete: row.get::<_, i32>(24)? != 0,
        })
    }

    fn decrypt(self, crypto: &CryptoService, key: &[u8; 32]) -> Result<Note> {
        let encrypted_content: EncryptedData = serde_json::from_str(&self.content)?;
        let encrypted_tags: EncryptedData = serde_json::from_str(&self.tags)?;

        // A stored timestamp that won't parse loses that timestamp, not the note.
        let optional_time = |text: Option<String>| text.and_then(|t| t.parse().ok());
        let (syntax_language, syntax_unknown) = SyntaxLanguage::from_name(&self.syntax_language);

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
            syntax_language,
            syntax_unknown,
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
            show_preview: self.show_preview,
            needs_sync: self.needs_sync,
            attachments_complete: self.attachments_complete,
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

    /// Create a new note (encrypted).
    ///
    /// A note written here starts its hash chain, so the server can tell later copies
    /// apart. One that already has a hash came from somewhere that gave it one.
    pub fn create(&self, note: &Note, key: &[u8; 32]) -> Result<()> {
        let mut hashed;
        let note = if note.needs_sync && note.content_hash.is_none() {
            hashed = note.clone();
            let hash = self.crypto.compute_content_hash(&hashed, key);
            hashed.hash_chain = Some(update_hash_chain(&hash, None));
            hashed.content_hash = Some(hash);
            &hashed
        } else {
            note
        };

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
                content_hash, parent_hash, hash_chain, show_preview, needs_sync, attachments_complete
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)",
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
                note.syntax_name(),
                &note.color,
                &note.content_hash,
                &note.parent_hash,
                hash_chain_json,
                note.show_preview.map(|value| value as i32),
                note.needs_sync as i32,
                note.attachments_complete as i32,
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

    /// The stored hash fields, with this version of the note added on top if it differs.
    ///
    /// These come from the row, not from `note`, which may be an older copy from memory.
    fn advance_hash_chain(&self, note: &Note, key: &[u8; 32]) -> Result<(Option<String>, Option<String>, Option<Vec<String>>)> {
        let stored: Option<(Option<String>, Option<String>, Option<String>)> = self.conn
            .query_row(
                "SELECT content_hash, parent_hash, hash_chain FROM notes WHERE id = ?1",
                params![&note.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (stored_hash, stored_parent, stored_chain) = stored.unwrap_or_default();
        let stored_chain: Option<Vec<String>> = stored_chain.and_then(|json| serde_json::from_str(&json).ok());

        let hash = self.crypto.compute_content_hash(note, key);
        if stored_hash.as_deref() == Some(hash.as_str()) {
            return Ok((stored_hash, stored_parent, stored_chain));
        }

        let chain = update_hash_chain(&hash, stored_chain.as_deref());
        Ok((Some(hash), stored_hash, Some(chain)))
    }

    /// Save a change made on this device, which the server still has to hear about.
    pub fn update(&self, note: &Note, key: &[u8; 32]) -> Result<()> {
        let (content_hash, parent_hash, hash_chain) = self.advance_hash_chain(note, key)?;

        let mut changed = note.clone();
        changed.content_hash = content_hash;
        changed.parent_hash = parent_hash;
        changed.hash_chain = hash_chain;
        changed.needs_sync = true;

        self.store(&changed, key)
    }

    /// Write a note exactly as given, hash fields and sync state included. This is for
    /// what arrives from elsewhere. A change made here goes through `update`.
    pub fn store(&self, note: &Note, key: &[u8; 32]) -> Result<()> {
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
                content_hash = ?18, parent_hash = ?19, hash_chain = ?20,
                show_preview = ?21, needs_sync = ?22, attachments_complete = ?23,
                remote_deleted_at = CASE WHEN ?11 = 0 THEN NULL ELSE remote_deleted_at END
             WHERE id = ?24",
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
                note.syntax_name(),
                &note.color,
                &note.content_hash,
                &note.parent_hash,
                hash_chain_json,
                note.show_preview.map(|value| value as i32),
                note.needs_sync as i32,
                note.attachments_complete as i32,
                &note.id,
            ],
        )?;

        Ok(())
    }

    /// Delete a note (soft delete)
    pub fn delete(&self, id: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE notes SET deleted = 1, deleted_at = ?1, modified_at = ?2, needs_sync = 1 WHERE id = ?3",
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

    /// Notes with a change for the server. A note waiting for its conflict to be
    /// resolved stays here, because pushing it again would only be refused again.
    pub fn get_pending(&self, key: &[u8; 32]) -> Result<NoteList> {
        self.query_notes(
            "WHERE needs_sync = 1 AND id NOT IN (
                SELECT note_id FROM note_sync_metadata WHERE last_sync_status = 'conflict'
             ) ORDER BY modified_at ASC",
            [],
            key,
        )
    }

    /// The server has this note as `pushed`. If it changed again since being read for
    /// the push, that later change still counts as pending. A pin or a colour leaves
    /// modified_at alone, so the hash has to match too, and nobody's hash covers deleted.
    pub fn mark_pushed(&self, pushed: &Note, synced_at: DateTime<Utc>) -> Result<()> {
        self.conn.execute(
            "UPDATE notes SET needs_sync = 0, synced_at = ?1
             WHERE id = ?2 AND modified_at = ?3 AND content_hash IS ?4 AND deleted = ?5",
            params![
                synced_at.to_rfc3339(),
                &pushed.id,
                pushed.modified_at.to_rfc3339(),
                &pushed.content_hash,
                pushed.deleted as i32,
            ],
        )?;
        Ok(())
    }

    /// The server won't take this note because it was permanently deleted after this
    /// edit. Stop sending it, and treat it as any other deletion from the server, unless
    /// it's changed since it was read for the push: an edit later than the deletion beats
    /// the deletion, so that one goes on the next push.
    pub fn hold_skipped_note(&self, pushed: &Note) -> Result<()> {
        let unchanged = self.conn.execute(
            "UPDATE notes SET needs_sync = 0
             WHERE id = ?1 AND modified_at = ?2 AND content_hash IS ?3 AND deleted = ?4",
            params![&pushed.id, pushed.modified_at.to_rfc3339(), &pushed.content_hash, pushed.deleted as i32],
        )?;

        if unchanged > 0 {
            self.hold_deletion(&pushed.id)?;
        }
        Ok(())
    }

    /// Give a note from an older build its first hash, unless it's changed and got one.
    pub fn set_first_hash(&self, id: &str, hash: &str, chain: &[String]) -> Result<()> {
        self.conn.execute(
            "UPDATE notes SET content_hash = ?1, hash_chain = ?2 WHERE id = ?3 AND content_hash IS NULL",
            params![hash, serde_json::to_string(chain)?, id],
        )?;
        Ok(())
    }

    /// Whether this vault has a row for this note, readable or not
    pub fn exists(&self, id: &str) -> Result<bool> {
        Ok(self.conn
            .query_row("SELECT 1 FROM notes WHERE id = ?1", params![id], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Put every note up for pushing, as the first sync with an account does.
    pub fn mark_all_pending(&self) -> Result<()> {
        self.conn.execute("UPDATE notes SET needs_sync = 1", [])?;
        Ok(())
    }

    /// Whether this note has a change for the server made after `time`.
    pub fn has_edit_after(&self, id: &str, time: DateTime<Utc>) -> Result<bool> {
        let modified_at: Option<String> = self.conn
            .query_row(
                "SELECT modified_at FROM notes WHERE id = ?1 AND needs_sync = 1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;

        Ok(modified_at.and_then(|text| text.parse::<DateTime<Utc>>().ok()).is_some_and(|at| at > time))
    }

    /// Mark as pending every note changed since the last sync, which is how a restore
    /// decides what the server hasn't seen.
    pub fn mark_changed_since_last_sync(&self) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE notes SET needs_sync = 1
             WHERE modified_at > COALESCE((SELECT last_sync_at FROM sync_metadata WHERE id = 1), '')",
            [],
        )?)
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
            "UPDATE notes SET archived = 1, archived_at = ?1, modified_at = ?2, needs_sync = 1 WHERE id = ?3",
            params![now.to_rfc3339(), now.to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Unarchive a note
    pub fn unarchive(&self, id: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE notes SET archived = 0, archived_at = NULL, modified_at = ?1, needs_sync = 1 WHERE id = ?2",
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
        self.remove_for_good("deleted = 1", params![])
    }

    /// Get pending deletions that need to be synced to server
    /// Deletions to send. A hard delete removes the note, so one whose note exists
    /// again is stale: the note came back from a backup or an import, and sending
    /// it would delete the note again, on every device.
    pub fn get_pending_deletions(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, deleted_at FROM note_deletions
             WHERE synced = 0 AND id NOT IN (SELECT id FROM notes)"
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

    /// Stop sending deletions the server has refused. The note comes back through a
    /// pull, so there's nothing else to do.
    pub fn forget_deletions(&self, ids: &[String]) -> Result<()> {
        for id in ids {
            self.conn.execute("DELETE FROM note_deletions WHERE id = ?1", params![id])?;
        }
        Ok(())
    }

    /// Note that the server says this note was permanently deleted. Nothing happens to
    /// it yet: `apply_held_deletions` or `refuse_held_deletions` settles that.
    pub fn hold_deletion(&self, id: &str) -> Result<()> {
        self.conn.execute("INSERT OR IGNORE INTO held_deletions (id) VALUES (?1)", params![id])?;
        Ok(())
    }

    /// How many notes still visible here the held deletions would take away.
    pub fn count_held_deletions(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM held_deletions
             JOIN notes ON notes.id = held_deletions.id
             WHERE notes.deleted = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Whether the user has been asked about deletions and hasn't answered yet.
    pub fn has_unanswered_deletions(&self) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM held_deletions WHERE asked = 1)",
            [],
            |row| row.get(0),
        )?)
    }

    /// Note that the user is being asked about everything held. It stays held until
    /// they answer, whatever happens to some of it in the meantime.
    pub fn mark_held_deletions_asked(&self) -> Result<()> {
        self.conn.execute("UPDATE held_deletions SET asked = 1", [])?;
        Ok(())
    }

    /// How many notes have reached the bin from elsewhere since `since`, by this device's
    /// clock. A server staying under the limit on each sync adds up here.
    pub fn count_arrived_in_bin_since(&self, since: DateTime<Utc>) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE deleted = 1 AND remote_deleted_at >= ?1",
            params![since.to_rfc3339()],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Move held deletions into the bin, returning how many notes went there.
    ///
    /// The server's word isn't proof that one of this user's devices asked for this, so
    /// these stay restorable. The time is this device's, so the server can't say the
    /// note has been there for months. No deletion is recorded: it came from the server.
    pub fn apply_held_deletions(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();

        let moved = self.conn.execute(
            "UPDATE notes SET deleted = 1, deleted_at = ?1, needs_sync = 0
             WHERE deleted = 0 AND id IN (SELECT id FROM held_deletions)",
            params![now],
        )?;
        self.conn.execute(
            "UPDATE notes SET remote_deleted_at = ?1
             WHERE remote_deleted_at IS NULL AND id IN (SELECT id FROM held_deletions)",
            params![now],
        )?;
        self.conn.execute("DELETE FROM held_deletions", [])?;

        Ok(moved)
    }

    /// Keep what the held deletions named, and push those notes back to the server. An
    /// edit later than a deletion beats it, so they return to every device.
    pub fn refuse_held_deletions(&self) -> Result<usize> {
        let kept = self.conn.execute(
            "UPDATE notes SET modified_at = ?1, needs_sync = 1
             WHERE id IN (SELECT id FROM held_deletions)",
            params![Utc::now().to_rfc3339()],
        )?;
        self.conn.execute("DELETE FROM held_deletions", [])?;

        Ok(kept)
    }

    /// Permanently remove notes deleted elsewhere that have been in the bin long enough.
    pub fn purge_remote_deletions(&self, older_than: DateTime<Utc>) -> Result<usize> {
        self.remove_for_good("remote_deleted_at < ?1", params![older_than.to_rfc3339()])
    }

    /// Permanently remove what was deleted here and has been in the bin since before
    /// `older_than`, telling the server. The date is this device's own. A note that came
    /// here deleted has `remote_deleted_at` instead, and doesn't get this far, because a
    /// date from the server shouldn't be able to empty the bin.
    pub fn purge_expired_bin(&self, older_than: DateTime<Utc>) -> Result<usize> {
        const EXPIRED: &str = "deleted = 1 AND remote_deleted_at IS NULL AND deleted_at < ?1";
        let cutoff = older_than.to_rfc3339();

        self.conn.execute(
            &format!(
                "INSERT OR REPLACE INTO note_deletions (id, deleted_at, synced)
                 SELECT id, ?2, 0 FROM notes WHERE {EXPIRED}"
            ),
            params![&cutoff, Utc::now().to_rfc3339()],
        )?;

        self.remove_for_good(EXPIRED, params![&cutoff])
    }

    /// Note that a note arrived in the bin from elsewhere, which starts its 30 days now.
    pub fn mark_arrived_in_bin(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE notes SET remote_deleted_at = COALESCE(remote_deleted_at, ?1) WHERE id = ?2 AND deleted = 1",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Remove the notes matching `condition`, and everything that only they needed: their
    /// versions, their sync state, and any file nothing else refers to. Nothing else does
    /// this, as the database has no foreign keys.
    fn remove_for_good(&self, condition: &str, parameters: &[&dyn rusqlite::ToSql]) -> Result<usize> {
        let mut stmt = self.conn.prepare(&format!("SELECT id FROM notes WHERE {condition}"))?;
        let ids: Vec<String> = stmt.query_map(parameters, |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;

        let mut attachment_ids = std::collections::HashSet::new();
        for id in &ids {
            let mut lists = self.conn.prepare(
                "SELECT attachments FROM notes WHERE id = ?1
                 UNION ALL SELECT attachments FROM note_versions WHERE note_id = ?1",
            )?;
            for list in lists.query_map(params![id], |row| row.get::<_, String>(0))? {
                let attachments: Vec<crate::models::Attachment> = serde_json::from_str(&list?).unwrap_or_default();
                attachment_ids.extend(attachments.into_iter().map(|attachment| attachment.id));
            }

            self.conn.execute("DELETE FROM note_versions WHERE note_id = ?1", params![id])?;
            self.conn.execute("DELETE FROM note_sync_metadata WHERE note_id = ?1", params![id])?;
            self.conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        }

        for attachment_id in attachment_ids {
            if !self.attachment_in_use(&attachment_id)? {
                self.conn.execute("DELETE FROM attachments WHERE id = ?1", params![&attachment_id])?;
            }
        }

        Ok(ids.len())
    }

    /// Whether any note or version still refers to this attachment. Notes can share one,
    /// and restoring a version needs what it had.
    pub fn attachment_in_use(&self, attachment_id: &str) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM notes WHERE instr(attachments, ?1) > 0)
                 OR EXISTS (SELECT 1 FROM note_versions WHERE instr(attachments, ?1) > 0)",
            params![attachment_id],
            |row| row.get(0),
        )?)
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
    fn a_new_note_starts_its_hash_chain() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let note = Note::new("First".to_string());

        repo.create(&note, &key).unwrap();

        let stored = repo.get(&note.id, &key).unwrap().unwrap();
        let hash = stored.content_hash.clone().unwrap();
        assert_eq!(stored.hash_chain, Some(vec![hash]));
        assert_eq!(stored.parent_hash, None);
    }

    #[test]
    fn saving_a_change_adds_to_the_hash_chain() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let mut note = Note::new("First".to_string());
        repo.create(&note, &key).unwrap();
        let first = repo.get(&note.id, &key).unwrap().unwrap().content_hash.unwrap();

        note.content = "Second".to_string();
        repo.update(&note, &key).unwrap();

        let stored = repo.get(&note.id, &key).unwrap().unwrap();
        let second = stored.content_hash.clone().unwrap();
        assert_ne!(second, first);
        assert_eq!(stored.parent_hash, Some(first.clone()));
        assert_eq!(stored.hash_chain, Some(vec![second, first]));
    }

    #[test]
    fn saving_with_nothing_changed_leaves_the_hash_chain_alone() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let note = Note::new("Same".to_string());
        repo.create(&note, &key).unwrap();
        let before = repo.get(&note.id, &key).unwrap().unwrap();

        repo.update(&note, &key).unwrap();

        let after = repo.get(&note.id, &key).unwrap().unwrap();
        assert_eq!(after.content_hash, before.content_hash);
        assert_eq!(after.hash_chain, before.hash_chain);
    }

    /// A list in memory can hold a copy from before the last save. The chain comes from
    /// the row, or an older copy would drop the hashes in between.
    #[test]
    fn a_stale_copy_in_memory_does_not_cut_the_hash_chain() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let mut stale = Note::new("First".to_string());
        repo.create(&stale, &key).unwrap();

        let mut other = repo.get(&stale.id, &key).unwrap().unwrap();
        other.content = "Second".to_string();
        repo.update(&other, &key).unwrap();

        stale.content = "Third".to_string();
        repo.update(&stale, &key).unwrap();

        let stored = repo.get(&stale.id, &key).unwrap().unwrap();
        assert_eq!(stored.hash_chain.unwrap().len(), 3);
    }

    #[test]
    fn a_change_made_while_pushing_stays_pending() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let note = Note::new("Sent".to_string());
        repo.create(&note, &key).unwrap();

        let mut later = repo.get(&note.id, &key).unwrap().unwrap();
        later.content = "Changed while the push was out".to_string();
        later.touch();
        repo.update(&later, &key).unwrap();

        repo.mark_pushed(&note, Utc::now()).unwrap();

        assert!(repo.get(&note.id, &key).unwrap().unwrap().needs_sync);
    }

    /// A pin leaves modified_at alone, so that can't be what tells the two apart.
    #[test]
    fn a_pin_made_while_pushing_stays_pending() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        repo.create(&Note::new("Sent".to_string()), &key).unwrap();
        let sent = repo.get_pending(&key).unwrap().notes.remove(0);

        let mut pinned = sent.clone();
        pinned.pinned = true;
        repo.update(&pinned, &key).unwrap();

        repo.mark_pushed(&sent, Utc::now()).unwrap();

        assert!(repo.get(&sent.id, &key).unwrap().unwrap().needs_sync);
    }

    #[test]
    fn a_note_unchanged_since_the_push_is_no_longer_pending() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        repo.create(&Note::new("Sent".to_string()), &key).unwrap();
        let sent = repo.get_pending(&key).unwrap().notes.remove(0);

        repo.mark_pushed(&sent, Utc::now()).unwrap();

        assert!(!repo.get(&sent.id, &key).unwrap().unwrap().needs_sync);
    }

    fn binned_note(repo: &NoteRepository<'_>, key: &[u8; 32], deleted_at: DateTime<Utc>) -> Note {
        let mut note = Note::new("In the bin".to_string());
        note.deleted = true;
        note.deleted_at = Some(deleted_at);
        repo.create(&note, key).unwrap();
        repo.store(&note, key).unwrap();
        note
    }

    #[test]
    fn a_note_past_its_time_in_the_bin_goes_and_the_server_hears() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let now = Utc::now();
        let old = binned_note(&repo, &key, now - chrono::Duration::days(31));
        let recent = binned_note(&repo, &key, now - chrono::Duration::days(29));

        let purged = repo.purge_expired_bin(now - chrono::Duration::days(30)).unwrap();

        assert_eq!(purged, 1);
        assert!(repo.get(&old.id, &key).unwrap().is_none());
        assert!(repo.get(&recent.id, &key).unwrap().is_some());
        let pending: Vec<String> = repo.get_pending_deletions().unwrap().into_iter().map(|(id, _)| id).collect();
        assert_eq!(pending, [old.id]);
    }

    /// If the note comes back before the next push, from a backup or an import,
    /// sending the deletion would delete it again, on every device.
    #[test]
    fn a_deletion_is_not_sent_for_a_note_that_came_back() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let note = Note::new("worth keeping".to_string());
        repo.create(&note, &key).unwrap();

        repo.hard_delete(&note.id).unwrap();
        assert_eq!(repo.get_pending_deletions().unwrap().len(), 1);

        repo.create(&note, &key).unwrap();

        assert!(repo.get_pending_deletions().unwrap().is_empty());
    }

    /// A compromised server could date a note decades ago. That must not empty the bin.
    #[test]
    fn a_date_from_a_server_cannot_empty_the_bin() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let now = Utc::now();
        let arrived = binned_note(&repo, &key, now - chrono::Duration::days(3650));
        repo.mark_arrived_in_bin(&arrived.id).unwrap();

        repo.purge_expired_bin(now - chrono::Duration::days(30)).unwrap();
        repo.purge_remote_deletions(now - chrono::Duration::days(30)).unwrap();

        assert!(repo.get(&arrived.id, &key).unwrap().is_some());
        assert!(repo.get_pending_deletions().unwrap().is_empty());
    }

    #[test]
    fn a_note_removed_for_good_takes_its_versions_and_only_its_own_files() {
        use crate::repository::{attachment::AttachmentRepository, NoteVersionRepository, VersionReason};
        use crate::models::Attachment;

        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let attachments = AttachmentRepository::new(db.connection());
        let reference = |id: &str| Attachment {
            id: id.to_string(),
            filename: "file.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 1,
            data: id.to_string(),
            thumbnail_data: None,
        };
        attachments.store("only-here", "file.txt", "text/plain", 1, b"a", &key).unwrap();
        attachments.store("shared", "file.txt", "text/plain", 1, b"b", &key).unwrap();

        let mut doomed = binned_note(&repo, &key, Utc::now() - chrono::Duration::days(31));
        doomed.attachments = vec![reference("only-here"), reference("shared")];
        repo.store(&doomed, &key).unwrap();
        NoteVersionRepository::new(db.connection()).create_version(&doomed, Utc::now(), VersionReason::Sync, &key).unwrap();

        let mut kept = Note::new("Keeps its file".to_string());
        kept.attachments = vec![reference("shared")];
        repo.create(&kept, &key).unwrap();

        repo.empty_trash().unwrap();

        let versions = NoteVersionRepository::new(db.connection()).get_versions_for_note(&doomed.id, &key).unwrap();
        assert!(versions.is_empty());
        assert_eq!(attachments.list_ids().unwrap(), ["shared"]);
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
    fn a_held_deletion_moves_a_note_to_the_bin_with_this_devices_time() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();

        repo.hold_deletion(&note.id).unwrap();
        assert_eq!(repo.count_held_deletions().unwrap(), 1);
        assert_eq!(repo.apply_held_deletions().unwrap(), 1);

        let binned = repo.get(&note.id, &key).unwrap().unwrap();
        assert!(binned.deleted);
        assert!(!binned.needs_sync);
        assert_eq!(repo.count_held_deletions().unwrap(), 0);
    }

    #[test]
    fn a_note_already_in_the_bin_is_not_counted_as_a_loss() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());

        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();
        repo.delete(&note.id).unwrap();
        repo.hold_deletion(&note.id).unwrap();

        assert_eq!(repo.count_held_deletions().unwrap(), 0);
        assert_eq!(repo.apply_held_deletions().unwrap(), 0);

        // It still leaves in time, because the server says it's gone
        assert_eq!(repo.purge_remote_deletions(Utc::now() + chrono::Duration::days(1)).unwrap(), 1);
    }

    #[test]
    fn a_purge_takes_the_versions_with_the_note() {
        let (db, key) = setup_test_db();
        let repo = NoteRepository::new(db.connection());
        let note = Note::new("Test".to_string());
        repo.create(&note, &key).unwrap();
        let versions = crate::repository::NoteVersionRepository::new(db.connection());
        versions.create_version(&note, Utc::now(), crate::repository::VersionReason::Sync, &key).unwrap();

        repo.hold_deletion(&note.id).unwrap();
        repo.apply_held_deletions().unwrap();
        repo.purge_remote_deletions(Utc::now() + chrono::Duration::days(1)).unwrap();

        assert_eq!(versions.count_versions_for_note(&note.id).unwrap(), 0);
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
