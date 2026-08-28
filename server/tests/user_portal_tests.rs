// User portal integration tests
//
// Tests for user self-service portal endpoints:
// - Change password (/api/v1/user/change-password)
// - Delete/deactivate account (/api/v1/user/account)
// - Logout (/api/v1/user/logout)
// - Get account info (/api/v1/user/account)

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::{delete, get, post},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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

// Helper to create test app with user routes
async fn create_test_app() -> (Router, SqlitePool) {
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

    // Build router with user routes (both public and protected)
    let protected_user_routes = Router::new()
        .route("/api/v1/user/account", get(jottery_server::api::user::get_account_info).delete(jottery_server::api::user::delete_account))
        .route("/api/v1/user/change-password", post(jottery_server::api::user::change_password))
        .route("/api/v1/user/logout", post(jottery_server::api::user::logout))
        .route("/api/v1/user/notes", delete(jottery_server::api::user::delete_all_notes))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::user_auth_middleware,
        ));

    let app = Router::new()
        .route("/api/v1/user/login", post(jottery_server::api::user::login))
        .route("/api/v1/auth/register-user", post(jottery_server::api::auth::register_user))
        .merge(protected_user_routes)
        .with_state(app_state);

    (app, pool)
}

// Helper to parse JSON response
async fn parse_json_response(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let text = String::from_utf8_lossy(&bytes);
        json!({"error": text.to_string()})
    })
}

// Helper to create and approve a test user, returning their session token
async fn create_approved_user_with_session(pool: &SqlitePool, email: &str, password: &str) -> String {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password_with_params(password, 19456, 2, 1)
        .expect("Failed to hash password");
    let now = chrono::Utc::now().to_rfc3339();

    // Insert approved user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_admin, is_active, created_at)
           VALUES (?, ?, ?, 1, 0, 1, ?)"#,
        user_id,
        email,
        password_hash,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create user");

    // Create session token
    let session_token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();

    let session_id = uuid::Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", Sha256::digest(session_token.as_bytes()));
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

// ============================================================================
// Login Tests
// ============================================================================

#[tokio::test]
async fn test_user_login_success() {
    let (app, pool) = create_test_app().await;

    // Create an approved user
    let _token = create_approved_user_with_session(&pool, "testuser@example.com", "password123456").await;

    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "testuser@example.com",
                "password": "password123456"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["sessionId"].is_string());
    assert!(body["expiresAt"].is_string());
    assert_eq!(body["user"]["email"], "testuser@example.com");

    pool.close().await;
}

#[tokio::test]
async fn test_user_login_wrong_password() {
    let (app, pool) = create_test_app().await;

    let _token = create_approved_user_with_session(&pool, "testuser@example.com", "correctpassword").await;

    let request = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "testuser@example.com",
                "password": "wrongpassword"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

// ============================================================================
// Change Password Tests
// ============================================================================

#[tokio::test]
async fn test_change_password_success() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "changepass@example.com", "oldpassword123").await;

    let request = Request::builder()
        .uri("/api/v1/user/change-password")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "currentPassword": "oldpassword123",
                "newPassword": "newpassword456"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_wrong_current_password() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "changepass@example.com", "correctpassword").await;

    let request = Request::builder()
        .uri("/api/v1/user/change-password")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "currentPassword": "wrongpassword",
                "newPassword": "newpassword456"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_new_password_too_short() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "changepass@example.com", "oldpassword123").await;

    let request = Request::builder()
        .uri("/api/v1/user/change-password")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "currentPassword": "oldpassword123",
                "newPassword": "short"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_requires_auth() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/user/change-password")
        .method("POST")
        .header("content-type", "application/json")
        // No authorization header
        .body(Body::from(
            json!({
                "currentPassword": "oldpassword123",
                "newPassword": "newpassword456"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Get Account Info Tests
// ============================================================================

#[tokio::test]
async fn test_get_account_info_success() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "account@example.com", "password123456").await;

    let request = Request::builder()
        .uri("/api/v1/user/account")
        .method("GET")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["email"], "account@example.com");
    assert!(body["noteCount"].is_number());
    assert!(body["attachmentCount"].is_number());
    assert!(body["storageUsedBytes"].is_number());
    assert!(body["storageQuotaMb"].is_number());
    assert!(body["createdAt"].is_string());

    pool.close().await;
}

#[tokio::test]
async fn test_get_account_info_requires_auth() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/user/account")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Logout Tests
// ============================================================================

#[tokio::test]
async fn test_logout_success() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "logout@example.com", "password123456").await;

    let request = Request::builder()
        .uri("/api/v1/user/logout")
        .method("POST")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    pool.close().await;
}

