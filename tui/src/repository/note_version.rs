#![allow(dead_code)]
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use tracing::debug;

use crate::crypto::{CryptoService, EncryptedData};
use crate::models::{Attachment, Note, SyntaxLanguage};

/// Note version snapshot for version history
#[derive(Debug, Clone)]
pub struct NoteVersion {
    pub version_key: String,      // `${note_id}:${version}`
    pub note_id: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub synced_at: DateTime<Utc>,
    pub content: String,           // Decrypted content
    pub tags: Vec<String>,         // Decrypted tags
    pub attachments: Vec<Attachment>,
    pub syntax_language: Option<SyntaxLanguage>,
    pub word_wrap: Option<bool>,
    pub color: Option<String>,  // Semantic color name
    pub reason: VersionReason,
}

/// Reason why a version was created
#[derive(Debug, Clone, Copy)]
pub enum VersionReason {
    Sync,
    ManualSync,
}

impl std::fmt::Display for VersionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sync => write!(f, "sync"),
            Self::ManualSync => write!(f, "manual-sync"),
        }
    }
}

impl std::str::FromStr for VersionReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sync" => Ok(Self::Sync),
            "manual-sync" => Ok(Self::ManualSync),
            _ => Err(format!("Unknown version reason: {}", s)),
        }
    }
}

/// Repository for note version operations
pub struct NoteVersionRepository<'a> {
    conn: &'a Connection,
    crypto: CryptoService,
}

