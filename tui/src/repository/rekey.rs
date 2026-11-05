//! Moving a vault's contents from one master key to another.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::crypto::{CryptoService, EncryptedData};

/// Re-encrypt `content` and `tags` in every row of a table with `old_key` to
/// `new_key`, returning how many rows changed. Tags stay in their stored form.
fn rekey_text_columns(
    conn: &Connection,
    crypto: &CryptoService,
    table: &str,
    id_column: &str,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<usize> {
    let mut read = conn.prepare(&format!("SELECT {id_column}, content, tags FROM {table}"))?;
    let rows: Vec<(String, String, String)> = read
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut write = conn.prepare(&format!("UPDATE {table} SET content = ?1, tags = ?2 WHERE {id_column} = ?3"))?;
    for (id, content, tags) in &rows {
        let rekeyed = |stored: &str| -> Result<String> {
            let encrypted: EncryptedData = serde_json::from_str(stored)?;
            let plain = crypto.decrypt_text(&encrypted, old_key)?;
            Ok(serde_json::to_string(&crypto.encrypt_text(&plain, new_key)?)?)
        };
        let content = rekeyed(content).with_context(|| format!("{table} row {id}"))?;
        let tags = rekeyed(tags).with_context(|| format!("{table} row {id}"))?;
        write.execute(params![content, tags, id])?;
    }

    Ok(rows.len())
}

/// Re-encrypt every attachment's filename and data.
fn rekey_attachments(
    conn: &Connection,
    crypto: &CryptoService,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<usize> {
    let mut read = conn.prepare("SELECT id, filename, data FROM attachments")?;
    let rows: Vec<(String, String, String)> = read
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut write = conn.prepare("UPDATE attachments SET filename = ?1, data = ?2 WHERE id = ?3")?;
    for (id, filename, data) in &rows {
        let context = || format!("attachment {id}");

        let encrypted: EncryptedData = serde_json::from_str(filename).with_context(context)?;
        let filename = crypto.decrypt_text(&encrypted, old_key).with_context(context)?;
        let filename = serde_json::to_string(&crypto.encrypt_text(&filename, new_key)?)?;

        let encrypted: EncryptedData = serde_json::from_str(data).with_context(context)?;
        let data = crypto.decrypt_binary(&encrypted, old_key).with_context(context)?;
        let data = serde_json::to_string(&crypto.encrypt_binary(&data, new_key)?)?;

        write.execute(params![filename, data, id])?;
    }

    Ok(rows.len())
}

/// Re-encrypt every saved search. A deleted row isn't shown, and can hold plaintext from
/// an older client, so one that won't open stays as it is.
fn rekey_saved_searches(
    conn: &Connection,
    crypto: &CryptoService,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<usize> {
    let mut read = conn.prepare("SELECT id, name, query, deleted FROM saved_searches")?;
    let rows: Vec<(String, String, String, bool)> = read
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get::<_, i32>(3)? != 0)))?
        .collect::<rusqlite::Result<_>>()?;

    let rekeyed = |stored: &str| -> Result<String> {
        let encrypted: EncryptedData = serde_json::from_str(stored)?;
        let plain = crypto.decrypt_text(&encrypted, old_key)?;
        Ok(serde_json::to_string(&crypto.encrypt_text(&plain, new_key)?)?)
    };

    let mut write = conn.prepare("UPDATE saved_searches SET name = ?1, query = ?2 WHERE id = ?3")?;
    let mut changed = 0;
    for (id, name, query, deleted) in &rows {
        match (rekeyed(name), rekeyed(query)) {
            (Ok(name), Ok(query)) => {
                write.execute(params![name, query, id])?;
                changed += 1;
            }
            _ if *deleted => {}
            (Err(e), _) | (_, Err(e)) => return Err(e).with_context(|| format!("saved search {id}")),
        }
    }

    Ok(changed)
}

