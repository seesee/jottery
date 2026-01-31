// Admin stats and user account API integration tests
//
// Tests for:
// - GET /api/v1/admin/stats
// - POST /api/v1/user/login
// - GET /api/v1/user/account
// - DELETE /api/v1/user/notes

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
    serde_json::from_slice(&bytes).unwrap()
}

// Helper to create an admin user and session for stats endpoint
async fn create_admin_user_and_session(pool: &SqlitePool) -> String {
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

    // Create session
    let session_token = uuid::Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", sha2::Sha256::digest(session_token.as_bytes()));
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    let session_id = uuid::Uuid::new_v4().to_string();
    let expires_at_str = expires_at.to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        session_id,
        user_id,
        token_hash,
        now,
        expires_at_str,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create session");

    session_token
}

// Helper to create a regular user for login tests
async fn create_test_user(pool: &SqlitePool, email: &str, password: &str, approved: bool) -> String {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password_with_params(password, 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let approved_value = if approved { 1 } else { 0 };

    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_admin, is_active, created_at)
           VALUES (?, ?, ?, ?, 0, 1, ?)"#,
        user_id,
        email,
        password_hash,
        approved_value,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create user");

    user_id
}

// Helper to create a client for a user
async fn create_client_for_user(pool: &SqlitePool, user_id: &str) -> String {
    let client_id = uuid::Uuid::new_v4().to_string();
    let api_key = format!("{:x}", sha2::Sha256::digest(uuid::Uuid::new_v4().to_string().as_bytes()));
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
           VALUES (?, ?, ?, ?, ?, ?, ?, 1)"#,
        client_id,
        user_id,
        api_key,
        "Test Device",
        "cli",
        now,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create client");

    client_id
}

// Helper to create notes for a user
async fn create_note_for_user(pool: &SqlitePool, user_id: &str, client_id: &str) {
    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO notes (id, user_id, client_id, created_at, modified_at, server_modified_at, content, tags, pinned, deleted, version, server_version)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 1, 1)"#,
        note_id,
        user_id,
        client_id,
        now,
        now,
        now,
        "Test note content",
        "[]"
    )
    .execute(pool)
    .await
    .expect("Failed to create note");
}

// ========== Admin Stats Tests ==========

#[tokio::test]
async fn test_admin_stats_requires_authentication() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/admin/stats", axum::routing::get(jottery_server::api::admin::stats::get_stats))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::admin::admin_auth_middleware,
        ))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/api/v1/admin/stats")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

#[tokio::test]
async fn test_admin_stats_success() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/admin/stats", axum::routing::get(jottery_server::api::admin::stats::get_stats))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::admin::admin_auth_middleware,
        ))
        .with_state(app_state);

    let session_token = create_admin_user_and_session(&pool).await;

    // Create some test data
    let user_id = create_test_user(&pool, "user@example.com", "password123", true).await;
    let client_id = create_client_for_user(&pool, &user_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;

    let request = Request::builder()
        .uri("/api/v1/admin/stats")
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;

    assert!(body["users"].is_object());
    assert!(body["users"]["total"].as_i64().unwrap() >= 2); // admin + user + default admin
    assert!(body["devices"].is_object());
    assert!(body["notes"].is_object());
    assert_eq!(body["notes"]["total"], 2);
    assert!(body["storage"].is_object());

    pool.close().await;
}

// ========== User Login Tests ==========

#[tokio::test]
async fn test_user_login_success() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/login", axum::routing::post(jottery_server::api::user::login))
        .with_state(app_state);

    let email = "testuser@example.com";
    let password = "testpassword123";
    create_test_user(&pool, email, password, true).await;

    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": email,
                "password": password
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["sessionId"].is_string());
    assert!(body["expiresAt"].is_string());
    assert_eq!(body["user"]["email"], email);
    assert_eq!(body["user"]["isAdmin"], false);

    pool.close().await;
}

#[tokio::test]
async fn test_user_login_wrong_password() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/login", axum::routing::post(jottery_server::api::user::login))
        .with_state(app_state);

    let email = "testuser@example.com";
    create_test_user(&pool, email, "correctpassword", true).await;

    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": email,
                "password": "wrongpassword"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

#[tokio::test]
async fn test_user_login_nonexistent_user() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/login", axum::routing::post(jottery_server::api::user::login))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "nonexistent@example.com",
                "password": "password123"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

// ========== User Account Info Tests ==========

#[tokio::test]
async fn test_get_account_info_success() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/account", axum::routing::get(jottery_server::api::user::get_account_info))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::user_auth_middleware,
        ))
        .with_state(app_state);

    let email = "testuser@example.com";
    let user_id = create_test_user(&pool, email, "password123", true).await;

    // Create session manually
    use jottery_server::db::SessionRepository;
    use jottery_server::models::CreateSessionParams;

    let session_token = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

    SessionRepository::create(
        &pool,
        CreateSessionParams {
            user_id: user_id.clone(),
            token: session_token.clone(),
            expires_at: expires_at.to_rfc3339(),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .expect("Failed to create session");

    // Create some notes for this user
    let client_id = create_client_for_user(&pool, &user_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;

    let request = Request::builder()
        .uri("/api/v1/user/account")
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["email"], email);
    assert_eq!(body["noteCount"], 2);
    assert_eq!(body["attachmentCount"], 0);
    assert_eq!(body["storageUsedBytes"], 0);
    assert!(body["storageQuotaMb"].as_i64().unwrap() > 0);

    pool.close().await;
}

#[tokio::test]
async fn test_get_account_info_requires_authentication() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/account", axum::routing::get(jottery_server::api::user::get_account_info))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::user_auth_middleware,
        ))
        .with_state(app_state);

    let request = Request::builder()
        .uri("/api/v1/user/account")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

// ========== Delete All Notes Tests ==========

#[tokio::test]
async fn test_delete_all_notes_success() {
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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        config,
    });

    let app = axum::Router::new()
        .route("/api/v1/user/notes", axum::routing::delete(jottery_server::api::user::delete_all_notes))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::user_auth_middleware,
        ))
        .with_state(app_state);

    let email = "testuser@example.com";
    let user_id = create_test_user(&pool, email, "password123", true).await;

    // Create session
    use jottery_server::db::SessionRepository;
    use jottery_server::models::CreateSessionParams;

    let session_token = uuid::Uuid::new_v4().to_string();
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);

    SessionRepository::create(
        &pool,
        CreateSessionParams {
            user_id: user_id.clone(),
            token: session_token.clone(),
            expires_at: expires_at.to_rfc3339(),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .expect("Failed to create session");

    // Create notes
    let client_id = create_client_for_user(&pool, &user_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;
    create_note_for_user(&pool, &user_id, &client_id).await;

    // Verify notes exist
    let count_before = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notes WHERE user_id = ?"
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_before, 3);

    // Delete all notes
    let request = Request::builder()
        .uri("/api/v1/user/notes")
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify notes are deleted
    let count_after = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notes WHERE user_id = ?"
    )
    .bind(&user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_after, 0);

    pool.close().await;
}
