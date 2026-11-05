//! HTTP application: the router, its middleware stack and the routes.
//!
//! Lives in the library so integration tests exercise the same router that
//! `main` serves, rather than each rebuilding a partial copy.

use axum::{
    extract::DefaultBodyLimit,
    http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderName, IF_MATCH},
    http::HeaderValue,
    routing::{delete, get, patch, post},
    Router,
};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::{
    api,
    config::Config,
    rate_limit,
    request_limits::{self, RequestLimits},
    AppState,
};

/// Body limit for every route that doesn't set its own, which includes all those
/// open to anonymous callers. A JSON body is held in memory whole before it's read.
const SMALL_BODY_LIMIT: usize = 64 * 1024;

/// Body limit for a pull, which lists the attachments a client already holds.
const PULL_BODY_LIMIT: usize = 16 * 1024 * 1024;

/// What an inbox submission of `max_content` bytes can take as JSON, where escaping
/// can double it.
fn inbox_body_limit(max_content: usize) -> usize {
    max_content * 2 + SMALL_BODY_LIMIT
}

/// Build CORS layer based on configuration
///
/// If CORS_ALLOWED_ORIGINS is set, only allow those origins.
/// Otherwise, allow any origin (useful for development/simple deployments).
///
/// Never allow credentials here: it's what stops another origin's script sending a
/// session cookie, with the header that `api::cookies` asks for.
fn build_cors_layer(config: &Config) -> CorsLayer {
    let mut cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(vec![
            AUTHORIZATION,
            CONTENT_TYPE,
            IF_MATCH,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static(api::cookies::CSRF_HEADER),
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

/// Build the complete application router for the given state.
pub fn build_app(state: Arc<AppState>) -> Router {
    let config = &state.config;

    // Build protected sync routes with API key auth middleware
    let sync_routes = Router::new()
        .route("/api/v1/sync/status", get(api::sync::get_status))
        .route(
            "/api/v1/sync/push",
            post(api::sync::push).layer(DefaultBodyLimit::max(config.max_payload_size)),
        )
        .route(
            "/api/v1/sync/pull",
            post(api::sync::pull).layer(DefaultBodyLimit::max(PULL_BODY_LIMIT)),
        )
        .route("/api/v1/sync/attachments/:id", get(api::sync::get_attachment))
        .route("/api/v1/sync/notes/:id", delete(api::sync::delete_note))
        .route("/api/v1/sync/device", delete(api::sync::revoke_self))
        .route(
            "/api/v1/auth/wrapped-key",
            get(api::auth::get_wrapped_key).put(api::auth::put_wrapped_key),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
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
        .route("/api/v1/user/wrapped-key", get(api::auth::get_wrapped_key_session).put(api::auth::put_wrapped_key_session))
        .route("/api/v1/user/logout", post(api::user::logout))
        .route("/api/v1/user/devices", get(api::user::list_devices))
        .route("/api/v1/user/devices/:id", delete(api::user::revoke_device))
        .route("/api/v1/user/inbox-token",
            post(api::user::generate_inbox_token).delete(api::user::revoke_inbox_token))
        .route("/api/v1/user/inbox-token/status",
            get(api::user::get_inbox_token_status))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api::middleware::user_auth_middleware,
        ));

    // Build protected admin routes with session auth middleware
    let admin_routes = Router::new()
        .route("/api/v1/admin/me", get(api::admin::users::me))
        .route("/api/v1/admin/logout", post(api::admin::users::logout))
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
            state.clone(),
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
        .route(
            "/api/v1/inbox",
            post(api::inbox::create_item)
                .layer(DefaultBodyLimit::max(inbox_body_limit(config.max_inbox_content_size))),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api::middleware::inbox_auth_middleware,
        ));

    // Inbox management routes (device API key auth — same as sync)
    let inbox_manage_routes = Router::new()
        .route("/api/v1/inbox", get(api::inbox::list_items).delete(api::inbox::delete_all))
        .route("/api/v1/inbox/:id", delete(api::inbox::delete_item))
        .route("/api/v1/inbox/status", get(api::inbox::get_status))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api::middleware::auth_middleware,
        ));

    // Routes that take a credential. Password guessing is also throttled per
    // email address inside the handlers.
    let credential_routes = rate_limit::limit_credentials(
        Router::new()
            .route("/api/v1/auth/login", post(api::auth::login))
            .route("/api/v1/user/login", post(api::user::login))
            .route("/api/v1/user/status", post(api::user::check_own_status))
            .route("/api/v1/auth/register-device", post(api::auth::register_device))
            .route("/api/v1/auth/clone-device", post(api::auth::clone_device)),
        &config.rate_limit,
    );

    // Account creation, which lands in the admin's pending list
    let registration_routes = rate_limit::limit_registration(
        Router::new().route("/api/v1/auth/register-user", post(api::auth::register_user)),
        &config.rate_limit,
    );

    // Approval status, which clients poll while they wait
    let status_routes = rate_limit::limit_status(
        Router::new().route("/api/v1/user/status", get(api::user::check_status)),
        &config.rate_limit,
    );

    // Build main router
    let app = Router::new()
        // Health check (no auth required)
        .route("/health", get(health_check))
        // Rate-limited anonymous routes
        .merge(credential_routes)
        .merge(registration_routes)
        .merge(status_routes)
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
        .with_state(state.clone())
        // Add middleware
        .layer(DefaultBodyLimit::max(SMALL_BODY_LIMIT))
        .layer(axum::middleware::from_fn_with_state(
            RequestLimits::default(),
            request_limits::enforce,
        ))
        .layer(CompressionLayer::new())
        .layer(build_cors_layer(config))
        // Security headers (OWASP recommended)
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
        ))
        // Content-Security-Policy — path-aware: permissive for SPA, restrictive for API
        .layer(axum::middleware::from_fn(csp_middleware))
        // Prevent caching of API responses (contains sensitive data)
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate, private"),
        ))
        // Prevent Adobe cross-domain policy file loading
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-permitted-cross-domain-policies"),
            HeaderValue::from_static("none"),
        ));

    // Conditionally add HSTS header (only for HTTPS deployments)
    if config.enable_hsts {
        tracing::info!("HSTS enabled: Strict-Transport-Security header will be sent");
        app.layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
    } else {
        app
    }
}

/// Sets a path-appropriate Content-Security-Policy header.
///
/// - `/admin` and `/user` routes serve a Svelte SPA that needs to load its own
///   scripts, stylesheets, and make API calls back to the server.
/// - All other routes (API) only serve JSON, so a restrictive policy is applied.
async fn csp_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;

    if !response.headers().contains_key(axum::http::header::CONTENT_SECURITY_POLICY) {
        let csp = if path.starts_with("/admin") || path.starts_with("/user") {
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'"
        } else {
            "default-src 'none'; frame-ancestors 'none'"
        };
        response.headers_mut().insert(
            axum::http::header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(csp),
        );
    }

    response
}

async fn health_check() -> &'static str {
    "OK"
}