impl<'a> NoteVersionRepository<'a> {
    /// Create a new note version repository
    pub fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            crypto: CryptoService::new(),
        }
    }

    /// Create a new version snapshot of a note
    /// Includes deduplication - skips if content unchanged from latest version
    pub fn create_version(
        &self,
        note: &Note,
        synced_at: DateTime<Utc>,
        reason: VersionReason,
        key: &[u8; 32],
    ) -> Result<Option<String>> {
        // Deduplication check
        if let Some(latest) = self.get_latest_version(&note.id, key)? {
            if latest.content == note.content {
                debug!("Skipping duplicate version for note {}", note.id);
                return Ok(None);
            }
        }

        let version_key = format!("{}:{}", note.id, note.version);

        // Encrypt content and tags
        let encrypted_content = self.crypto.encrypt_text(&note.content, key)?;
        let encrypted_tags = self.crypto.encrypt_json(&note.tags, key)?;

        // Serialize attachments
        let attachments_json = serde_json::to_string(&note.attachments)
            .context("Failed to serialize attachments")?;

        self.conn.execute(
            "INSERT INTO note_versions (
                version_key, note_id, version, created_at, synced_at, content, tags,
                attachments, syntax_language, word_wrap, color, reason
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &version_key,
                &note.id,
                note.version,
                Utc::now().to_rfc3339(),
                synced_at.to_rfc3339(),
                serde_json::to_string(&encrypted_content)?,
                serde_json::to_string(&encrypted_tags)?,
                attachments_json,
                note.syntax_language.to_string(),
                note.word_wrap as i32,
                note.color.as_ref(),
                reason.to_string(),
            ],
        )?;

        debug!("Created version {} for note {}", note.version, note.id);
        Ok(Some(version_key))
    }

    /// Get all versions for a note, sorted by version number (newest first)
    pub fn get_versions_for_note(&self, note_id: &str, key: &[u8; 32]) -> Result<Vec<NoteVersion>> {
        let mut stmt = self.conn.prepare(
            "SELECT version_key, note_id, version, created_at, synced_at, content, tags,
                    attachments, syntax_language, word_wrap, color, reason
             FROM note_versions
             WHERE note_id = ?1
             ORDER BY version DESC"
        )?;

        let versions = stmt.query_map(params![note_id], |row| {
            Ok((
                row.get::<_, String>(0)?,   // version_key
                row.get::<_, String>(1)?,   // note_id
                row.get::<_, i32>(2)?,      // version
                row.get::<_, String>(3)?,   // created_at
                row.get::<_, String>(4)?,   // synced_at
                row.get::<_, String>(5)?,   // content (encrypted)
                row.get::<_, String>(6)?,   // tags (encrypted)
                row.get::<_, String>(7)?,   // attachments
                row.get::<_, String>(8)?,   // syntax_language
                row.get::<_, i32>(9)?,      // word_wrap
                row.get::<_, Option<String>>(10)?, // color
                row.get::<_, String>(11)?,  // reason
            ))
        })?;

        let mut result = Vec::new();
        for version_data in versions {
            let (
                version_key,
                note_id,
                version,
                created_at,
                synced_at,
                content_json,
                tags_json,
                attachments_json,
                syntax_language,
                word_wrap,
                color,
                reason,
            ) = version_data?;

            // Decrypt content and tags
            let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
            let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

            let content = self.crypto.decrypt_text(&encrypted_content, key)?;
            let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;

            // Deserialize attachments
            let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

            result.push(NoteVersion {
                version_key,
                note_id,
                version,
                created_at: created_at.parse()?,
                synced_at: synced_at.parse()?,
                content,
                tags,
                attachments,
                syntax_language: Some(syntax_language.parse().unwrap_or_default()),
                word_wrap: Some(word_wrap != 0),
                color,
                reason: reason.parse().unwrap_or(VersionReason::Sync),
            });
        }

        Ok(result)
    }

    /// Get a specific version of a note
    pub fn get_version(&self, note_id: &str, version: i32, key: &[u8; 32]) -> Result<Option<NoteVersion>> {
        let version_key = format!("{}:{}", note_id, version);

        let mut stmt = self.conn.prepare(
            "SELECT version_key, note_id, version, created_at, synced_at, content, tags,
                    attachments, syntax_language, word_wrap, color, reason
             FROM note_versions
             WHERE version_key = ?1"
        )?;

        let result = stmt.query_row(params![&version_key], |row| {
            Ok((
                row.get::<_, String>(0)?,   // version_key
                row.get::<_, String>(1)?,   // note_id
                row.get::<_, i32>(2)?,      // version
                row.get::<_, String>(3)?,   // created_at
                row.get::<_, String>(4)?,   // synced_at
                row.get::<_, String>(5)?,   // content (encrypted)
                row.get::<_, String>(6)?,   // tags (encrypted)
                row.get::<_, String>(7)?,   // attachments
                row.get::<_, String>(8)?,   // syntax_language
                row.get::<_, i32>(9)?,      // word_wrap
                row.get::<_, Option<String>>(10)?, // color
                row.get::<_, String>(11)?,  // reason
            ))
        });

        let result = match result {
            Ok(data) => Some(data),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };

        match result {
            Some((
                version_key,
                note_id,
                version,
                created_at,
                synced_at,
                content_json,
                tags_json,
                attachments_json,
                syntax_language,
                word_wrap,
                color,
                reason,
            )) => {
                // Decrypt content and tags
                let encrypted_content: EncryptedData = serde_json::from_str(&content_json)?;
                let encrypted_tags: EncryptedData = serde_json::from_str(&tags_json)?;

                let content = self.crypto.decrypt_text(&encrypted_content, key)?;
                let tags: Vec<String> = self.crypto.decrypt_json(&encrypted_tags, key)?;

                // Deserialize attachments
                let attachments: Vec<Attachment> = serde_json::from_str(&attachments_json)?;

                Ok(Some(NoteVersion {
                    version_key,
                    note_id,
                    version,
                    created_at: created_at.parse()?,
                    synced_at: synced_at.parse()?,
                    content,
                    tags,
                    attachments,
                    syntax_language: Some(syntax_language.parse().unwrap_or_default()),
                    word_wrap: Some(word_wrap != 0),
                    color,
                    reason: reason.parse().unwrap_or(VersionReason::Sync),
                }))
            }
            None => Ok(None),
        }
    }

    /// Get the latest version for a note
    pub fn get_latest_version(&self, note_id: &str, key: &[u8; 32]) -> Result<Option<NoteVersion>> {
        let versions = self.get_versions_for_note(note_id, key)?;
        Ok(versions.into_iter().next())
    }

    /// Delete all versions for a note (cascade delete when note is permanently deleted)
    pub fn delete_versions_for_note(&self, note_id: &str) -> Result<usize> {
        let deleted = self.conn.execute(
            "DELETE FROM note_versions WHERE note_id = ?1",
            params![note_id],
        )?;

        debug!("Deleted {} versions for note {}", deleted, note_id);
        Ok(deleted)
    }

    /// Count total versions for a note
    pub fn count_versions_for_note(&self, note_id: &str) -> Result<i32> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM note_versions WHERE note_id = ?1",
            params![note_id],
            |row| row.get(0),
        )?;

        Ok(count)
    }

    /// Check if a version exists by version_key
    pub fn get_version_by_key(&self, version_key: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT version_key FROM note_versions WHERE version_key = ?1",
            params![version_key],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(key) => Ok(Some(key)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Insert a version from sync server (already decrypted)
    /// Used during pull to store versions received from server
    pub fn insert_version_from_sync(
        &self,
        version: &NoteVersion,
        key: &[u8; 32],
    ) -> Result<()> {
        // Encrypt content and tags for storage
        let encrypted_content = self.crypto.encrypt_text(&version.content, key)?;
        let encrypted_tags = self.crypto.encrypt_json(&version.tags, key)?;

        // Serialize attachments
        let attachments_json = serde_json::to_string(&version.attachments)
            .context("Failed to serialize attachments")?;

        // Convert fields for database
        let syntax_language_str = version.syntax_language
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "plain".to_string());

        let word_wrap_int = version.word_wrap.unwrap_or(true) as i32;

        self.conn.execute(
            "INSERT INTO note_versions (
                version_key, note_id, version, created_at, synced_at, content, tags,
                attachments, syntax_language, word_wrap, reason
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(version_key) DO NOTHING",
            params![
                &version.version_key,
                &version.note_id,
                version.version,
                version.created_at.to_rfc3339(),
                version.synced_at.to_rfc3339(),
                serde_json::to_string(&encrypted_content)?,
                serde_json::to_string(&encrypted_tags)?,
                attachments_json,
                syntax_language_str,
                word_wrap_int,
                version.reason.to_string(),
            ],
        )?;

        debug!("Inserted version from sync: {}", version.version_key);
        Ok(())
    }
}
