//! Saved searches: a name and a query, encrypted, synced as last-write-wins.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::crypto::{CryptoService, EncryptedData};
use crate::models::sync::SyncSavedSearch;

/// A saved search that's been decrypted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub query: String,
    pub order: i64,
}

fn stored_time(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    row.get::<_, String>(index)?.parse().map_err(|e: chrono::ParseError| {
        rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn optional_time(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    Ok(row.get::<_, Option<String>>(index)?.and_then(|text| text.parse().ok()))
}

/// A row as stored, which is the form that goes over the wire.
fn stored_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SyncSavedSearch> {
    Ok(SyncSavedSearch {
        id: row.get(0)?,
        name: row.get(1)?,
        query: row.get(2)?,
        order: row.get(3)?,
        created_at: stored_time(row, 4)?,
        modified_at: stored_time(row, 5)?,
        deleted: row.get::<_, i32>(6)? != 0,
        deleted_at: optional_time(row, 7)?,
        version: row.get(8)?,
    })
}

const COLUMNS: &str = "id, name, query, sort_order, created_at, modified_at, deleted, deleted_at, version";

pub struct SavedSearchRepository<'a> {
    conn: &'a Connection,
    crypto: CryptoService,
}

impl<'a> SavedSearchRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn, crypto: CryptoService::new() }
    }

    fn encrypted(&self, text: &str, key: &[u8; 32]) -> Result<String> {
        Ok(serde_json::to_string(&self.crypto.encrypt_text(text, key)?)?)
    }

    fn decrypted(&self, stored: &str, key: &[u8; 32]) -> Result<String> {
        let encrypted: EncryptedData = serde_json::from_str(stored)?;
        self.crypto.decrypt_text(&encrypted, key)
    }

    /// Whether this vault can read a row: one that doesn't open came from something
    /// without the key.
    pub fn is_readable(&self, row: &SyncSavedSearch, key: &[u8; 32]) -> bool {
        self.decrypted(&row.name, key).is_ok() && self.decrypted(&row.query, key).is_ok()
    }

    /// Every live search, in display order. A row that won't open is left out.
    pub fn list(&self, key: &[u8; 32]) -> Result<Vec<SavedSearch>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {COLUMNS} FROM saved_searches WHERE deleted = 0 ORDER BY sort_order, created_at"
        ))?;
        let rows = stmt.query_map([], stored_row)?.collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(rows.into_iter()
            .filter_map(|row| {
                Some(SavedSearch {
                    name: self.decrypted(&row.name, key).ok()?,
                    query: self.decrypted(&row.query, key).ok()?,
                    id: row.id,
                    order: row.order,
                })
            })
            .collect())
    }

    /// Save a search at the end of the list. It'll go to the server on the next sync.
    pub fn create(&self, name: &str, query: &str, key: &[u8; 32]) -> Result<SavedSearch> {
        let next_order: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM saved_searches WHERE deleted = 0",
            [],
            |row| row.get(0),
        )?;

        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO saved_searches (id, name, query, sort_order, created_at, modified_at, needs_sync)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, 1)",
            params![&id, self.encrypted(name, key)?, self.encrypted(query, key)?, next_order, &now],
        )?;

        Ok(SavedSearch { id, name: name.to_string(), query: query.to_string(), order: next_order })
    }

    /// Delete a search on every device. The row stays, to say so.
    pub fn delete(&self, id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE saved_searches
             SET deleted = 1, deleted_at = ?1, modified_at = ?1, version = version + 1, needs_sync = 1
             WHERE id = ?2",
            params![&now, id],
        )?;
        Ok(())
    }

    /// Rows the server hasn't got, as they go to it.
    pub fn pending(&self) -> Result<Vec<SyncSavedSearch>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {COLUMNS} FROM saved_searches WHERE needs_sync = 1"))?;
        let rows = stmt.query_map([], stored_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The server has this row. A change made since it was read stays pending.
    pub fn mark_pushed(&self, pushed: &SyncSavedSearch, synced_at: DateTime<Utc>) -> Result<()> {
        self.conn.execute(
            "UPDATE saved_searches SET needs_sync = 0, synced_at = ?1
             WHERE id = ?2 AND modified_at = ?3 AND version = ?4",
            params![synced_at.to_rfc3339(), &pushed.id, pushed.modified_at.to_rfc3339(), pushed.version],
        )?;
        Ok(())
    }

    /// Put every row up for pushing, as the first sync with an account does.
    pub fn mark_all_pending(&self) -> Result<()> {
        self.conn.execute("UPDATE saved_searches SET needs_sync = 1", [])?;
        Ok(())
    }

    /// Store a row from elsewhere, whose fields are already in stored form, if it's newer
    /// than what's here. Conflicts are last-write-wins on modifiedAt, on every client.
    pub fn apply(&self, row: &SyncSavedSearch, needs_sync: bool, synced_at: Option<DateTime<Utc>>) -> Result<bool> {
        let local: Option<DateTime<Utc>> = self.conn
            .query_row("SELECT modified_at FROM saved_searches WHERE id = ?1", params![&row.id], |r| stored_time(r, 0))
            .optional()?;

        if local.is_some_and(|local| row.modified_at <= local) {
            return Ok(false);
        }

        self.conn.execute(
            "INSERT OR REPLACE INTO saved_searches
                (id, name, query, sort_order, created_at, modified_at, synced_at, deleted, deleted_at, version, needs_sync)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &row.id,
                &row.name,
                &row.query,
                row.order,
                row.created_at.to_rfc3339(),
                row.modified_at.to_rfc3339(),
                synced_at.map(|at| at.to_rfc3339()),
                row.deleted as i32,
                row.deleted_at.map(|at| at.to_rfc3339()),
                row.version,
                needs_sync as i32,
            ],
        )?;
        Ok(true)
    }

    /// Every row as stored, deleted ones included, for a backup.
    pub fn list_stored(&self) -> Result<Vec<SyncSavedSearch>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {COLUMNS} FROM saved_searches ORDER BY sort_order, created_at"))?;
        let rows = stmt.query_map([], stored_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn database() -> (Database, [u8; 32]) {
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        (db, CryptoService::new().generate_master_key())
    }

    #[test]
    fn a_saved_search_is_encrypted_and_comes_back() {
        let (db, key) = database();
        let repo = SavedSearchRepository::new(db.connection());

        repo.create("Work", "#work -done", &key).unwrap();

        let stored: (String, String) = db.connection()
            .query_row("SELECT name, query FROM saved_searches", [], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        assert!(!stored.0.contains("Work"));
        assert!(!stored.1.contains("#work"));

        let listed = repo.list(&key).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].name.as_str(), listed[0].query.as_str()), ("Work", "#work -done"));
    }

    #[test]
    fn new_searches_go_to_the_end_of_the_list() {
        let (db, key) = database();
        let repo = SavedSearchRepository::new(db.connection());

        repo.create("First", "a", &key).unwrap();
        repo.create("Second", "b", &key).unwrap();

        let names: Vec<String> = repo.list(&key).unwrap().into_iter().map(|search| search.name).collect();
        assert_eq!(names, ["First", "Second"]);
    }

    #[test]
    fn a_deleted_search_leaves_the_list_but_goes_to_the_server() {
        let (db, key) = database();
        let repo = SavedSearchRepository::new(db.connection());
        let search = repo.create("Gone", "x", &key).unwrap();

        repo.delete(&search.id).unwrap();

        assert!(repo.list(&key).unwrap().is_empty());
        let pending = repo.pending().unwrap();
        assert!(pending[0].deleted);
        assert_eq!(pending[0].version, 2);
    }

    #[test]
    fn the_later_write_wins() {
        let (db, key) = database();
        let repo = SavedSearchRepository::new(db.connection());
        repo.create("Here", "x", &key).unwrap();
        let mut row = repo.pending().unwrap().remove(0);

        let earlier = row.modified_at - chrono::Duration::minutes(1);
        row.modified_at = earlier;
        assert!(!repo.apply(&row, false, None).unwrap());
        assert_eq!(repo.pending().unwrap().len(), 1);

        row.modified_at = earlier + chrono::Duration::minutes(2);
        assert!(repo.apply(&row, false, None).unwrap());
        assert!(repo.pending().unwrap().is_empty());
    }

    #[test]
    fn a_change_made_during_a_push_stays_pending() {
        let (db, key) = database();
        let repo = SavedSearchRepository::new(db.connection());
        let search = repo.create("Here", "x", &key).unwrap();
        let sent = repo.pending().unwrap().remove(0);

        repo.delete(&search.id).unwrap();
        repo.mark_pushed(&sent, Utc::now()).unwrap();

        assert_eq!(repo.pending().unwrap().len(), 1);
    }

    #[test]
    fn a_row_from_another_key_is_not_readable() {
        let (db, key) = database();
        let repo = SavedSearchRepository::new(db.connection());
        repo.create("Here", "x", &key).unwrap();
        let row = repo.pending().unwrap().remove(0);

        assert!(repo.is_readable(&row, &key));
        assert!(!repo.is_readable(&row, &CryptoService::new().generate_master_key()));
    }
}
