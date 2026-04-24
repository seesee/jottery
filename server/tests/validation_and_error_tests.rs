// Validation and error handling integration tests
//
// Tests for:
// - Malformed JSON requests
// - Missing required fields
// - Invalid data types
// - Invalid UUIDs
// - Payload size limit enforcement
// - Edge cases and boundary conditions

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::Digest;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use std::sync::Arc;
use tower::ServiceExt;

// Helper to create test database with migrations
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

// Helper to parse JSON response
async fn parse_json_response(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!({}))
}

// Helper to create a test user and device, returns API key
async fn create_test_user_and_device(pool: &SqlitePool) -> String {
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

    let device_id = uuid::Uuid::new_v4().to_string();
    let api_key = format!("{:x}", uuid::Uuid::new_v4());
    let api_key_hash = format!("{:x}", sha2::Sha256::digest(api_key.as_bytes()));

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

// ========== Malformed JSON Tests ==========

#[tokio::test]
async fn test_malformed_json_sync_push() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;

    // Send invalid JSON (missing closing brace)
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(r#"{"notes": [{"id": "test""#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Minimal router returns 400 BAD_REQUEST for malformed JSON
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    pool.close().await;
}

#[tokio::test]
async fn test_malformed_json_user_login() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/login", axum::routing::post(jottery_server::api::user::login))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email": "test@example.com", "password": "#))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Minimal router returns 400 BAD_REQUEST for malformed JSON
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    pool.close().await;
}

// ========== Missing Required Fields Tests ==========

#[tokio::test]
async fn test_sync_push_missing_notes_field() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;

    // Missing "notes" field
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

#[tokio::test]
async fn test_sync_push_note_missing_required_fields() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;

    // Note missing "content" field
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": uuid::Uuid::new_v4().to_string(),
                        "createdAt": chrono::Utc::now().to_rfc3339(),
                        "modifiedAt": chrono::Utc::now().to_rfc3339()
                        // Missing: content, tags, attachments, deleted, version, etc.
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

#[tokio::test]
async fn test_register_user_missing_email() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/auth/register-user", axum::routing::post(jottery_server::api::auth::register_user))
        .with_state(app_state);

    // Missing email field
    let request = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "password": "testpassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

#[tokio::test]
async fn test_login_missing_password() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/login", axum::routing::post(jottery_server::api::user::login))
        .with_state(app_state);

    // Missing password field
    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "test@example.com"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

// ========== Invalid Data Type Tests ==========

#[tokio::test]
async fn test_sync_push_wrong_data_type() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;

    // "notes" should be array but is a string
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": "not an array",
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

#[tokio::test]
async fn test_sync_push_version_wrong_type() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();

    // Version should be number but is string
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": uuid::Uuid::new_v4().to_string(),
                        "content": "Test",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": "not a number",
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

#[tokio::test]
async fn test_register_device_wrong_type() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/auth/register-device", axum::routing::post(jottery_server::api::auth::register_device))
        .with_state(app_state);

    // deviceType should be string but is number
    let request = Request::builder()
        .uri("/api/v1/auth/register-device")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "test@example.com",
                "password": "password123",
                "deviceName": "Test Device",
                "deviceType": 123
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Axum returns 422 UNPROCESSABLE_ENTITY for JSON deserialization errors
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    pool.close().await;
}

// ========== Invalid UUID Tests ==========

#[tokio::test]
async fn test_get_user_invalid_uuid() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    // Create admin session
    use jottery_server::utils::password::hash_password_with_params;

    let admin_id = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password_with_params("admin123", 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_admin, is_active, created_at)
           VALUES (?, ?, ?, 1, 1, 1, ?)"#,
        admin_id,
        "admin@test.com",
        password_hash,
        now
    )
    .execute(&pool)
    .await
    .unwrap();

    let session_token = uuid::Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", sha2::Sha256::digest(session_token.as_bytes()));
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();
    let session_id = uuid::Uuid::new_v4().to_string();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        session_id,
        admin_id,
        token_hash,
        now,
        expires_at,
        now
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = axum::Router::new()
        .route("/api/v1/admin/users/:id", axum::routing::get(jottery_server::api::admin::users::get_user))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::admin::admin_auth_middleware,
        ))
        .with_state(app_state);

    // Use invalid UUID format - the endpoint will accept it but fail to find it
    let request = Request::builder()
        .uri("/api/v1/admin/users/not-a-valid-uuid")
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should return NOT_FOUND since the UUID won't match any user
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    pool.close().await;
}

#[tokio::test]
async fn test_empty_string_fields() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/auth/register-user", axum::routing::post(jottery_server::api::auth::register_user))
        .with_state(app_state);

    // Empty email string
    let request = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "",
                "password": "testpassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should reject empty email
    assert!(response.status().is_client_error());

    pool.close().await;
}

// ========== Very Large Payload Tests ==========

