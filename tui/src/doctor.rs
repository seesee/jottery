//! `jottery doctor`: what's wrong with a vault, if anything.

use anyhow::Result;
use rusqlite::Connection;

use crate::crypto::{CryptoService, EncryptedData};
use crate::repository::{attachment::AttachmentRepository, sync::SyncRepository, NoteRepository, SavedSearchRepository};

/// What a check of the vault found.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HealthReport {
    pub notes: usize,
    pub unreadable_notes: usize,
    pub versions: usize,
    pub unreadable_versions: usize,
    pub attachments: usize,
    pub unreadable_attachments: usize,
    /// Attachments a note refers to with no file on this device
    pub missing_attachments: usize,
    pub saved_searches: usize,
    pub unreadable_saved_searches: usize,
    /// Waiting for the user, which isn't damage
    pub conflicts: usize,
}

impl HealthReport {
    /// How many things are wrong.
    pub fn problems(&self) -> usize {
        self.unreadable_notes
            + self.unreadable_versions
            + self.unreadable_attachments
            + self.missing_attachments
            + self.unreadable_saved_searches
    }
}

/// How many stored versions there are, and how many of them this key can't open.
fn check_versions(conn: &Connection, crypto: &CryptoService, key: &[u8; 32]) -> Result<(usize, usize)> {
    let mut stmt = conn.prepare("SELECT content FROM note_versions")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;

    let (mut total, mut unreadable) = (0, 0);
    for row in rows {
        total += 1;
        let opens = serde_json::from_str::<EncryptedData>(&row?)
            .is_ok_and(|encrypted| crypto.decrypt_text(&encrypted, key).is_ok());
        unreadable += !opens as usize;
    }

    Ok((total, unreadable))
}

/// Read everything in the vault once, and say what didn't open or isn't there.
pub fn check(conn: &Connection, key: &[u8; 32]) -> Result<HealthReport> {
    let crypto = CryptoService::new();

    let listed = NoteRepository::new(conn).list_everything(key)?;
    let (versions, unreadable_versions) = check_versions(conn, &crypto, key)?;

    let attachment_repo = AttachmentRepository::new(conn);
    let attachment_ids = attachment_repo.list_ids()?;
    let unreadable_attachments = attachment_ids.iter()
        .filter(|id| attachment_repo.get(id, key).is_err())
        .count();

    let saved_search_repo = SavedSearchRepository::new(conn);
    let stored_searches = saved_search_repo.list_stored()?;
    let live_searches = stored_searches.iter().filter(|search| !search.deleted).count();
    let readable_searches = saved_search_repo.list(key)?.len();

    Ok(HealthReport {
        notes: listed.notes.len() + listed.unreadable,
        unreadable_notes: listed.unreadable,
        versions,
        unreadable_versions,
        attachments: attachment_ids.len(),
        unreadable_attachments,
        missing_attachments: attachment_repo.list_missing_blobs()?.len(),
        saved_searches: live_searches,
        unreadable_saved_searches: live_searches - readable_searches,
        conflicts: SyncRepository::new(conn).get_conflict_note_ids()?.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::{Attachment, Note};
    use crate::repository::{NoteVersionRepository, VersionReason};
    use chrono::Utc;

    fn vault() -> (Database, [u8; 32]) {
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        (db, CryptoService::new().generate_master_key())
    }

    #[test]
    fn a_healthy_vault_has_no_problems() {
        let (db, key) = vault();
        let note = Note::new("Fine".to_string());
        NoteRepository::new(db.connection()).create(&note, &key).unwrap();
        NoteVersionRepository::new(db.connection()).create_version(&note, Utc::now(), VersionReason::Sync, &key).unwrap();
        SavedSearchRepository::new(db.connection()).create("Fine", "x", &key).unwrap();

        let report = check(db.connection(), &key).unwrap();

        assert_eq!(report.problems(), 0);
        assert_eq!((report.notes, report.versions, report.saved_searches), (1, 1, 1));
    }

    #[test]
    fn rows_that_will_not_open_are_counted() {
        let (db, key) = vault();
        let other_key = CryptoService::new().generate_master_key();
        let note = Note::new("Under another key".to_string());
        NoteRepository::new(db.connection()).create(&note, &other_key).unwrap();
        NoteVersionRepository::new(db.connection()).create_version(&note, Utc::now(), VersionReason::Sync, &other_key).unwrap();
        AttachmentRepository::new(db.connection()).store("file", "a.txt", "text/plain", 1, b"a", &other_key).unwrap();
        SavedSearchRepository::new(db.connection()).create("Hidden", "x", &other_key).unwrap();

        let report = check(db.connection(), &key).unwrap();

        assert_eq!(report.unreadable_notes, 1);
        assert_eq!(report.unreadable_versions, 1);
        assert_eq!(report.unreadable_attachments, 1);
        assert_eq!(report.unreadable_saved_searches, 1);
        assert_eq!(report.problems(), 4);
    }

    #[test]
    fn a_note_naming_a_file_that_is_not_here_is_a_problem() {
        let (db, key) = vault();
        let mut note = Note::new("Has a reference".to_string());
        note.attachments.push(Attachment {
            id: "missing".to_string(),
            filename: "a.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 1,
            data: "missing".to_string(),
            thumbnail_data: None,
        });
        NoteRepository::new(db.connection()).create(&note, &key).unwrap();

        let report = check(db.connection(), &key).unwrap();

        assert_eq!(report.missing_attachments, 1);
        assert_eq!(report.problems(), 1);
    }
}
