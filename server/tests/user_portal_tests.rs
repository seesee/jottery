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
        password_complexity: "none".to_string(),
        enable_hsts: false,
    };

    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
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
