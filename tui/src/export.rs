/// JSON import/export functionality
/// Matches web app export format

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use crate::{
    crypto::CryptoService,
    db::Database,
    models::Note,
    repository::NoteRepository,
};

/// Export format (matches web app)
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    #[serde(rename = "exportDate")]
    pub export_date: String,
    pub notes: Vec<ExportNote>,
}

/// Exported note (decrypted)
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportNote {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "modifiedAt")]
    pub modified_at: String,
    pub content: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    #[serde(rename = "wordWrap")]
    pub word_wrap: Option<bool>,
    #[serde(rename = "syntaxLanguage")]
    pub syntax_language: Option<String>,
}

impl From<&Note> for ExportNote {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id.clone(),
            created_at: note.created_at.to_rfc3339(),
            modified_at: note.modified_at.to_rfc3339(),
            content: note.content.clone(),
            tags: note.tags.clone(),
            pinned: note.pinned,
            word_wrap: Some(note.word_wrap),
            syntax_language: Some(note.syntax_language.to_string()),
        }
    }
}

/// Export notes to JSON file
pub fn export_notes<P: AsRef<Path>>(
    db: &Database,
    key: &[u8; 32],
    path: P,
) -> Result<usize> {
    let repo = NoteRepository::new(db.connection());

    // Load all notes (including deleted for complete backup)
    let notes = repo.list(true, key)?;

    // Convert to export format
    let export_notes: Vec<ExportNote> = notes.iter().map(ExportNote::from).collect();

    let export_data = ExportData {
        version: "1.0".to_string(),
        export_date: Utc::now().to_rfc3339(),
        notes: export_notes,
    };

    // Write to file
    let file = File::create(path.as_ref())
        .context("Failed to create export file")?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &export_data)
        .context("Failed to write JSON")?;

    Ok(notes.len())
}

/// Import notes from JSON file
pub fn import_notes<P: AsRef<Path>>(
    db: &Database,
    key: &[u8; 32],
    path: P,
) -> Result<usize> {
    let file = File::open(path.as_ref())
        .context("Failed to open import file")?;
    let reader = BufReader::new(file);

    let export_data: ExportData = serde_json::from_reader(reader)
        .context("Failed to parse JSON")?;

    let repo = NoteRepository::new(db.connection());
    let mut imported = 0;

    for export_note in export_data.notes {
        // Convert to Note
        let note = Note {
            id: export_note.id,
            created_at: export_note.created_at.parse()?,
            modified_at: export_note.modified_at.parse()?,
            synced_at: None,
            content: export_note.content,
            tags: export_note.tags,
            attachments: Vec::new(), // Attachments not supported in basic export
            pinned: export_note.pinned,
            deleted: false,
            deleted_at: None,
            sync_hash: None,
            version: 1,
            word_wrap: export_note.word_wrap.unwrap_or(true),
            syntax_language: export_note
                .syntax_language
                .and_then(|s| s.parse().ok())
                .unwrap_or_default(),
        };

        // Try to create (will fail if already exists, which is fine)
        match repo.create(&note, key) {
            Ok(_) => imported += 1,
            Err(_) => {
                // Note already exists, try to update
                if repo.update(&note, key).is_ok() {
                    imported += 1;
                }
            }
        }
    }

    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::CryptoService;
    use tempfile::NamedTempFile;

    #[test]
    fn test_export_import_roundtrip() {
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key("test_password", &salt, 100_000).unwrap();

        // Create temporary database
        let db = Database::in_memory("test_password").unwrap();
        let repo = NoteRepository::new(db.connection());

        // Create test notes
        let note1 = Note::new("Test note 1".to_string());
        let note2 = Note::new("Test note 2".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();

        // Export
        let export_file = NamedTempFile::new().unwrap();
        let count = export_notes(&db, &key, export_file.path()).unwrap();
        assert_eq!(count, 2);

        // Create new database and import
        let db2 = Database::in_memory("test_password").unwrap();
        let imported = import_notes(&db2, &key, export_file.path()).unwrap();
        assert_eq!(imported, 2);

        // Verify
        let repo2 = NoteRepository::new(db2.connection());
        let notes = repo2.list(false, &key).unwrap();
        assert_eq!(notes.len(), 2);
    }
}
