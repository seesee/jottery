use sqlx::{
    migrate::{MigrateError, Migrator},
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::{str::FromStr, time::Duration};

pub mod attachments;
pub mod sessions;
pub mod users;

pub use attachments::{AttachmentRepository, BlobStored};
pub use sessions::SessionRepository;
pub use users::UserRepository;

/// Every migration, embedded in the binary.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// How long a statement waits for SQLite's single write lock before failing.
/// A push holds that lock for one transaction, so this needs to outlast a
/// large push rather than sqlx's default of five seconds.
const BUSY_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?.busy_timeout(BUSY_TIMEOUT);

    // Foreign keys are on by default, on every connection in the pool. The journal
    // stays in its default mode on purpose: in WAL mode, recent commits sit in a
    // second file, so the plain copy of the database file that server/README.md
    // describes would quietly miss them.
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
}

/// How often a device's or session's "last active" time is written. A write needs
/// SQLite's one write lock, so doing it on each request would make every read wait
/// behind every other client's write, for a time nobody needs to the second.
const ACTIVITY_WRITE_INTERVAL_SECS: i64 = 60;

/// Whether a "last active" time is old enough to be worth writing again.
pub fn activity_is_stale(last_active: Option<&str>) -> bool {
    last_active
        .and_then(|time| chrono::DateTime::parse_from_rfc3339(time).ok())
        .is_none_or(|time| {
            (chrono::Utc::now() - time.with_timezone(&chrono::Utc)).num_seconds()
                >= ACTIVITY_WRITE_INTERVAL_SECS
        })
}

/// Apply pending migrations with foreign keys off.
///
/// Always use this rather than `MIGRATOR.run` on a database that may hold data.
/// A migration that rebuilds a table has to drop the old one, and DROP TABLE
/// performs an implicit DELETE that fires `ON DELETE CASCADE`, emptying every
/// child table. The pragma must be set here because it is a no-op inside a
/// transaction, and sqlx's SQLite driver runs every migration in one: it ignores
/// the `-- no-transaction` marker.
///
/// This also makes upgrades from before migration 004 safe. Those migrations
/// predate this and would otherwise lose data the same way.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), MigrateError> {
    // The pragma is per connection, and the migrator runs on this one.
    let mut conn = pool.acquire().await?;

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;
    let result = MIGRATOR.run(&mut *conn).await;
    // This connection goes back to the pool, so restore it whatever happened.
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await?;
    result?;

    // Rows that were already orphaned aren't worth refusing to start over, but
    // they shouldn't pass unnoticed.
    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&mut *conn)
        .await?;
    if !violations.is_empty() {
        tracing::warn!(
            "Database has {} rows with dangling foreign keys",
            violations.len()
        );
    }

    // Addresses didn't use to be compared without case, so an install can hold
    // accounts that differ only by case. Both keep working, and need a person to
    // decide which one is real.
    let collisions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (
             SELECT 1 FROM users GROUP BY email COLLATE NOCASE HAVING COUNT(*) > 1
         )",
    )
    .fetch_one(&mut *conn)
    .await?;
    if collisions > 0 {
        tracing::warn!(
            "{} email addresses belong to more than one account, differing only by case",
            collisions
        );
    }

    Ok(())
}
