// Inbox API endpoint integration tests
//
// Tests for inbox endpoints:
// - POST /api/v1/inbox (inbox token auth)
// - GET /api/v1/inbox (device API key auth)
// - DELETE /api/v1/inbox/:id (device API key auth)
// - DELETE /api/v1/inbox (device API key auth)
// - GET /api/v1/inbox/status (device API key auth)
// - POST /api/v1/user/inbox-token (session auth)
// - DELETE /api/v1/user/inbox-token (session auth)
// - GET /api/v1/user/inbox-token/status (session auth)
// - Sync pull inbox integration

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower::{Service, ServiceExt};

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

/// Create test app with inbox, sync, and user routes
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

    // Inbox submission routes (inbox token auth)
    let inbox_submit_routes = Router::new()
        .route("/api/v1/inbox", post(jottery_server::api::inbox::create_item))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::inbox_auth_middleware,
        ));

    // Inbox management routes (device API key auth)
    let inbox_manage_routes = Router::new()
        .route(
            "/api/v1/inbox",
            get(jottery_server::api::inbox::list_items)
                .delete(jottery_server::api::inbox::delete_all),
        )
        .route(
            "/api/v1/inbox/:id",
            delete(jottery_server::api::inbox::delete_item),
        )
        .route(
            "/api/v1/inbox/status",
            get(jottery_server::api::inbox::get_status),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ));

    // Sync routes (device API key auth)
    let sync_routes = Router::new()
        .route(
            "/api/v1/sync/pull",
            post(jottery_server::api::sync::pull),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ));

    // User routes (session auth) — for inbox token management
    let user_routes = Router::new()
        .route(
            "/api/v1/user/inbox-token",
            post(jottery_server::api::user::generate_inbox_token)
                .delete(jottery_server::api::user::revoke_inbox_token),
        )
        .route(
            "/api/v1/user/inbox-token/status",
            get(jottery_server::api::user::get_inbox_token_status),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::user_auth_middleware,
        ));

    // Admin routes (for quota management)
    let admin_routes = Router::new()
        .route(
            "/api/v1/admin/users/:id/settings",
            patch(jottery_server::api::admin::users::update_user_settings),
        )
        .route(
            "/api/v1/admin/users/:id",
            get(jottery_server::api::admin::users::get_user),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::admin::admin_auth_middleware,
        ));

    let app = Router::new()
        .merge(inbox_submit_routes)
        .merge(inbox_manage_routes)
        .merge(sync_routes)
        .merge(user_routes)
        .merge(admin_routes)
        .with_state(app_state);

    (app, pool)
}

async fn parse_json_response(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let text = String::from_utf8_lossy(&bytes);
        json!({"error": text.to_string()})
    })
}

/// Create an approved user, returning user_id
async fn create_test_user(pool: &SqlitePool) -> String {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let email = format!("testuser{}@example.com", uuid::Uuid::new_v4());
    let password_hash = hash_password_with_params("testpassword123", 19456, 2, 1).unwrap();
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

/// Create a device for a user, returning the plaintext API key
async fn create_test_device(pool: &SqlitePool, user_id: &str) -> String {
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
        "cli",
        now,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create test device");

    api_key
}

/// Create a user + device, returning the plaintext API key
async fn create_test_user_and_device(pool: &SqlitePool) -> (String, String) {
    let user_id = create_test_user(pool).await;
    let api_key = create_test_device(pool, &user_id).await;
    (user_id, api_key)
}

/// Generate an inbox token for a user directly in the database.
/// Returns the plaintext token.
async fn set_inbox_token(pool: &SqlitePool, user_id: &str) -> String {
    let token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));

    sqlx::query!(
        "UPDATE users SET inbox_token_hash = ? WHERE id = ?",
        token_hash,
        user_id
    )
    .execute(pool)
    .await
    .expect("Failed to set inbox token");

    token
}

/// Create a session for a user, returning the plaintext session token
async fn create_session(pool: &SqlitePool, user_id: &str) -> String {
    let session_token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();
    let session_id = uuid::Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", Sha256::digest(session_token.as_bytes()));
    let now = chrono::Utc::now().to_rfc3339();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        session_id,
        user_id,
        token_hash,
        expires_at,
        now,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create session");

    session_token
}

