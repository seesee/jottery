//! Attachment storage.
//!
//! An attachment belongs to a user, not to a note: `note_attachments` records
//! which notes reference it, so a note can share an attachment with another
//! note, and the blob outlives any one of them.
//!
//! `attachments_meta.id` is the key of the blob in `attachments_data`. Clients
//! only ever see `attachment_id`, which is unique per user, so every lookup here
//! goes through `(user_id, attachment_id)`.

use sqlx::{Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::models::AttachmentRef;

pub struct AttachmentRepository;

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

    /// Store a blob for an attachment this user has metadata for. Returns false
    /// if they have none, which leaves other users' attachments out of reach.
    pub async fn store_blob(
        conn: &mut SqliteConnection,
        user_id: &str,
        attachment_id: &str,
        data: &[u8],
        now: &str,
    ) -> Result<bool, sqlx::Error> {
        let blob_key = sqlx::query_scalar!(
            "SELECT id FROM attachments_meta WHERE user_id = ? AND attachment_id = ?",
            user_id,
            attachment_id
        )
        .fetch_optional(&mut *conn)
        .await?;

        let Some(blob_key) = blob_key else {
            return Ok(false);
        };

        sqlx::query!(
            r#"
            INSERT INTO attachments_data (id, data, created_at)
            VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET data = excluded.data
            "#,
            blob_key,
            data,
            now
        )
        .execute(&mut *conn)
        .await?;

        Ok(true)
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
}
