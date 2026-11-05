use std::future::IntoFuture;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

use jottery_server::api::sse::{create_token_store, SyncNotification};
use jottery_server::app::build_app;
use jottery_server::config::Config;
use jottery_server::rate_limit::LoginThrottle;
use jottery_server::shutdown::{self, Shutdown};
use jottery_server::{db, AppState};

/// How long requests in flight get to finish once the server is asked to stop.
/// Inside `docker stop`'s ten seconds, so a slow client can't get us killed
/// part-way through closing the database.
const DRAIN_LIMIT: Duration = Duration::from_secs(8);

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
    db::run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Database migrations complete");

    // Never start with the published default admin password
    jottery_server::bootstrap::ensure_default_admin(&pool, &config)
        .await
        .expect("Failed to prepare the admin account");

    // Housekeeping runs in the background, not during requests
    jottery_server::maintenance::spawn(pool.clone());

    // Create broadcast channel for SSE sync notifications
    // Capacity of 100 is sufficient - notifications are small and ephemeral
    let (sync_broadcast, _) = broadcast::channel::<SyncNotification>(100);

    // Create SSE token store for short-lived tokens
    let sse_tokens = create_token_store();

    let shutdown = Shutdown::default();

    // Build application state
    let app_state = Arc::new(AppState {
        pool: pool.clone(),
        config: config.clone(),
        sync_broadcast,
        sse_tokens,
        login_throttle: Arc::new(LoginThrottle::new(config.rate_limit.enabled)),
        shutdown: shutdown.clone(),
    });

    let app = build_app(app_state);

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

    // Rate limiting needs the peer address of each connection
    let server = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown({
            let shutdown = shutdown.clone();
            async move {
                shutdown::signal().await;
                tracing::info!("Stopping: finishing requests in flight");
                shutdown.trigger();
            }
        });

    let drain_limit = async {
        shutdown.triggered().await;
        tokio::time::sleep(DRAIN_LIMIT).await;
    };

    tokio::select! {
        result = server.into_future() => result.expect("Server failed"),
        _ = drain_limit => tracing::warn!("Stopping with requests still in flight"),
    }

    // Closes every connection, so nothing is left for the next start to recover
    pool.close().await;
    tracing::info!("Stopped");
}
