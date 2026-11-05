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

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    // Enable foreign keys
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    Ok(pool)
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

    Ok(())
}
