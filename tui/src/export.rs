//! JSON import/export functionality
//! Matches web app export format

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::{DateTime, Utc};
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

use crate::{
    db::Database,
    models::{validation::is_uuid, Attachment, Note},
    repository::{attachment::AttachmentRepository, NoteRepository},
};

/// Export format (matches web app)
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    #[serde(rename = "exportDate")]
    pub export_date: String,
    pub notes: Vec<ExportNote>,
}

/// An attachment in an export, with its contents
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportAttachment {
    pub filename: String,
    #[serde(rename = "mimeType", default)]
    pub mime_type: String,
    /// Base64 of the file
    pub data: String,
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
    #[serde(default)]
    pub attachments: Vec<ExportAttachment>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub locked: bool,
    /// Older exports and other clients' have no such field, and hold no deleted notes
    #[serde(default)]
    pub deleted: bool,
    #[serde(rename = "deletedAt", default, skip_serializing_if = "Option::is_none")]
    pub deleted_at: Option<String>,
    #[serde(rename = "wordWrap")]
    pub word_wrap: Option<bool>,
    #[serde(rename = "syntaxLanguage")]
    pub syntax_language: Option<String>,
    pub color: Option<String>,
}

/// A note in export form, attachments included. An attachment whose file isn't on this
/// device can't be included, and doesn't stop the export.
pub fn export_note(note: &Note, attachments: &AttachmentRepository<'_>, key: &[u8; 32]) -> ExportNote {
    let attachments = note.attachments.iter()
        .filter_map(|attachment| match attachments.get(&attachment.id, key) {
            Ok(Some((_, _, _, data))) => Some(ExportAttachment {
                filename: attachment.filename.clone(),
                mime_type: attachment.mime_type.clone(),
                data: BASE64.encode(data),
            }),
            Ok(None) => {
                tracing::warn!("Attachment {} has no file here, so it isn't in the export", attachment.id);
                None
            }
            Err(e) => {
                tracing::warn!("Attachment {} couldn't be read for export: {e}", attachment.id);
                None
            }
        })
        .collect();

    ExportNote {
        id: note.id.clone(),
        created_at: note.created_at.to_rfc3339(),
        modified_at: note.modified_at.to_rfc3339(),
        content: note.content.clone(),
        tags: note.tags.clone(),
        attachments,
        pinned: note.pinned,
        archived: note.archived,
        locked: note.locked,
        deleted: note.deleted,
        deleted_at: note.deleted_at.map(|at| at.to_rfc3339()),
        word_wrap: Some(note.word_wrap),
        syntax_language: Some(note.syntax_name()),
        color: note.color.clone(),
    }
}

/// An export would have replaced a file, and nobody has said it may.
#[derive(Debug, thiserror::Error)]
#[error("{}", t!("export.file_exists"))]
pub struct ExportFileExists;

/// Write a decrypted export, readable by this user alone.
///
/// The file only appears under its name once it's complete. A file already there stays,
/// unless `overwrite` says otherwise, and then it's replaced rather than rewritten, or it
/// would keep whatever permissions it had.
pub fn write_private(path: &Path, contents: &[u8], overwrite: bool) -> Result<()> {
    if !overwrite && path.exists() {
        return Err(ExportFileExists.into());
    }

    let folder = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };

    // A temporary file is private from the moment it exists
    let mut file = tempfile::NamedTempFile::new_in(folder).context("Failed to create export file")?;
    file.write_all(contents).context("Failed to write export file")?;

    let placed = if overwrite { file.persist(path) } else { file.persist_noclobber(path) };
    match placed {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Err(ExportFileExists.into()),
        Err(e) => Err(e.error).context("Failed to create export file"),
    }
}

/// Export notes to JSON file
pub fn export_notes<P: AsRef<Path>>(
    db: &Database,
    key: &[u8; 32],
    path: P,
    overwrite: bool,
) -> Result<usize> {
    let repo = NoteRepository::new(db.connection());
    let attachments = AttachmentRepository::new(db.connection());

    // Load all notes (including deleted for complete backup)
    let notes = repo.list(true, key)?;

    // Convert to export format
    let export_notes: Vec<ExportNote> = notes.iter()
        .map(|note| export_note(note, &attachments, key))
        .collect();

    let export_data = ExportData {
        version: "1.0".to_string(),
        export_date: Utc::now().to_rfc3339(),
        notes: export_notes,
    };

    // Write to file
    let json = serde_json::to_vec_pretty(&export_data).context("Failed to write JSON")?;
    write_private(path.as_ref(), &json, overwrite)?;

    Ok(notes.len())
}

/// What an import did with the notes in a file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportSummary {
    /// Notes this vault didn't have
    pub created: usize,
    /// Notes the file held a newer copy of
    pub updated: usize,
    /// Notes this vault already had, at least as new as the file's
    pub skipped: usize,
    /// Entries that couldn't be used, and attachments that couldn't be stored
    pub failed: usize,
}

