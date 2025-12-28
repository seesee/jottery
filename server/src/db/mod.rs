use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

pub mod sessions;
pub mod users;

pub use sessions::SessionRepository;
pub use users::UserRepository;

pub async fn init_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Enable foreign keys
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;

    Ok(pool)
}
