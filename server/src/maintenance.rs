//! Periodic housekeeping, kept out of the request path.

use sqlx::SqlitePool;
use std::time::Duration;

/// How often housekeeping runs.
const INTERVAL: Duration = Duration::from_secs(60 * 60);

/// How long a soft-deleted note stays on the server before it is purged.
const DELETED_NOTE_RETENTION_DAYS: i64 = 30;

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
    Ok(())
}

/// Delete notes that have sat in the recycle bin past the retention period, so
/// they don't accumulate on the server indefinitely.
async fn purge_deleted_notes(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let cutoff =
        (chrono::Utc::now() - chrono::Duration::days(DELETED_NOTE_RETENTION_DAYS)).to_rfc3339();

    let result = sqlx::query!(
        "DELETE FROM notes WHERE deleted = 1 AND deleted_at IS NOT NULL AND deleted_at < ?",
        cutoff
    )
    .execute(pool)
    .await?;

    let purged = result.rows_affected();
    if purged > 0 {
        tracing::info!(
            "Purged {} notes deleted more than {} days ago",
            purged,
            DELETED_NOTE_RETENTION_DAYS
        );
    }
    Ok(purged)
}