/// Create an admin user with session, returning (user_id, session_token)
async fn create_admin_user_and_session(pool: &SqlitePool) -> (String, String) {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let email = format!("admin{}@example.com", uuid::Uuid::new_v4());
    let password_hash = hash_password_with_params("adminpassword123", 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_admin, is_active, created_at)
           VALUES (?, ?, ?, 1, 1, 1, ?)"#,
        user_id,
        email,
        password_hash,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create admin user");

    let session_token = create_session(pool, &user_id).await;
    (user_id, session_token)
}

/// Insert an inbox item directly into the database
async fn insert_inbox_item(
    pool: &SqlitePool,
    user_id: &str,
    content: &str,
    tags: &[&str],
    source: Option<&str>,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let tags_json = serde_json::to_string(&tags).unwrap();
    let created_at = chrono::Utc::now().to_rfc3339();
    let size_bytes = (content.len() + tags_json.len()) as i64;

    sqlx::query!(
        r#"INSERT INTO inbox_items (id, user_id, content, tags, created_at, source, size_bytes)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id,
        user_id,
        content,
        tags_json,
        created_at,
        source,
        size_bytes
    )
    .execute(pool)
    .await
    .expect("Failed to insert inbox item");

    id
}

// ============================================================================
// Inbox Token Management Tests
// ============================================================================

#[tokio::test]
async fn test_generate_inbox_token() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let session_token = create_session(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/user/inbox-token")
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = parse_json_response(response.into_body()).await;
    let token = body["token"].as_str().unwrap();
    assert_eq!(token.len(), 64); // 32 bytes as hex

    pool.close().await;
}

#[tokio::test]
async fn test_generate_inbox_token_replaces_existing() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let session_token = create_session(&pool, &user_id).await;

    // Generate first token
    let request1 = Request::builder()
        .uri("/api/v1/user/inbox-token")
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response1 = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(request1)
        .await
        .unwrap();
    assert_eq!(response1.status(), StatusCode::CREATED);
    let body1 = parse_json_response(response1.into_body()).await;
    let token1 = body1["token"].as_str().unwrap().to_string();

    // Generate second token
    let request2 = Request::builder()
        .uri("/api/v1/user/inbox-token")
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::CREATED);
    let body2 = parse_json_response(response2.into_body()).await;
    let token2 = body2["token"].as_str().unwrap().to_string();

    // Tokens should be different
    assert_ne!(token1, token2);

    // Old token should no longer work (verify by checking hash in DB)
    let old_hash = format!("{:x}", Sha256::digest(token1.as_bytes()));
    let result = sqlx::query!(
        "SELECT id FROM users WHERE inbox_token_hash = ?",
        old_hash
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(result.is_none(), "Old token hash should no longer be in DB");

    pool.close().await;
}

#[tokio::test]
async fn test_revoke_inbox_token() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let session_token = create_session(&pool, &user_id).await;

    // First generate a token
    set_inbox_token(&pool, &user_id).await;

    // Revoke it
    let request = Request::builder()
        .uri("/api/v1/user/inbox-token")
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify token is removed from DB
    let result = sqlx::query!(
        "SELECT inbox_token_hash FROM users WHERE id = ?",
        user_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(result.inbox_token_hash.is_none());

    pool.close().await;
}

#[tokio::test]
async fn test_inbox_token_status_has_token() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let session_token = create_session(&pool, &user_id).await;

    // Set a token
    set_inbox_token(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/user/inbox-token/status")
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["hasToken"], true);

    pool.close().await;
}

#[tokio::test]
async fn test_inbox_token_status_no_token() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let session_token = create_session(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/user/inbox-token/status")
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["hasToken"], false);

    pool.close().await;
}

