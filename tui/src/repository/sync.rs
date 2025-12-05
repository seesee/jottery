use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::sync::{NoteSyncMetadata, SyncMetadata, SyncStatus};

/// Repository for sync metadata operations
pub struct SyncRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SyncRepository<'a> {
    /// Create a new sync repository
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get global sync metadata
    pub fn get_metadata(&self) -> Result<Option<SyncMetadata>> {
        let result = self.conn
            .query_row(
                "SELECT last_sync_at, last_push_at, last_pull_at, api_key, client_id,
                        sync_enabled, sync_endpoint, auto_sync_interval
                 FROM sync_metadata WHERE id = 1",
                [],
                |row| {
                    Ok(SyncMetadata {
                        last_sync_at: row
                            .get::<_, Option<String>>(0)?
                            .map(|s| s.parse())
                            .transpose()
                            .ok()
                            .flatten(),
                        last_push_at: row
                            .get::<_, Option<String>>(1)?
                            .map(|s| s.parse())
                            .transpose()
                            .ok()
                            .flatten(),
                        last_pull_at: row
                            .get::<_, Option<String>>(2)?
                            .map(|s| s.parse())
                            .transpose()
                            .ok()
                            .flatten(),
                        api_key: row.get(3)?,
                        client_id: row.get(4)?,
                        sync_enabled: row.get::<_, i32>(5)? != 0,
                        sync_endpoint: row.get(6)?,
                        auto_sync_interval: row.get(7)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Update global sync metadata
    pub fn update_metadata(&self, metadata: &SyncMetadata) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_metadata (
                id, last_sync_at, last_push_at, last_pull_at, api_key, client_id,
                sync_enabled, sync_endpoint, auto_sync_interval
            ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                metadata.last_sync_at.map(|dt| dt.to_rfc3339()),
                metadata.last_push_at.map(|dt| dt.to_rfc3339()),
                metadata.last_pull_at.map(|dt| dt.to_rfc3339()),
                &metadata.api_key,
                &metadata.client_id,
                metadata.sync_enabled as i32,
                &metadata.sync_endpoint,
                metadata.auto_sync_interval,
            ],
        )?;

        Ok(())
    }

    /// Update specific sync metadata fields
    pub fn update_last_sync(&self, timestamp: DateTime<Utc>) -> Result<()> {
        self.conn.execute(
            "UPDATE sync_metadata SET last_sync_at = ?1 WHERE id = 1",
            params![timestamp.to_rfc3339()],
        )?;
        Ok(())
    }

    /// Set API key and client ID
    pub fn set_credentials(&self, api_key: &str, client_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sync_metadata SET api_key = ?1, client_id = ?2 WHERE id = 1",
            params![api_key, client_id],
        )?;
        Ok(())
    }

    /// Enable/disable sync
    pub fn set_sync_enabled(&self, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE sync_metadata SET sync_enabled = ?1 WHERE id = 1",
            params![enabled as i32],
        )?;
        Ok(())
    }

    /// Get per-note sync metadata
    pub fn get_note_metadata(&self, note_id: &str) -> Result<Option<NoteSyncMetadata>> {
        let result = self.conn
            .query_row(
                "SELECT note_id, synced_at, sync_hash, server_version, last_sync_status, error_message
                 FROM note_sync_metadata WHERE note_id = ?1",
                params![note_id],
                |row| {
                    Ok(NoteSyncMetadata {
                        note_id: row.get(0)?,
                        synced_at: row.get::<_, String>(1)?.parse().unwrap(),
                        sync_hash: row.get(2)?,
                        server_version: row.get(3)?,
                        last_sync_status: parse_sync_status(&row.get::<_, String>(4)?),
                        error_message: row.get(5)?,
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Update per-note sync metadata
    pub fn update_note_metadata(&self, metadata: &NoteSyncMetadata) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO note_sync_metadata (
                note_id, synced_at, sync_hash, server_version, last_sync_status, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &metadata.note_id,
                metadata.synced_at.to_rfc3339(),
                &metadata.sync_hash,
                metadata.server_version,
                metadata.last_sync_status.to_string(),
                &metadata.error_message,
            ],
        )?;

        Ok(())
    }

    /// Get all pending notes (notes that need to sync)
    pub fn get_pending_notes(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT note_id FROM note_sync_metadata WHERE last_sync_status = 'pending'"
        )?;

        let note_ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(note_ids)
    }

    /// Get count of notes by sync status
    pub fn count_by_status(&self, status: SyncStatus) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM note_sync_metadata WHERE last_sync_status = ?1",
            params![status.to_string()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Clear all sync metadata (for re-registration)
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM sync_metadata WHERE id = 1", [])?;
        self.conn.execute("DELETE FROM note_sync_metadata", [])?;
        Ok(())
    }
}

/// Parse sync status string
fn parse_sync_status(s: &str) -> SyncStatus {
    match s.to_lowercase().as_str() {
        "synced" => SyncStatus::Synced,
        "pending" => SyncStatus::Pending,
        "conflict" => SyncStatus::Conflict,
        "error" => SyncStatus::Error,
        _ => SyncStatus::Pending,
    }
}
