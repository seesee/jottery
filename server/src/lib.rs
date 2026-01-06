// Library exports for jottery-server
pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod utils;

// Re-export AppState for tests
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: config::Config,
}
