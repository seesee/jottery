// Library exports for jottery-server
pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod services;
pub mod utils;

// Re-export types for tests and external use
pub use api::sse::{SyncBroadcast, SyncNotification};

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: config::Config,
    pub sync_broadcast: SyncBroadcast,
}
