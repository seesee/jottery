// Library exports for jottery-server
pub mod api;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod services;
pub mod utils;

// Re-export types for tests and external use
pub use api::sse::{SseTokenStore, SyncBroadcast, SyncNotification};

use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: config::Config,
    pub sync_broadcast: SyncBroadcast,
    pub sse_tokens: SseTokenStore,
    /// WebAuthn instance, built at startup from config. None when the
    /// server isn't configured for passkey support (all three
    /// `WEBAUTHN_RP_*` env vars must be set; a partial config is
    /// treated as "disabled" at Config::webauthn_enabled()).
    pub webauthn: Option<Arc<webauthn_rs::Webauthn>>,
}