#[tokio::test]
async fn test_inbox_token_requires_session_auth() {
    let (app, _pool) = create_test_app().await;

    // Try without any auth
    let request = Request::builder()
        .uri("/api/v1/user/inbox-token")
        .method("POST")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Inbox Submission Tests (POST /api/v1/inbox with inbox token auth)
// ============================================================================

#[tokio::test]
async fn test_create_inbox_item_success() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({
                "content": "My inbox note"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["id"].is_string());
    assert!(body["createdAt"].is_string());

    // Verify stored in database
    let item_id = body["id"].as_str().unwrap();
    let row = sqlx::query!(
        "SELECT content, user_id FROM inbox_items WHERE id = ?",
        item_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.content, "My inbox note");
    assert_eq!(row.user_id, user_id);

    pool.close().await;
}

#[tokio::test]
async fn test_create_inbox_item_with_tags() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({
                "content": "Tagged note",
                "tags": ["inbox", "important"]
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = parse_json_response(response.into_body()).await;
    let item_id = body["id"].as_str().unwrap();

    let row = sqlx::query!(
        "SELECT tags FROM inbox_items WHERE id = ?",
        item_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let tags: Vec<String> = serde_json::from_str(&row.tags).unwrap();
    assert_eq!(tags, vec!["inbox", "important"]);

    pool.close().await;
}

#[tokio::test]
async fn test_create_inbox_item_with_source() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({
                "content": "From a webhook",
                "source": "github-webhook"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = parse_json_response(response.into_body()).await;
    let item_id = body["id"].as_str().unwrap();

    let row = sqlx::query!(
        "SELECT source FROM inbox_items WHERE id = ?",
        item_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.source.as_deref(), Some("github-webhook"));

    pool.close().await;
}

#[tokio::test]
async fn test_create_inbox_item_invalid_token() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", "Bearer invalid_token_here")
        .body(Body::from(
            json!({ "content": "Should fail" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_inbox_item_revoked_token() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    // Revoke the token
    sqlx::query!(
        "UPDATE users SET inbox_token_hash = NULL WHERE id = ?",
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({ "content": "Should fail" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_inbox_item_inactive_user() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    // Deactivate the user
    sqlx::query!(
        "UPDATE users SET is_active = 0 WHERE id = ?",
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({ "content": "Should fail" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_inbox_item_empty_content() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({ "content": "   " }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_create_inbox_item_quota_count_exceeded() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    // Set quota to 2 items
    sqlx::query!(
        "UPDATE users SET inbox_max_items = 2 WHERE id = ?",
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert 2 items directly
    insert_inbox_item(&pool, &user_id, "Item 1", &[], None).await;
    insert_inbox_item(&pool, &user_id, "Item 2", &[], None).await;

    // Third item should be rejected
    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({ "content": "Item 3 — over quota" }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_create_inbox_item_quota_size_exceeded() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    // Set size quota to 1 MB
    sqlx::query!(
        "UPDATE users SET inbox_max_size_mb = 1 WHERE id = ?",
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert an item close to the limit (just under 1 MB)
    let large_content = "x".repeat(1024 * 1024 - 100);
    insert_inbox_item(&pool, &user_id, &large_content, &[], None).await;

    // Next item should push over the limit
    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({ "content": "x".repeat(200) }).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_create_inbox_item_calculates_size() {
    let (app, pool) = create_test_app().await;
    let user_id = create_test_user(&pool).await;
    let inbox_token = set_inbox_token(&pool, &user_id).await;

    let content = "Hello, world!";
    let tags = vec!["tag1", "tag2"];

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", inbox_token))
        .body(Body::from(
            json!({
                "content": content,
                "tags": tags
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let body = parse_json_response(response.into_body()).await;
    let item_id = body["id"].as_str().unwrap();

    let row = sqlx::query!(
        "SELECT size_bytes FROM inbox_items WHERE id = ?",
        item_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // size = content.len() + serialised tags JSON len
    let expected_tags_json = serde_json::to_string(&tags).unwrap();
    let expected_size = content.len() + expected_tags_json.len();
    assert_eq!(row.size_bytes, expected_size as i64);

    pool.close().await;
}

// ============================================================================
// Inbox Management Tests (device API key auth)
// ============================================================================

#[tokio::test]
async fn test_list_inbox_items() {
    let (app, pool) = create_test_app().await;
    let (user_id, api_key) = create_test_user_and_device(&pool).await;

    // Insert some items
    let id1 = insert_inbox_item(&pool, &user_id, "First item", &["tag1"], Some("curl")).await;
    let id2 = insert_inbox_item(&pool, &user_id, "Second item", &[], None).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("GET")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2);

    // Items are ordered by created_at DESC so second item comes first
    // (both were created at nearly the same time, so check by IDs)
    let item_ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    assert!(item_ids.contains(&id1.as_str()));
    assert!(item_ids.contains(&id2.as_str()));

    // Check structure of first item in response
    let item = items.iter().find(|i| i["id"].as_str().unwrap() == id1).unwrap();
    assert_eq!(item["content"], "First item");
    assert_eq!(item["tags"].as_array().unwrap(), &[json!("tag1")]);
    assert_eq!(item["source"], "curl");
    assert!(item["createdAt"].is_string());
    assert!(item["sizeBytes"].is_number());

    pool.close().await;
}

#[tokio::test]
async fn test_list_inbox_items_empty() {
    let (app, pool) = create_test_app().await;
    let (_user_id, api_key) = create_test_user_and_device(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("GET")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body.as_array().unwrap().len(), 0);

    pool.close().await;
}

#[tokio::test]
async fn test_list_inbox_items_user_isolation() {
    let (app, pool) = create_test_app().await;

    // Create two users with items
    let (user_id1, api_key1) = create_test_user_and_device(&pool).await;
    let (user_id2, api_key2) = create_test_user_and_device(&pool).await;

    insert_inbox_item(&pool, &user_id1, "User 1's item", &[], None).await;
    insert_inbox_item(&pool, &user_id2, "User 2's item", &[], None).await;

    // User 1 should only see their own items
    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("GET")
        .header("Authorization", format!("Bearer {}", api_key1))
        .body(Body::empty())
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["content"], "User 1's item");

    // User 2 should only see their own items
    let request2 = Request::builder()
        .uri("/api/v1/inbox")
        .method("GET")
        .header("Authorization", format!("Bearer {}", api_key2))
        .body(Body::empty())
        .unwrap();

    let response2 = app.oneshot(request2).await.unwrap();
    let body2 = parse_json_response(response2.into_body()).await;
    let items2 = body2.as_array().unwrap();
    assert_eq!(items2.len(), 1);
    assert_eq!(items2[0]["content"], "User 2's item");

    pool.close().await;
}

#[tokio::test]
async fn test_delete_inbox_item() {
    let (app, pool) = create_test_app().await;
    let (user_id, api_key) = create_test_user_and_device(&pool).await;

    let item_id = insert_inbox_item(&pool, &user_id, "To be deleted", &[], None).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/inbox/{}", item_id))
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify deleted from DB
    let result = sqlx::query!(
        "SELECT id FROM inbox_items WHERE id = ?",
        item_id
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(result.is_none());

    pool.close().await;
}

#[tokio::test]
async fn test_delete_inbox_item_wrong_user() {
    let (app, pool) = create_test_app().await;

    let (user_id1, _api_key1) = create_test_user_and_device(&pool).await;
    let (_user_id2, api_key2) = create_test_user_and_device(&pool).await;

    // Create item owned by user 1
    let item_id = insert_inbox_item(&pool, &user_id1, "User 1's item", &[], None).await;

    // User 2 tries to delete it
    let request = Request::builder()
        .uri(&format!("/api/v1/inbox/{}", item_id))
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", api_key2))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Item should still exist
    let result = sqlx::query!(
        "SELECT id FROM inbox_items WHERE id = ?",
        item_id
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert!(result.is_some(), "Item should not have been deleted");

    pool.close().await;
}

#[tokio::test]
async fn test_delete_inbox_item_not_found() {
    let (app, pool) = create_test_app().await;
    let (_user_id, api_key) = create_test_user_and_device(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/inbox/nonexistent-id")
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_all_inbox_items() {
    let (app, pool) = create_test_app().await;
    let (user_id, api_key) = create_test_user_and_device(&pool).await;

    // Insert several items
    insert_inbox_item(&pool, &user_id, "Item 1", &[], None).await;
    insert_inbox_item(&pool, &user_id, "Item 2", &[], None).await;
    insert_inbox_item(&pool, &user_id, "Item 3", &[], None).await;

    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify all deleted
    let count = sqlx::query!(
        "SELECT COUNT(*) as count FROM inbox_items WHERE user_id = ?",
        user_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count.count, 0);

    pool.close().await;
}

#[tokio::test]
async fn test_delete_all_preserves_other_users() {
    let (app, pool) = create_test_app().await;

    let (user_id1, api_key1) = create_test_user_and_device(&pool).await;
    let (user_id2, _api_key2) = create_test_user_and_device(&pool).await;

    insert_inbox_item(&pool, &user_id1, "User 1 item", &[], None).await;
    insert_inbox_item(&pool, &user_id2, "User 2 item", &[], None).await;

    // User 1 deletes all their items
    let request = Request::builder()
        .uri("/api/v1/inbox")
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", api_key1))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // User 1's items should be gone
    let count1 = sqlx::query!(
        "SELECT COUNT(*) as count FROM inbox_items WHERE user_id = ?",
        user_id1
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count1.count, 0);

    // User 2's items should still exist
    let count2 = sqlx::query!(
        "SELECT COUNT(*) as count FROM inbox_items WHERE user_id = ?",
        user_id2
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count2.count, 1);

    pool.close().await;
}

#[tokio::test]
async fn test_inbox_status() {
    let (app, pool) = create_test_app().await;
    let (user_id, api_key) = create_test_user_and_device(&pool).await;

    // Set quota
    sqlx::query!(
        "UPDATE users SET inbox_max_items = 50, inbox_max_size_mb = 5 WHERE id = ?",
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert items
    insert_inbox_item(&pool, &user_id, "Item 1", &["tag1"], None).await;
    insert_inbox_item(&pool, &user_id, "Item 2", &[], None).await;

    let request = Request::builder()
        .uri("/api/v1/inbox/status")
        .method("GET")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["count"], 2);
    assert!(body["totalSizeBytes"].as_i64().unwrap() > 0);
    assert_eq!(body["maxItems"], 50);
    assert_eq!(body["maxSizeMb"], 5);

    pool.close().await;
}

// ============================================================================
// Sync Pull Integration Tests
// ============================================================================

#[tokio::test]
async fn test_pull_includes_inbox_items() {
    let (app, pool) = create_test_app().await;
    let (user_id, api_key) = create_test_user_and_device(&pool).await;

    // Insert an inbox item
    let item_id = insert_inbox_item(
        &pool,
        &user_id,
        "Inbox note via sync",
        &["synced"],
        Some("api"),
    )
    .await;

    let request = Request::builder()
        .uri("/api/v1/sync/pull")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "lastSyncAt": null,
                "knownNoteIds": [],
                "knownAttachmentIds": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;

    // Should include inbox items
    let inbox_items = body["inboxItems"].as_array().unwrap();
    assert_eq!(inbox_items.len(), 1);
    assert_eq!(inbox_items[0]["id"], item_id);
    assert_eq!(inbox_items[0]["content"], "Inbox note via sync");
    assert_eq!(inbox_items[0]["tags"], json!(["synced"]));
    assert_eq!(inbox_items[0]["source"], "api");

    assert_eq!(body["inboxCount"], 1);

    pool.close().await;
}

#[tokio::test]
async fn test_pull_inbox_items_empty_when_none() {
    let (app, pool) = create_test_app().await;
    let (_user_id, api_key) = create_test_user_and_device(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/sync/pull")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "lastSyncAt": null,
                "knownNoteIds": [],
                "knownAttachmentIds": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;

    // When no inbox items, fields should be absent (skip_serializing_if = "Option::is_none")
    assert!(body.get("inboxItems").is_none() || body["inboxItems"].is_null());
    assert!(body.get("inboxCount").is_none() || body["inboxCount"].is_null());

    pool.close().await;
}

// ============================================================================
// Admin Quota Management Tests
// ============================================================================

#[tokio::test]
async fn test_admin_update_inbox_quota() {
    let (app, pool) = create_test_app().await;
    let (_admin_id, admin_session) = create_admin_user_and_session(&pool).await;
    let user_id = create_test_user(&pool).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/settings", user_id))
        .method("PATCH")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", admin_session))
        .body(Body::from(
            json!({
                "inboxMaxItems": 200,
                "inboxMaxSizeMb": 20
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify in database
    let user = sqlx::query!(
        "SELECT inbox_max_items, inbox_max_size_mb FROM users WHERE id = ?",
        user_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(user.inbox_max_items, Some(200));
    assert_eq!(user.inbox_max_size_mb, Some(20));

    pool.close().await;
}

#[tokio::test]
async fn test_admin_user_detail_includes_inbox_info() {
    let (app, pool) = create_test_app().await;
    let (_admin_id, admin_session) = create_admin_user_and_session(&pool).await;
    let user_id = create_test_user(&pool).await;

    // Set inbox token and add items
    set_inbox_token(&pool, &user_id).await;
    insert_inbox_item(&pool, &user_id, "Test item", &["tag1"], None).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}", user_id))
        .method("GET")
        .header("Authorization", format!("Bearer {}", admin_session))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;

    // Should include inbox quota fields
    assert!(body["inboxMaxItems"].is_number());
    assert!(body["inboxMaxSizeMb"].is_number());

    pool.close().await;
}