#[tokio::test]
async fn test_logout_invalidates_session() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "logout2@example.com", "password123456").await;

    // First logout
    let logout_request = Request::builder()
        .uri("/api/v1/user/logout")
        .method("POST")
        .header("authorization", format!("Bearer {}", token.clone()))
        .body(Body::empty())
        .unwrap();

    let app2 = app.clone();
    let logout_response = app.oneshot(logout_request).await.unwrap();
    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    // Try to use the same token - should fail
    let account_request = Request::builder()
        .uri("/api/v1/user/account")
        .method("GET")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let account_response = app2.oneshot(account_request).await.unwrap();
    assert_eq!(account_response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

// ============================================================================
// Delete Account Tests
// ============================================================================

#[tokio::test]
async fn test_deactivate_account_success() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "deactivate@example.com", "password123456").await;

    let request = Request::builder()
        .uri("/api/v1/user/account?mode=deactivate")
        .method("DELETE")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is deactivated
    let user = sqlx::query!(
        "SELECT is_active FROM users WHERE email = ?",
        "deactivate@example.com"
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch user");

    assert_eq!(user.is_active, 0);

    pool.close().await;
}

#[tokio::test]
async fn test_delete_account_success() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "delete@example.com", "password123456").await;

    let request = Request::builder()
        .uri("/api/v1/user/account?mode=delete")
        .method("DELETE")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is deleted
    let user = sqlx::query!(
        "SELECT id FROM users WHERE email = ?",
        "delete@example.com"
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query user");

    assert!(user.is_none(), "User should be deleted");

    pool.close().await;
}

#[tokio::test]
async fn test_delete_account_invalid_mode() {
    let (app, pool) = create_test_app().await;

    let token = create_approved_user_with_session(&pool, "invalid@example.com", "password123456").await;

    let request = Request::builder()
        .uri("/api/v1/user/account?mode=invalid")
        .method("DELETE")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    pool.close().await;
}

#[tokio::test]
async fn test_delete_account_requires_auth() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/user/account?mode=delete")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Phase 0 Auth Hardening Tests
// ============================================================================

/// Helper: create a second session for an existing user and return its token.
async fn create_additional_session(pool: &SqlitePool, user_id: &str) -> String {
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
        session_id, user_id, token_hash, expires_at, now, now,
    )
    .execute(pool)
    .await
    .expect("Failed to create additional session");
    session_token
}

/// Helper: create a client (device) row for a user and return (client_id, api_key).
async fn create_device_for_user(pool: &SqlitePool, user_id: &str, name: &str) -> (String, String) {
    let client_id = uuid::Uuid::new_v4().to_string();
    let api_key: String = (0..32).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
    let api_key_hash = format!("{:x}", Sha256::digest(api_key.as_bytes()));
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query!(
        r#"INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, is_active)
           VALUES (?, ?, ?, ?, 'web', ?, 1)"#,
        client_id, user_id, api_key_hash, name, now,
    )
    .execute(pool)
    .await
    .expect("Failed to create client");
    (client_id, api_key)
}