#[tokio::test]
async fn test_extremely_large_note_content() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();

    // Create a very large note (2MB of content)
    let large_content = "A".repeat(2 * 1024 * 1024);

    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": uuid::Uuid::new_v4().to_string(),
                        "content": large_content,
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should reject content exceeding payload limit
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    pool.close().await;
}

#[tokio::test]
async fn test_many_tags() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();

    // Create note with many tags
    let tags: Vec<String> = (0..100).map(|i| format!("tag{}", i)).collect();

    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": uuid::Uuid::new_v4().to_string(),
                        "content": "Note with many tags",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": tags,
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should accept many tags
    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["accepted"].as_array().unwrap().len(), 1);

    pool.close().await;
}

#[tokio::test]
async fn test_special_characters_in_content() {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();

    // Content with emojis, unicode, special chars
    let special_content = "Hello 👋 世界 \n\t\r Special: <>&\"' {} [] \\";

    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": uuid::Uuid::new_v4().to_string(),
                        "content": special_content,
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": ["emoji-tag-🎉"],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    pool.close().await;
}

// ========== Graceful FK Error Handling Tests ==========

#[tokio::test]
async fn test_sync_push_skips_versions_for_rejected_notes() {
    // Test that when a note is rejected (conflict), versions for that note are gracefully skipped
    // This prevents FK constraint errors when pushing versions for notes that weren't accepted
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;

    // Get the user_id for this device
    let api_key_hash = format!("{:x}", sha2::Sha256::digest(api_key.as_bytes()));
    let device = sqlx::query!(
        "SELECT id, user_id FROM clients WHERE api_key = ?",
        api_key_hash
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to get device");

    let note_id = uuid::Uuid::new_v4().to_string();
    let server_time = chrono::Utc::now().to_rfc3339();

    // First, create a note on the server with a NEWER timestamp
    sqlx::query!(
        r#"INSERT INTO notes (id, user_id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, deleted, version, server_version)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 2, 2)"#,
        note_id,
        device.user_id,
        device.id,
        server_time,
        server_time,  // Server has newer timestamp
        server_time,
        "Server content",
        "[]"
    )
    .execute(&pool)
    .await
    .expect("Failed to create server note");

    // Now push the same note with an OLDER timestamp (will be rejected)
    // Also include a version for this note (would cause FK error if not handled gracefully)
    let older_time = "2020-01-01T00:00:00Z";
    let version_key = format!("{}:1", note_id);

    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Client content (older)",
                        "createdAt": older_time,
                        "modifiedAt": older_time,  // Older than server
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": [
                    {
                        "versionKey": version_key,
                        "noteId": note_id,
                        "version": 1,
                        "createdAt": older_time,
                        "syncedAt": older_time,
                        "content": "Version content",
                        "tags": [],
                        "attachments": [],
                        "syntaxLanguage": null,
                        "wordWrap": null,
                        "reason": "sync"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Push should still succeed (200 OK), even though note was rejected
    assert_eq!(response.status(), StatusCode::OK);

    // Parse response to verify note was rejected
    let body = parse_json_response(response.into_body()).await;
    assert!(body["rejected"].as_array().unwrap().len() > 0, "Note should be rejected due to conflict");
    assert_eq!(body["accepted"].as_array().unwrap().len(), 0, "No notes should be accepted");

    // Verify version was NOT inserted (because note was rejected)
    let version_count: i32 = sqlx::query_scalar!(
        "SELECT COUNT(*) as count FROM note_versions WHERE version_key = ?",
        version_key
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count versions") as i32;

    assert_eq!(version_count, 0, "Version should not be inserted for rejected note");

    pool.close().await;
}

#[tokio::test]
async fn test_sync_push_handles_orphan_attachment_data_gracefully() {
    // Test that attachment data for non-existent metadata is gracefully skipped
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
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
            auth_rate_limit_period_seconds: 2,
            auth_rate_limit_burst: 5,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    let api_key = create_test_user_and_device(&pool).await;
    let orphan_attachment_id = uuid::Uuid::new_v4().to_string();

    // Push attachment data without corresponding metadata (simulates incomplete sync)
    // Note: We're sending attachment data but the note referencing it doesn't exist
    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [],
                "attachments": [
                    {
                        "id": orphan_attachment_id,
                        "data": "SGVsbG8gV29ybGQ="  // Base64 of "Hello World"
                    }
                ],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Push should succeed despite orphan attachment (graceful handling)
    assert_eq!(response.status(), StatusCode::OK);

    // Verify attachment data was NOT inserted (no corresponding metadata)
    let attachment_count: i32 = sqlx::query_scalar!(
        "SELECT COUNT(*) as count FROM attachments_data WHERE id = ?",
        orphan_attachment_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count attachments") as i32;

    assert_eq!(attachment_count, 0, "Orphan attachment data should not be inserted");

    pool.close().await;
}
