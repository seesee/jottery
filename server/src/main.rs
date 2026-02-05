use axum::{
    extract::DefaultBodyLimit,
    http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderName},
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::HeaderValue;

mod api;
mod config;
mod db;
mod error;
mod models;
mod utils;

use crate::api::sse::{SseTokenStore, SyncBroadcast, SyncNotification, create_token_store};
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub config: Config,
    pub sync_broadcast: SyncBroadcast,
    pub sse_tokens: SseTokenStore,
}

/// Build CORS layer based on configuration
///
/// If CORS_ALLOWED_ORIGINS is set, only allow those origins.
/// Otherwise, allow any origin (useful for development/simple deployments).
fn build_cors_layer(config: &Config) -> CorsLayer {
    let mut cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(vec![
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-api-key"),
        ]);

    if let Some(ref origins) = config.cors_allowed_origins {
        if origins.iter().any(|o| o == "*") {
            // Explicit wildcard allows any origin
            tracing::warn!("CORS: Allowing any origin (wildcard configured)");
            cors = cors.allow_origin(Any);
        } else {
            // Use specific origins from configuration
            tracing::info!("CORS: Allowing specific origins: {:?}", origins);
            let allowed_origins: Vec<_> = origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect();
            cors = cors.allow_origin(allowed_origins);
        }
    } else {
        // Default to same-origin only (no cross-origin requests)
        // This is secure by default - set CORS_ALLOWED_ORIGINS for cross-origin access
        tracing::info!("CORS: Same-origin only (set CORS_ALLOWED_ORIGINS to allow cross-origin requests)");
        // Don't set allow_origin - this effectively blocks cross-origin requests
    }

    cors
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // Load config
    let config = Config::from_env().expect("Failed to load configuration");
    tracing::info!("Starting Jottery Sync Server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Database: {}", config.database_url);
    tracing::info!("Port: {}", config.port);

    // Initialize database
    let pool = db::init_pool(&config.database_url)
        .await
        .expect("Failed to initialize database");

    tracing::info!("Database connection established");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database migrations complete");

    // Create broadcast channel for SSE sync notifications
    // Capacity of 100 is sufficient - notifications are small and ephemeral
    let (sync_broadcast, _) = broadcast::channel::<SyncNotification>(100);

    // Create SSE token store for short-lived tokens
    let sse_tokens = create_token_store();

    // Build application state
    let app_state = Arc::new(AppState {
        pool,
        config: config.clone(),
        sync_broadcast,
        sse_tokens,
    });

    // Build protected sync routes with API key auth middleware
    let sync_routes = Router::new()
        .route("/api/v1/sync/status", get(api::sync::get_status))
        .route("/api/v1/sync/push", post(api::sync::push))
        .route("/api/v1/sync/pull", post(api::sync::pull))
        .route("/api/v1/sync/notes/:id", delete(api::sync::delete_note))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            api::middleware::auth_middleware,
        ));

    // SSE routes for real-time sync notifications
    // Token endpoint exchanges API key for short-lived SSE token
    // Events endpoint uses the short-lived token (not the API key)
    let sse_routes = Router::new()
        .route("/api/v1/sync/events/token", get(api::sse::get_sse_token))
        .route("/api/v1/sync/events", get(api::sse::sync_events));

    // Build protected user routes with session auth middleware (for account management)
    let user_routes = Router::new()
        .route("/api/v1/user/account", get(api::user::get_account_info).delete(api::user::delete_account))
        .route("/api/v1/user/notes", delete(api::user::delete_all_notes))
        .route("/api/v1/user/change-password", post(api::user::change_password))
        .route("/api/v1/user/logout", post(api::user::logout))
        .route("/api/v1/user/devices", get(api::user::list_devices))
        .route("/api/v1/user/devices/:id", delete(api::user::revoke_device))
        .route("/api/v1/user/inbox-token",
            post(api::user::generate_inbox_token).delete(api::user::revoke_inbox_token))
        .route("/api/v1/user/inbox-token/status",
            get(api::user::get_inbox_token_status))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            api::middleware::user_auth_middleware,
        ));

    // Build protected admin routes with session auth middleware
    let admin_routes = Router::new()
        .route("/api/v1/admin/users", get(api::admin::users::list_users))
        .route("/api/v1/admin/users/:id", get(api::admin::users::get_user))
        .route("/api/v1/admin/users/:id/approve", post(api::admin::users::approve_user))
        .route("/api/v1/admin/users/:id/deactivate", post(api::admin::users::deactivate_user))
        .route("/api/v1/admin/users/:id/activate", post(api::admin::users::activate_user))
        .route("/api/v1/admin/users/:id/toggle-admin", post(api::admin::users::toggle_admin))
        .route("/api/v1/admin/users/:id", delete(api::admin::users::delete_user))
        .route("/api/v1/admin/users/:id/devices", get(api::admin::users::list_user_devices))
        .route("/api/v1/admin/users/:id/settings", patch(api::admin::users::update_user_settings))
        .route("/api/v1/admin/users/:id/reset-password", post(api::admin::users::reset_user_password))
        .route("/api/v1/admin/devices/:id", delete(api::admin::users::revoke_device).patch(api::admin::users::rename_device))
        .route("/api/v1/admin/stats", get(api::admin::stats::get_stats))
        .route("/api/v1/admin/audit", get(api::admin::stats::get_audit_log))
        .route("/api/v1/admin/notes/metadata", get(api::admin::stats::get_notes_metadata))
        .route("/api/v1/admin/change-password", post(api::admin::users::change_password))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            api::admin::admin_auth_middleware,
        ));

    // Serve admin dashboard static files
    let admin_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Failed to get parent directory")
        .join("admin/dist");
    let admin_index = admin_dir.join("index.html");

    tracing::info!("Serving admin dashboard from: {}", admin_dir.display());
    tracing::info!("Serving user portal from: {}", admin_dir.display());

    // Inbox submission routes (inbox token auth — limited scope)
    let inbox_submit_routes = Router::new()
        .route("/api/v1/inbox", post(api::inbox::create_item))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            api::middleware::inbox_auth_middleware,
        ));

    // Inbox management routes (device API key auth — same as sync)
    let inbox_manage_routes = Router::new()
        .route("/api/v1/inbox", get(api::inbox::list_items).delete(api::inbox::delete_all))
        .route("/api/v1/inbox/{id}", delete(api::inbox::delete_item))
        .route("/api/v1/inbox/status", get(api::inbox::get_status))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            api::middleware::auth_middleware,
        ));

    // Auth routes (rate limiting disabled - requires proxy-aware key extractor)
    // TODO: Re-enable with SmartIpKeyExtractor when running behind reverse proxy
    let auth_routes = Router::new()
        .route("/api/v1/auth/login", post(api::auth::login))
        .route("/api/v1/user/login", post(api::user::login));

    // Registration routes (rate limiting disabled - requires proxy-aware key extractor)
    let registration_routes = Router::new()
        .route("/api/v1/auth/register-user", post(api::auth::register_user))
        .route("/api/v1/auth/register-device", post(api::auth::register_device))
        .route("/api/v1/auth/clone-device", post(api::auth::clone_device));

    // Build main router
    let app = Router::new()
        // Health check (no auth required)
        .route("/health", get(health_check))
        // Rate-limited auth and registration routes
        .merge(auth_routes)
        .merge(registration_routes)
        // Status check (no rate limiting needed - it's read-only)
        .route("/api/v1/user/status", get(api::user::check_status))
        // Merge protected routes
        .merge(sync_routes)
        .merge(sse_routes)
        .merge(inbox_submit_routes)
        .merge(inbox_manage_routes)
        .merge(user_routes)
        .merge(admin_routes)
        // Serve admin dashboard and user portal (same SPA, different paths)
        // Must be after API routes to avoid conflicts
        .nest_service("/admin", ServeDir::new(&admin_dir).fallback(ServeFile::new(&admin_index)))
        .nest_service("/user", ServeDir::new(&admin_dir).fallback(ServeFile::new(&admin_index)))
        // Add state
        .with_state(app_state)
        // Add middleware
        .layer(DefaultBodyLimit::max(config.max_payload_size))
        .layer(CompressionLayer::new())
        .layer(build_cors_layer(&config))
        // Security headers
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ));

    // Conditionally add HSTS header (only for HTTPS deployments)
    let app = if config.enable_hsts {
        tracing::info!("HSTS enabled: Strict-Transport-Security header will be sent");
        app.layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
    } else {
        app
    };

    // Start server - try IPv6 first (which also accepts IPv4 on most systems),
    // fall back to IPv4-only if IPv6 is not available
    let addr_v6 = format!("[::]:{}", config.port);
    let addr_v4 = format!("0.0.0.0:{}", config.port);

    let listener = match tokio::net::TcpListener::bind(&addr_v6).await {
        Ok(l) => {
            tracing::info!("Listening on {} (IPv4 and IPv6)", addr_v6);
            l
        }
        Err(_) => {
            tracing::info!("IPv6 not available, listening on {} (IPv4 only)", addr_v4);
            tokio::net::TcpListener::bind(&addr_v4)
                .await
                .expect("Failed to bind to address")
        }
    };

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

async fn health_check() -> &'static str {
    "OK"
}