/// Store an imported note's attachments under new ids, returning what was stored and
/// how many couldn't be. New ids because the file's aren't known to be unique here.
fn store_attachments(
    repo: &AttachmentRepository<'_>,
    attachments: &[ExportAttachment],
    key: &[u8; 32],
) -> (Vec<Attachment>, usize) {
    let mut stored = Vec::new();
    let mut failed = 0;

    for attachment in attachments {
        let id = uuid::Uuid::new_v4().to_string();
        let mime_type = if attachment.mime_type.is_empty() {
            "application/octet-stream"
        } else {
            attachment.mime_type.as_str()
        };

        let saved = BASE64.decode(&attachment.data).map_err(anyhow::Error::from).and_then(|data| {
            let size = data.len() as i64;
            repo.store(&id, &attachment.filename, mime_type, size, &data, key).map(|_| size)
        });

        match saved {
            Ok(size) => stored.push(Attachment {
                id: id.clone(),
                filename: attachment.filename.clone(),
                mime_type: mime_type.to_string(),
                size,
                data: id,
                thumbnail_data: None,
            }),
            Err(e) => {
                tracing::warn!("Skipping an attachment that couldn't be imported: {e}");
                failed += 1;
            }
        }
    }

    (stored, failed)
}

/// Bring a note this vault already has up to date with a newer copy from a file.
///
/// The file only speaks for what it holds. Attachments stay, because an export may have
/// none of them, and so does being in the bin, which a file saying nothing doesn't undo.
fn update_from_file(local: &mut Note, imported: ExportNote, modified_at: DateTime<Utc>) {
    local.modified_at = modified_at;
    local.content = imported.content;
    local.tags = imported.tags;
    local.pinned = imported.pinned;
    local.archived = imported.archived;
    local.locked = imported.locked;
    local.word_wrap = imported.word_wrap.unwrap_or(local.word_wrap);
    if let Some(name) = imported.syntax_language {
        local.set_syntax_name(&name);
    }
    local.color = imported.color;
    local.version += 1;
}

/// Add one note from a file to a vault that doesn't have it.
fn create_from_file(
    repo: &NoteRepository<'_>,
    attachments: &AttachmentRepository<'_>,
    imported: ExportNote,
    id: String,
    timestamps: (DateTime<Utc>, DateTime<Utc>),
    key: &[u8; 32],
) -> Result<usize> {
    let (created_at, modified_at) = timestamps;
    let (stored, failed) = store_attachments(attachments, &imported.attachments, key);

    let mut note = Note::new(imported.content);
    note.id = id;
    note.created_at = created_at;
    note.modified_at = modified_at;
    note.tags = imported.tags;
    note.attachments = stored;
    note.pinned = imported.pinned;
    note.archived = imported.archived;
    note.locked = imported.locked;
    note.deleted = imported.deleted;
    note.deleted_at = imported.deleted_at.and_then(|at| at.parse().ok());
    note.word_wrap = imported.word_wrap.unwrap_or(true);
    if let Some(name) = imported.syntax_language {
        note.set_syntax_name(&name);
    }
    note.color = imported.color;
    // The server may know this note, and the file can't speak for its whole list
    note.attachments_complete = false;
    // Previews can render, and a file can come from anywhere
    note.show_preview = Some(false);

    repo.create(&note, key)?;

    // A date in a file says nothing about how long the note has been in this bin
    if note.deleted {
        repo.mark_arrived_in_bin(&note.id)?;
    }
    Ok(failed)
}

