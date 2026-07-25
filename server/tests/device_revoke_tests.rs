// Device self-revoke endpoint tests
//
// DELETE /api/v1/sync/device (device API key auth)
//
// Lets a client unlink itself from the server using only its own API key, so
// disconnecting sync on a device does not require the user to re-enter their
// account password. Backs Settings -> Sync -> Disconnect in the mobile clients.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::{delete, get},
    Router,
};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower::ServiceExt;

// ============================================================================
// Helpers
// ============================================================================

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

fn test_config() -> jottery_server::config::Config {
    jottery_server::config::Config {
        database_url: "sqlite::memory:".to_string(),
        port: 3030,
        max_payload_size: 5_242_880,
        cors_allowed_origins: None,
        session_expiry_days: 7,
        default_admin_email: "admin@localhost".to_string(),
        default_admin_password: "changeme".to_string(),
        argon2_m_cost: 19456,
        argon2_t_cost: 2,
        argon2_p_cost: 1,
        default_storage_quota_mb: 1000,
        default_max_upload_size_mb: 5,
        default_inbox_max_items: 100,
        default_inbox_max_size_mb: 10,
        password_complexity: "none".to_string(),
        enable_hsts: false,
        max_device_name_length: 255,
        max_inbox_content_size: 1_048_576,
        max_note_content_size: 10_485_760,
        max_tag_length: 100,
        max_tags_per_note: 50,
    }
}

async fn create_test_app() -> (Router, SqlitePool) {
    let pool = setup_test_db().await;

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config: test_config(),
    });

    // Sync routes (device API key auth) — mirrors main.rs
    let sync_routes = Router::new()
        .route(
            "/api/v1/sync/status",
            get(jottery_server::api::sync::get_status),
        )
        .route(
            "/api/v1/sync/device",
            delete(jottery_server::api::sync::revoke_self),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ));

    let app = Router::new().merge(sync_routes).with_state(app_state);

    (app, pool)
}

async fn create_test_user(pool: &SqlitePool) -> String {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let email = format!("testuser{}@example.com", uuid::Uuid::new_v4());
    // Randomised rather than a literal: these tests authenticate with device API
    // keys and never use the password, and a hard-coded one trips CodeQL's
    // rust/hard-coded-cryptographic-value rule.
    let password = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password_with_params(&password, 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_active, created_at)
           VALUES (?, ?, ?, 1, 1, ?)"#,
        user_id,
        email,
        password_hash,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create test user");

    user_id
}

/// Create a device for a user, returning (device_id, plaintext API key)
async fn create_test_device(pool: &SqlitePool, user_id: &str) -> (String, String) {
    let device_id = uuid::Uuid::new_v4().to_string();
    let api_key = format!("{:x}", uuid::Uuid::new_v4());
    let api_key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        device_id,
        user_id,
        api_key_hash,
        "Test Device",
        "ios",
        now,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create test device");

    (device_id, api_key)
}

async fn is_active(pool: &SqlitePool, device_id: &str) -> i64 {
    sqlx::query!("SELECT is_active FROM clients WHERE id = ?", device_id)
        .fetch_one(pool)
        .await
        .expect("Device row missing")
        .is_active
        .unwrap_or(0)
}

fn revoke_request(api_key: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri("/api/v1/sync/device")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap()
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn revoke_self_deactivates_calling_device() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let (device_id, api_key) = create_test_device(&pool, &user_id).await;

    assert_eq!(is_active(&pool, &device_id).await, 1, "device starts active");

    let response = app.oneshot(revoke_request(&api_key)).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        is_active(&pool, &device_id).await,
        0,
        "device should be deactivated after self-revoke"
    );
}

/// The device row is kept, not deleted, so it still shows in the user's device
/// list and can be hard-deleted from the web UI.
#[tokio::test]
async fn revoke_self_soft_revokes_rather_than_deleting() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let (device_id, api_key) = create_test_device(&pool, &user_id).await;

    let response = app.oneshot(revoke_request(&api_key)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let row = sqlx::query!("SELECT COUNT(*) as count FROM clients WHERE id = ?", device_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.count, 1, "device row should still exist");
}

/// After revoking, the API key must stop working — otherwise "Disconnect" would
/// leave a live credential on the server.
#[tokio::test]
async fn revoked_api_key_no_longer_authenticates() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let (_device_id, api_key) = create_test_device(&pool, &user_id).await;

    let revoke = app
        .clone()
        .oneshot(revoke_request(&api_key))
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);

    let followup = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/sync/status")
                .header("Authorization", format!("Bearer {}", api_key))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        followup.status(),
        StatusCode::UNAUTHORIZED,
        "revoked key must not authenticate"
    );
}

#[tokio::test]
async fn revoke_self_requires_authentication() {
    let (app, _pool) = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/sync/device")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn revoke_self_rejects_invalid_key() {
    let (app, _pool) = create_test_app().await;

    let response = app.oneshot(revoke_request("not-a-real-key")).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// Revoking one device must not disturb the user's other devices.
#[tokio::test]
async fn revoke_self_leaves_other_devices_active() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let (first_id, first_key) = create_test_device(&pool, &user_id).await;
    let (second_id, _second_key) = create_test_device(&pool, &user_id).await;

    let response = app.oneshot(revoke_request(&first_key)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    assert_eq!(is_active(&pool, &first_id).await, 0, "caller revoked");
    assert_eq!(
        is_active(&pool, &second_id).await,
        1,
        "sibling device must stay active"
    );
}
