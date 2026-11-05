//! Periodic housekeeping, kept out of the request path.

use sqlx::SqlitePool;
use std::time::Duration;

use crate::api::sync::record_tombstone;
use crate::db::{AttachmentRepository, SessionRepository, UserRepository};

/// How often housekeeping runs.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How long a soft-deleted note stays on the server before it is purged.
const DELETED_NOTE_RETENTION_DAYS: i64 = 30;

/// Notes purged per transaction, so that other writers aren't held up for long.
const PURGE_BATCH_SIZE: i64 = 500;

/// How long an attachment nothing references is kept before it is deleted. This
/// covers a device that is still catching up and re-links it.
const UNREFERENCED_ATTACHMENT_RETENTION_DAYS: i64 = 7;

/// Run housekeeping now and then every hour, for the life of the process.
pub fn spawn(pool: SqlitePool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(INTERVAL);
        loop {
            interval.tick().await;
            if let Err(e) = run_once(&pool).await {
                tracing::warn!("Housekeeping failed: {}", e);
            }
        }
    });
}

/// One housekeeping pass.
pub async fn run_once(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    purge_deleted_notes(pool).await?;

    let purged = AttachmentRepository::purge_unreferenced(
        pool,
        chrono::Duration::days(UNREFERENCED_ATTACHMENT_RETENTION_DAYS),
    )
    .await?;
    if purged > 0 {
        tracing::info!("Purged {} attachments nothing references", purged);
    }

    refresh_storage_used(pool).await?;

    // A session is otherwise only removed if its token is presented after it expires
    let expired = SessionRepository::delete_expired(pool).await?;
    if expired > 0 {
        tracing::info!("Removed {} expired sessions", expired);
    }

    Ok(())
}

/// Bring each user's stored usage back to what's really there. A push only adds to
/// it, which overcounts edits and doesn't see deletions. Each user is measured
/// outside any transaction, so that this doesn't hold the write lock while it reads.
async fn refresh_storage_used(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let users = sqlx::query!("SELECT id, storage_quota_mb FROM users")
        .fetch_all(pool)
        .await?;

    for user in users {
        let used = UserRepository::measure_storage(pool, &user.id).await?;
        sqlx::query!(
            "UPDATE users SET storage_used_bytes = ? WHERE id = ?",
            used,
            user.id
        )
        .execute(pool)
        .await?;

        if user.storage_quota_mb.is_some_and(|quota| used > quota * 1024 * 1024) {
            tracing::warn!("User {} is over their storage quota: {} bytes", user.id, used);
        }
    }

    Ok(())
}

/// Delete notes that have sat in the recycle bin past the retention period, so
/// they don't accumulate on the server indefinitely.
///
/// The period runs from when the server last changed the note. `deleted_at` comes
/// from the client, and a note that arrives with an old one must still stay long
/// enough for other devices to see it.
///
/// Each note leaves a tombstone in the same transaction. A device that was offline
/// for the whole retention period would otherwise keep the note live for ever, and
/// push it back to every device on its next edit.
async fn purge_deleted_notes(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let mut purged = 0;

    loop {
        let now = chrono::Utc::now();
        let cutoff = (now - chrono::Duration::days(DELETED_NOTE_RETENTION_DAYS)).to_rfc3339();
        let now = now.to_rfc3339();

        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        let due = sqlx::query!(
            "SELECT id, user_id, deleted_at FROM notes
             WHERE deleted = 1 AND server_modified_at < ?
             LIMIT ?",
            cutoff,
            PURGE_BATCH_SIZE
        )
        .fetch_all(&mut *tx)
        .await?;

        for note in &due {
            let deleted_at = note.deleted_at.as_deref().unwrap_or(&now);
            sqlx::query!(
                "DELETE FROM notes WHERE id = ? AND user_id = ?",
                note.id,
                note.user_id
            )
            .execute(&mut *tx)
            .await?;
            record_tombstone(&mut tx, &note.user_id, &note.id, deleted_at, None, &now).await?;
        }

        tx.commit().await?;

        purged += due.len() as u64;
        if (due.len() as i64) < PURGE_BATCH_SIZE {
            break;
        }
    }

    if purged > 0 {
        tracing::info!(
            "Purged {} notes deleted more than {} days ago",
            purged,
            DELETED_NOTE_RETENTION_DAYS
        );
    }
    Ok(purged)
}