/// Import notes from JSON file.
///
/// A note the vault already has is only replaced by a newer copy. It all goes in one
/// transaction, so a failure partway through leaves the vault as it was.
pub fn import_notes<P: AsRef<Path>>(
    db: &Database,
    key: &[u8; 32],
    path: P,
) -> Result<ImportSummary> {
    let file = File::open(path.as_ref())
        .context("Failed to open import file")?;
    let reader = BufReader::new(file);

    let export_data: ExportData = serde_json::from_reader(reader)
        .context("Failed to parse JSON")?;

    let transaction = db.connection().unchecked_transaction()?;
    let repo = NoteRepository::new(&transaction);
    let attachments = AttachmentRepository::new(&transaction);
    let mut summary = ImportSummary::default();

    for imported in export_data.notes {
        // The file may come from anywhere. An id that isn't one gets replaced.
        let id = if is_uuid(&imported.id) {
            imported.id.clone()
        } else {
            uuid::Uuid::new_v4().to_string()
        };

        let (Ok(created_at), Ok(modified_at)) = (
            imported.created_at.parse::<DateTime<Utc>>(),
            imported.modified_at.parse::<DateTime<Utc>>(),
        ) else {
            summary.failed += 1;
            continue;
        };

        match repo.get(&id, key) {
            Ok(Some(mut local)) => {
                if local.modified_at >= modified_at {
                    summary.skipped += 1;
                } else {
                    update_from_file(&mut local, imported, modified_at);
                    repo.update(&local, key)?;
                    summary.updated += 1;
                }
            }
            Ok(None) => {
                summary.failed += create_from_file(&repo, &attachments, imported, id, (created_at, modified_at), key)?;
                summary.created += 1;
            }
            // The row is there but unreadable, so replacing it could lose a note
            Err(e) => {
                tracing::warn!("Skipping note {id}, which this vault has but can't read: {e}");
                summary.failed += 1;
            }
        }
    }

    drop(attachments);
    drop(repo);
    transaction.commit()?;

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::CryptoService;
    use tempfile::NamedTempFile;

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

    #[test]
    fn test_export_import_roundtrip() {
        let password = test_password();
        let crypto = CryptoService::new();
        let salt = crypto.generate_salt();
        let key = crypto.derive_key(&password, &salt, 100_000).unwrap();

        // Create temporary database
        let db = Database::in_memory(&password).unwrap();
        let repo = NoteRepository::new(db.connection());

        // Create test notes
        let note1 = Note::new("Test note 1".to_string());
        let note2 = Note::new("Test note 2".to_string());

        repo.create(&note1, &key).unwrap();
        repo.create(&note2, &key).unwrap();

        // Export
        let export_file = NamedTempFile::new().unwrap();
        let count = export_notes(&db, &key, export_file.path(), true).unwrap();
        assert_eq!(count, 2);

        // Create new database and import
        let db2 = Database::in_memory(&password).unwrap();
        let imported = import_notes(&db2, &key, export_file.path()).unwrap();
        assert_eq!(imported.created, 2);

        // Verify
        let repo2 = NoteRepository::new(db2.connection());
        let notes = repo2.list(false, &key).unwrap();
        assert_eq!(notes.len(), 2);
    }

    /// A vault with one note, its key, and a file to import from.
    fn vault_with(note: &Note) -> (Database, [u8; 32]) {
        let key = CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        NoteRepository::new(db.connection()).create(note, &key).unwrap();
        (db, key)
    }

    fn file_holding(notes: serde_json::Value) -> NamedTempFile {
        let file = NamedTempFile::new().unwrap();
        let data = serde_json::json!({ "version": "1.0", "exportDate": "2026-01-01T00:00:00Z", "notes": notes });
        std::fs::write(file.path(), data.to_string()).unwrap();
        file
    }

    fn exported(note: &Note, content: &str, modified_at: DateTime<Utc>) -> serde_json::Value {
        serde_json::json!({
            "id": note.id,
            "createdAt": note.created_at.to_rfc3339(),
            "modifiedAt": modified_at.to_rfc3339(),
            "content": content,
            "tags": [],
            "pinned": false,
        })
    }

    #[test]
    fn an_older_copy_in_a_file_does_not_replace_a_newer_note() {
        let note = Note::new("Written today".to_string());
        let (db, key) = vault_with(&note);
        let file = file_holding(serde_json::json!([exported(&note, "Last year's text", note.modified_at - chrono::Duration::days(365))]));

        let summary = import_notes(&db, &key, file.path()).unwrap();

        assert_eq!(summary, ImportSummary { skipped: 1, ..Default::default() });
        let kept = NoteRepository::new(db.connection()).get(&note.id, &key).unwrap().unwrap();
        assert_eq!(kept.content, "Written today");
    }

    #[test]
    fn a_newer_copy_in_a_file_keeps_the_notes_attachments() {
        let mut note = Note::new("Has a file".to_string());
        note.attachments.push(Attachment {
            id: uuid::Uuid::new_v4().to_string(),
            filename: "keep.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 3,
            data: String::new(),
            thumbnail_data: None,
        });
        let (db, key) = vault_with(&note);
        let file = file_holding(serde_json::json!([exported(&note, "Edited", note.modified_at + chrono::Duration::days(1))]));

        let summary = import_notes(&db, &key, file.path()).unwrap();

        assert_eq!(summary, ImportSummary { updated: 1, ..Default::default() });
        let updated = NoteRepository::new(db.connection()).get(&note.id, &key).unwrap().unwrap();
        assert_eq!(updated.content, "Edited");
        assert_eq!(updated.attachments.len(), 1);
    }

    #[test]
    fn a_newer_copy_in_a_file_leaves_a_note_in_the_bin() {
        let mut note = Note::new("Binned".to_string());
        note.deleted = true;
        note.deleted_at = Some(Utc::now());
        let (db, key) = vault_with(&note);
        let file = file_holding(serde_json::json!([exported(&note, "Edited", note.modified_at + chrono::Duration::days(1))]));

        import_notes(&db, &key, file.path()).unwrap();

        let updated = NoteRepository::new(db.connection()).get(&note.id, &key).unwrap().unwrap();
        assert!(updated.deleted);
        assert!(updated.deleted_at.is_some());
    }

    #[test]
    fn a_new_note_brings_its_attachments() {
        let note = Note::new("Elsewhere".to_string());
        let key = CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let mut entry = exported(&note, "From a file", note.modified_at);
        entry["attachments"] = serde_json::json!([
            { "filename": "hello.txt", "mimeType": "text/plain", "data": BASE64.encode("hello") },
            { "filename": "broken.txt", "mimeType": "text/plain", "data": "!!! not base64 !!!" },
        ]);
        let file = file_holding(serde_json::json!([entry]));

        let summary = import_notes(&db, &key, file.path()).unwrap();

        assert_eq!(summary, ImportSummary { created: 1, failed: 1, ..Default::default() });
        let stored = NoteRepository::new(db.connection()).get(&note.id, &key).unwrap().unwrap();
        assert_eq!(stored.attachments.len(), 1);
        let (_, _, _, data) = AttachmentRepository::new(db.connection())
            .get(&stored.attachments[0].id, &key).unwrap().unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn exporting_and_importing_keeps_attachments_and_the_bin() {
        let key = CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let attachment_id = uuid::Uuid::new_v4().to_string();
        let mut binned = Note::new("Binned".to_string());
        binned.deleted = true;
        binned.deleted_at = Some(Utc::now());
        binned.attachments.push(Attachment {
            id: attachment_id.clone(),
            filename: "keep.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 3,
            data: attachment_id.clone(),
            thumbnail_data: None,
        });
        NoteRepository::new(db.connection()).create(&binned, &key).unwrap();
        AttachmentRepository::new(db.connection())
            .store(&attachment_id, "keep.txt", "text/plain", 3, b"abc", &key)
            .unwrap();

        let file = NamedTempFile::new().unwrap();
        export_notes(&db, &key, file.path(), true).unwrap();

        let other = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        import_notes(&other, &key, file.path()).unwrap();

        let imported = NoteRepository::new(other.connection()).get(&binned.id, &key).unwrap().unwrap();
        assert!(imported.deleted);
        assert_eq!(imported.attachments[0].filename, "keep.txt");
    }

    #[test]
    fn an_entry_that_cannot_be_used_costs_that_entry() {
        let note = Note::new("Good".to_string());
        let mut bad_date = exported(&Note::new("Bad".to_string()), "Bad", note.modified_at);
        bad_date["modifiedAt"] = serde_json::json!("not a date");
        let mut odd_id = exported(&Note::new("Odd".to_string()), "Odd id", note.modified_at);
        odd_id["id"] = serde_json::json!("../../note");
        let key = CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let file = file_holding(serde_json::json!([bad_date, exported(&note, "Good", note.modified_at), odd_id]));

        let summary = import_notes(&db, &key, file.path()).unwrap();

        assert_eq!(summary, ImportSummary { created: 2, failed: 1, ..Default::default() });
        let stored = NoteRepository::new(db.connection()).list(true, &key).unwrap();
        assert!(stored.iter().all(|note| is_uuid(&note.id)));
    }

    /// One transaction: nothing stays behind if the import can't finish.
    #[test]
    fn a_failed_import_changes_nothing() {
        let note = Note::new("Good".to_string());
        let key = CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let second = Note::new("Second".to_string());
        let file = file_holding(serde_json::json!([
            exported(&note, "Good", note.modified_at),
            exported(&second, "Second", note.modified_at),
        ]));

        // Make the second write impossible, after the first has gone in
        db.connection().execute_batch(
            "CREATE TRIGGER refuse_second AFTER INSERT ON notes
             WHEN (SELECT COUNT(*) FROM notes) > 1
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        ).unwrap();

        assert!(import_notes(&db, &key, file.path()).is_err());
        assert_eq!(NoteRepository::new(db.connection()).count(true).unwrap(), 0);
    }

    #[test]
    #[cfg(unix)]
    fn an_export_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");

        write_private(&path, b"decrypted", false).unwrap();

        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn an_export_leaves_a_file_alone_unless_told_otherwise() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        std::fs::write(&path, "already here").unwrap();

        let error = write_private(&path, b"decrypted", false).unwrap_err();

        assert!(error.is::<ExportFileExists>());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "already here");
    }

    /// Writing over it would keep whatever permissions the old file had.
    #[test]
    #[cfg(unix)]
    fn an_export_that_overwrites_takes_the_loose_permissions_away() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        std::fs::write(&path, "already here").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_private(&path, b"decrypted", true).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"decrypted");
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