#[tokio::test]
async fn test_change_password_revokes_other_sessions_but_keeps_current() {
    let (app, pool) = create_test_app().await;

    // A user with two sessions: the one we'll use to call change-password,
    // and a second one that should be invalidated.
    let current_token = create_approved_user_with_session(&pool, "two-sessions@example.com", "initialpw12").await;

    // Look up user_id for the account we just created
    let user_id: String = sqlx::query_scalar!(
        "SELECT id FROM users WHERE email = ?", "two-sessions@example.com"
    )
    .fetch_one(&pool).await.unwrap();

    let other_token = create_additional_session(&pool, &user_id).await;

    // Before: two sessions exist
    let before: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM sessions WHERE user_id = ?", user_id)
        .fetch_one(&pool).await.unwrap();
    assert_eq!(before, 2);

    // Change password via current session
    let request = Request::builder()
        .uri("/api/v1/user/change-password")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", current_token))
        .body(Body::from(json!({
            "currentPassword": "initialpw12",
            "newPassword": "rotatedpw1234",
        }).to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // After: current session still valid, other session gone
    let after: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM sessions WHERE user_id = ?", user_id)
        .fetch_one(&pool).await.unwrap();
    assert_eq!(after, 1, "only the current session should survive");

    // Verify the `other_token` no longer validates (look it up by its hash)
    let other_hash = format!("{:x}", Sha256::digest(other_token.as_bytes()));
    let other_still_there: Option<String> = sqlx::query_scalar!(
        "SELECT id FROM sessions WHERE token_hash = ?", other_hash
    )
    .fetch_optional(&pool).await.unwrap();
    assert!(other_still_there.is_none(), "revoked session must not be in the DB");

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_deactivates_device_api_keys() {
    let (app, pool) = create_test_app().await;
    let token = create_approved_user_with_session(&pool, "devices@example.com", "initialpw12").await;
    let user_id: String = sqlx::query_scalar!(
        "SELECT id FROM users WHERE email = ?", "devices@example.com"
    )
    .fetch_one(&pool).await.unwrap();

    // Two active devices
    let (_, _) = create_device_for_user(&pool, &user_id, "phone").await;
    let (_, _) = create_device_for_user(&pool, &user_id, "laptop").await;

    let active_before: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM clients WHERE user_id = ? AND is_active = 1", user_id
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(active_before, 2);

    let request = Request::builder()
        .uri("/api/v1/user/change-password")
        .method("POST")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(json!({
            "currentPassword": "initialpw12",
            "newPassword": "rotatedpw1234",
        }).to_string()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Both devices must be soft-deactivated
    let active_after: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM clients WHERE user_id = ? AND is_active = 1", user_id
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(active_after, 0, "all device api keys should be deactivated after password change");

    pool.close().await;
}

#[tokio::test]
async fn test_check_status_requires_password() {
    let (_, pool) = create_test_app().await;
    // We need to register the /status route locally because create_test_app
    // doesn't expose it — build a tiny router around just this handler.
    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
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
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(), sync_broadcast, sse_tokens, config,
    });
    let status_app = Router::new()
        .route("/api/v1/user/status", post(jottery_server::api::user::check_status))
        .with_state(app_state);

    // Create one approved user and one pending user.
    let _ = create_approved_user_with_session(&pool, "approved@example.com", "secretpw12345").await;

    // Pending user (approved = 0)
    use jottery_server::utils::password::hash_password_with_params;
    let pending_id = uuid::Uuid::new_v4().to_string();
    let pending_hash = hash_password_with_params("pendingpw12345", 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_admin, is_active, created_at)
           VALUES (?, ?, ?, 0, 0, 1, ?)"#,
        pending_id, "pending@example.com", pending_hash, now,
    ).execute(&pool).await.unwrap();

    // Unknown email → 401
    let r = status_app.clone().oneshot(
        Request::builder()
            .uri("/api/v1/user/status")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(json!({"email": "nobody@example.com", "password": "whatever123"}).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "unknown email must not reveal non-existence");

    // Wrong password → 401
    let r = status_app.clone().oneshot(
        Request::builder()
            .uri("/api/v1/user/status")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(json!({"email": "approved@example.com", "password": "wrongpw1234567"}).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED, "wrong password must not reveal existence");

    // Correct credentials → approved
    let r = status_app.clone().oneshot(
        Request::builder()
            .uri("/api/v1/user/status")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(json!({"email": "approved@example.com", "password": "secretpw12345"}).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = parse_json_response(r.into_body()).await;
    assert_eq!(body["status"], "approved");

    // Correct credentials on pending account → pending_approval
    let r = status_app.oneshot(
        Request::builder()
            .uri("/api/v1/user/status")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(json!({"email": "pending@example.com", "password": "pendingpw12345"}).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = parse_json_response(r.into_body()).await;
    assert_eq!(body["status"], "pending_approval");

    pool.close().await;
}

// ============================================================================
// Workflow Tests
// ============================================================================

#[cfg(test)]
mod workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_password_change_and_login_with_new_password() {
        let (app, pool) = create_test_app().await;

        let token = create_approved_user_with_session(&pool, "workflow@example.com", "oldpassword123").await;

        // Step 1: Change password
        let change_request = Request::builder()
            .uri("/api/v1/user/change-password")
            .method("POST")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(
                json!({
                    "currentPassword": "oldpassword123",
                    "newPassword": "newpassword456"
                })
                .to_string(),
            ))
            .unwrap();

        let change_response = app.clone().oneshot(change_request).await.unwrap();
        assert_eq!(change_response.status(), StatusCode::NO_CONTENT);

        // Step 2: Login with new password should succeed
        let login_request = Request::builder()
            .uri("/api/v1/user/login")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "workflow@example.com",
                    "password": "newpassword456"
                })
                .to_string(),
            ))
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);

        // Step 3: Login with old password should fail
        let old_login_request = Request::builder()
            .uri("/api/v1/user/login")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "workflow@example.com",
                    "password": "oldpassword123"
                })
                .to_string(),
            ))
            .unwrap();

        let old_login_response = app.oneshot(old_login_request).await.unwrap();
        assert_eq!(old_login_response.status(), StatusCode::UNAUTHORIZED);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_deactivate_prevents_api_access() {
        let (app, pool) = create_test_app().await;

        let token = create_approved_user_with_session(&pool, "deactivate2@example.com", "password123456").await;

        // Step 1: Deactivate account
        let deactivate_request = Request::builder()
            .uri("/api/v1/user/account?mode=deactivate")
            .method("DELETE")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let deactivate_response = app.clone().oneshot(deactivate_request).await.unwrap();
        assert_eq!(deactivate_response.status(), StatusCode::NO_CONTENT);

        // Step 2: Login still works (creates a session token)
        // Note: The login endpoint doesn't check is_active - it should arguably reject inactive users
        let login_request = Request::builder()
            .uri("/api/v1/user/login")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "deactivate2@example.com",
                    "password": "password123456"
                })
                .to_string(),
            ))
            .unwrap();

        let login_response = app.clone().oneshot(login_request).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK, "Login endpoint accepts deactivated users");

        // Get the new session token
        let body = parse_json_response(login_response.into_body()).await;
        let new_token = body["sessionId"].as_str().expect("Should have session token");

        // Step 3: But using that session to access protected endpoints should fail
        let account_request = Request::builder()
            .uri("/api/v1/user/account")
            .method("GET")
            .header("authorization", format!("Bearer {}", new_token))
            .body(Body::empty())
            .unwrap();

        let account_response = app.oneshot(account_request).await.unwrap();
        assert_eq!(
            account_response.status(),
            StatusCode::FORBIDDEN,
            "Deactivated user should be blocked by auth middleware"
        );

        pool.close().await;
    }
}
