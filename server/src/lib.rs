// Library exports for jottery-server
pub mod api;
pub mod app;
pub mod audit;
pub mod bootstrap;
pub mod config;
pub mod db;
pub mod error;
pub mod maintenance;
pub mod models;
pub mod rate_limit;
pub mod request_limits;
pub mod services;
pub mod shutdown;
pub mod utils;

// Re-export types for tests and external use
pub use api::sse::{SseTokenStore, SyncBroadcast, SyncNotification};

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: config::Config,
    pub sync_broadcast: SyncBroadcast,
    pub sse_tokens: SseTokenStore,
    pub login_throttle: std::sync::Arc<rate_limit::LoginThrottle>,
    pub shutdown: shutdown::Shutdown,
}