/// Re-encrypt notes, versions, attachments and saved searches from `old_key` to `new_key`.
///
/// Stops at the first row that won't decrypt. Run this in a transaction so that
/// leaves everything as it was, because a half-moved vault loses the other half.
pub fn rekey_vault(
    conn: &Connection,
    crypto: &CryptoService,
    old_key: &[u8; 32],
    new_key: &[u8; 32],
) -> Result<usize> {
    let notes = rekey_text_columns(conn, crypto, "notes", "id", old_key, new_key)?;
    let versions = rekey_text_columns(conn, crypto, "note_versions", "version_key", old_key, new_key)?;
    let attachments = rekey_attachments(conn, crypto, old_key, new_key)?;
    let saved_searches = rekey_saved_searches(conn, crypto, old_key, new_key)?;
    Ok(notes + versions + attachments + saved_searches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::Note;
    use crate::repository::{attachment::AttachmentRepository, NoteRepository, NoteVersionRepository, VersionReason};
    use chrono::Utc;

    struct Vault {
        db: Database,
        old_key: [u8; 32],
        new_key: [u8; 32],
        note: Note,
    }

    fn vault() -> Vault {
        let crypto = CryptoService::new();
        let (old_key, new_key) = (crypto.generate_master_key(), crypto.generate_master_key());
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();

        let mut note = Note::new("Local note".to_string());
        note.tags = vec!["tagged".to_string()];
        NoteRepository::new(db.connection()).create(&note, &old_key).unwrap();
        NoteVersionRepository::new(db.connection())
            .create_version(&note, Utc::now(), VersionReason::Sync, &old_key)
            .unwrap();
        AttachmentRepository::new(db.connection())
            .store("attachment-id", "photo.png", "image/png", 3, b"png", &old_key)
            .unwrap();

        Vault { db, old_key, new_key, note }
    }

    #[test]
    fn everything_ends_up_under_the_new_key() {
        let vault = vault();
        let conn = vault.db.connection();

        rekey_vault(conn, &CryptoService::new(), &vault.old_key, &vault.new_key).unwrap();

        let note = NoteRepository::new(conn).get(&vault.note.id, &vault.new_key).unwrap().unwrap();
        assert_eq!(note.content, "Local note");
        assert_eq!(note.tags, vec!["tagged"]);

        let versions = NoteVersionRepository::new(conn).get_versions_for_note(&vault.note.id, &vault.new_key).unwrap();
        assert_eq!(versions[0].content, "Local note");

        let (filename, _, _, data) = AttachmentRepository::new(conn).get("attachment-id", &vault.new_key).unwrap().unwrap();
        assert_eq!(filename, "photo.png");
        assert_eq!(data, b"png");
    }

    #[test]
    fn a_row_under_another_key_stops_the_move_and_a_transaction_undoes_it() {
        let vault = vault();
        let conn = vault.db.connection();
        let stranger = CryptoService::new().generate_master_key();
        NoteRepository::new(conn).create(&Note::new("Not ours".to_string()), &stranger).unwrap();

        let transaction = conn.unchecked_transaction().unwrap();
        assert!(rekey_vault(&transaction, &CryptoService::new(), &vault.old_key, &vault.new_key).is_err());
        drop(transaction);

        let note = NoteRepository::new(conn).get(&vault.note.id, &vault.old_key).unwrap().unwrap();
        assert_eq!(note.content, "Local note");
    }

    #[test]
    fn saved_searches_move_with_the_vault() {
        use crate::repository::SavedSearchRepository;

        let vault = vault();
        let crypto = CryptoService::new();
        let repo = SavedSearchRepository::new(vault.db.connection());
        repo.create("Work", "#work", &vault.old_key).unwrap();

        rekey_vault(vault.db.connection(), &crypto, &vault.old_key, &vault.new_key).unwrap();

        let listed = repo.list(&vault.new_key).unwrap();
        assert_eq!(listed[0].query, "#work");
        assert!(repo.list(&vault.old_key).unwrap().is_empty());
    }

    /// Older clients pushed deleted rows as plaintext, and nothing reads those.
    #[test]
    fn a_deleted_saved_search_that_will_not_open_does_not_stop_the_move() {
        use crate::repository::SavedSearchRepository;

        let vault = vault();
        let crypto = CryptoService::new();
        vault.db.connection().execute(
            "INSERT INTO saved_searches (id, name, query, created_at, modified_at, deleted)
             VALUES ('gone', 'plaintext', 'plaintext', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
            [],
        ).unwrap();
        SavedSearchRepository::new(vault.db.connection()).create("Live", "x", &vault.old_key).unwrap();

        rekey_vault(vault.db.connection(), &crypto, &vault.old_key, &vault.new_key).unwrap();
    }
}
