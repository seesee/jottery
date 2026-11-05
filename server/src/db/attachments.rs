//! Attachment storage.
//!
//! An attachment belongs to a user, not to a note: `note_attachments` records
//! which notes reference it, so a note can share an attachment with another
//! note, and the blob outlives any one of them.
//!
//! `attachments_meta.id` is the key of the blob in `attachments_data`. Clients
//! only ever see `attachment_id`, which is unique per user, so every lookup here
//! goes through `(user_id, attachment_id)`.

use sqlx::{Sqlite, SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::models::AttachmentRef;

pub struct AttachmentRepository;

/// What happened to a blob pushed by a client.
#[derive(Debug, PartialEq, Eq)]
pub enum BlobStored {
    /// The user has no such attachment, so nothing was stored.
    NoMetadata,
    /// First time the server has held this blob.
    Added,
    /// The server already held a blob for this attachment.
    Replaced,
}

impl AttachmentRepository {
    /// Record that a note references an attachment, creating or updating the
    /// attachment's metadata.
    pub async fn link_to_note(
        conn: &mut SqliteConnection,
        user_id: &str,
        note_id: &str,
        attachment: &AttachmentRef,
        now: &str,
    ) -> Result<(), sqlx::Error> {
        let blob_key = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"
            INSERT INTO attachments_meta (id, attachment_id, user_id, filename, mime_type, size, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, attachment_id) DO UPDATE SET
                filename = excluded.filename,
                mime_type = excluded.mime_type,
                size = excluded.size,
                unreferenced_at = NULL
            "#,
            blob_key,
            attachment.id,
            user_id,
            attachment.filename,
            attachment.mime_type,
            attachment.size,
            now
        )
        .execute(&mut *conn)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO note_attachments (user_id, note_id, attachment_id)
            VALUES (?, ?, ?)
            ON CONFLICT DO NOTHING
            "#,
            user_id,
            note_id,
            attachment.id
        )
        .execute(&mut *conn)
        .await?;

        Ok(())
    }

    /// Remove a note's references to everything except `keep`. The attachments
    /// themselves stay until housekeeping finds nothing else needs them.
    pub async fn unlink_others(
        conn: &mut SqliteConnection,
        user_id: &str,
        note_id: &str,
        keep: &[&str],
    ) -> Result<(), sqlx::Error> {
        let mut query: sqlx::QueryBuilder<'_, Sqlite> =
            sqlx::QueryBuilder::new("DELETE FROM note_attachments WHERE user_id = ");
        query.push_bind(user_id);
        query.push(" AND note_id = ");
        query.push_bind(note_id);
        if !keep.is_empty() {
            query.push(" AND attachment_id NOT IN (");
            let mut ids = query.separated(", ");
            for attachment_id in keep {
                ids.push_bind(*attachment_id);
            }
            ids.push_unseparated(")");
        }

        query.build().execute(&mut *conn).await?;
        Ok(())
    }

    /// The attachments of the given notes, as `(note_id, attachment)` pairs in
    /// the order the server first saw them.
    pub async fn refs_for_notes(
        conn: &mut SqliteConnection,
        user_id: &str,
        note_ids: &[&str],
    ) -> Result<Vec<(String, AttachmentRef)>, sqlx::Error> {
        if note_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query: sqlx::QueryBuilder<'_, Sqlite> = sqlx::QueryBuilder::new(
            "SELECT na.note_id, m.attachment_id, m.filename, m.mime_type, m.size
             FROM note_attachments na
             JOIN attachments_meta m
               ON m.user_id = na.user_id AND m.attachment_id = na.attachment_id
             WHERE na.user_id = ",
        );
        query.push_bind(user_id);
        query.push(" AND na.note_id IN (");
        let mut ids = query.separated(", ");
        for note_id in note_ids {
            ids.push_bind(*note_id);
        }
        ids.push_unseparated(") ORDER BY m.rowid");

        let rows: Vec<(String, String, String, String, i64)> =
            query.build_query_as().fetch_all(&mut *conn).await?;

        Ok(rows
            .into_iter()
            .map(|(note_id, id, filename, mime_type, size)| {
                let attachment = AttachmentRef {
                    data: id.clone(),
                    id,
                    filename,
                    mime_type,
                    size,
                };
                (note_id, attachment)
            })
            .collect())
    }

    /// The size of each stored blob of the given notes, as
    /// `(note_id, attachment_id, bytes)`. Reads no blob data.
    pub async fn blob_sizes(
        conn: &mut SqliteConnection,
        user_id: &str,
        note_ids: &[&str],
    ) -> Result<Vec<(String, String, usize)>, sqlx::Error> {
        if note_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query: sqlx::QueryBuilder<'_, Sqlite> = sqlx::QueryBuilder::new(
            "SELECT na.note_id, m.attachment_id, LENGTH(d.data)
             FROM note_attachments na
             JOIN attachments_meta m
               ON m.user_id = na.user_id AND m.attachment_id = na.attachment_id
             JOIN attachments_data d ON d.id = m.id
             WHERE na.user_id = ",
        );
        query.push_bind(user_id);
        query.push(" AND na.note_id IN (");
        let mut ids = query.separated(", ");
        for note_id in note_ids {
            ids.push_bind(*note_id);
        }
        ids.push_unseparated(")");

        let rows: Vec<(String, String, i64)> = query.build_query_as().fetch_all(&mut *conn).await?;

        Ok(rows
            .into_iter()
            .map(|(note_id, attachment_id, size)| (note_id, attachment_id, size as usize))
            .collect())
    }

    /// Store a blob for an attachment this user has metadata for. Without
    /// metadata nothing is stored, which keeps other users' attachments out of
    /// reach.
    pub async fn store_blob(
        conn: &mut SqliteConnection,
        user_id: &str,
        attachment_id: &str,
        data: &[u8],
        now: &str,
    ) -> Result<BlobStored, sqlx::Error> {
        let existing = sqlx::query!(
            r#"SELECT m.id AS "blob_key!: String", d.id AS "existing_blob: String"
               FROM attachments_meta m
               LEFT JOIN attachments_data d ON d.id = m.id
               WHERE m.user_id = ? AND m.attachment_id = ?"#,
            user_id,
            attachment_id
        )
        .fetch_optional(&mut *conn)
        .await?;

        let Some(existing) = existing else {
            return Ok(BlobStored::NoMetadata);
        };

        sqlx::query!(
            r#"
            INSERT INTO attachments_data (id, data, created_at)
            VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET data = excluded.data
            "#,
            existing.blob_key,
            data,
            now
        )
        .execute(&mut *conn)
        .await?;

        Ok(if existing.existing_blob.is_some() {
            BlobStored::Replaced
        } else {
            BlobStored::Added
        })
    }

    /// The notes that reference an attachment.
    pub async fn notes_referencing(
        conn: &mut SqliteConnection,
        user_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT note_id FROM note_attachments WHERE user_id = ? AND attachment_id = ?",
            user_id,
            attachment_id
        )
        .fetch_all(&mut *conn)
        .await
    }

    /// Fetch the blobs of this user's attachments as `(attachment_id, data)`.
    pub async fn blobs(
        conn: &mut SqliteConnection,
        user_id: &str,
        attachment_ids: &[&str],
    ) -> Result<Vec<(String, Vec<u8>)>, sqlx::Error> {
        if attachment_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query: sqlx::QueryBuilder<'_, Sqlite> = sqlx::QueryBuilder::new(
            "SELECT m.attachment_id, d.data
             FROM attachments_meta m
             JOIN attachments_data d ON d.id = m.id
             WHERE m.user_id = ",
        );
        query.push_bind(user_id);
        query.push(" AND m.attachment_id IN (");
        let mut ids = query.separated(", ");
        for attachment_id in attachment_ids {
            ids.push_bind(*attachment_id);
        }
        ids.push_unseparated(")");

        query.build_query_as().fetch_all(&mut *conn).await
    }

    /// Delete attachments nothing needs any more, and return how many went.
    ///
    /// An attachment is kept while a note references it, or a version snapshot
    /// does, because restoring that version needs the blob. Once neither is
    /// true it is kept for `grace` as well, and only deleted if nothing has
    /// referenced it again by then. Deleting the metadata removes the blob.
    ///
    /// Version snapshots keep attachment references as JSON. Searching that text
    /// for the id can only err towards keeping a blob.
    pub async fn purge_unreferenced(
        pool: &SqlitePool,
        grace: chrono::Duration,
    ) -> Result<u64, sqlx::Error> {
        let now = chrono::Utc::now();
        let started = now.to_rfc3339();
        let cutoff = (now - grace).to_rfc3339();

        sqlx::query!(
            "UPDATE attachments_meta SET unreferenced_at = ?
             WHERE unreferenced_at IS NULL AND NOT EXISTS (
                 SELECT 1 FROM note_attachments na
                 WHERE na.user_id = attachments_meta.user_id
                   AND na.attachment_id = attachments_meta.attachment_id
             )",
            started
        )
        .execute(pool)
        .await?;

        let result = sqlx::query!(
            "DELETE FROM attachments_meta
             WHERE unreferenced_at < ?
               AND NOT EXISTS (
                   SELECT 1 FROM note_attachments na
                   WHERE na.user_id = attachments_meta.user_id
                     AND na.attachment_id = attachments_meta.attachment_id
               )
               AND NOT EXISTS (
                   SELECT 1 FROM note_versions v
                   WHERE v.user_id = attachments_meta.user_id
                     AND instr(v.attachments, attachments_meta.attachment_id) > 0
               )",
            cutoff
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}
