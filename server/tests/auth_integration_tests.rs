// Authentication integration tests
//
// End-to-end tests for multi-user authentication endpoints:
// - User registration (/api/v1/auth/register-user)
// - Device registration (/api/v1/auth/register-device)
// - Admin login (/api/v1/admin/login)
// - User login (/api/v1/user/login)
// - User approval workflow
// - Session management

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use std::sync::Arc;
use tower::{Service, util::ServiceExt};

// Helper to create test database with migrations
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

// Helper to create test app
async fn create_test_app() -> (axum::Router, SqlitePool) {
    let pool = setup_test_db().await;

    // Create config
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
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    // Build router with all routes (similar to main.rs)
    let app = axum::Router::new()
        .route("/api/v1/auth/register-user", axum::routing::post(jottery_server::api::auth::register_user))
        .route("/api/v1/auth/register-device", axum::routing::post(jottery_server::api::auth::register_device))
        .route("/api/v1/auth/login", axum::routing::post(jottery_server::api::auth::login))
        .route("/api/v1/user/login", axum::routing::post(jottery_server::api::user::login))
        .with_state(app_state);

    (app, pool)
}

// Helper to parse JSON response
async fn parse_json_response(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_user_registration_success() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "newuser@example.com",
                "password": "securepassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["email"], "newuser@example.com");
    assert_eq!(body["status"], "pending_approval");
    assert!(body["userId"].is_string());
}

#[tokio::test]
async fn test_user_registration_duplicate_email() {
    let (app1, pool) = create_test_app().await;

    // Register first user
    let request1 = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "duplicate@example.com",
                "password": "password123456"
            })
            .to_string(),
        ))
        .unwrap();

    let response1 = app1.oneshot(request1).await.unwrap();

    assert_eq!(response1.status(), StatusCode::CREATED);

    // Create a new app instance that shares the same database pool
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
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    let app2 = axum::Router::new()
        .route("/api/v1/auth/register-user", axum::routing::post(jottery_server::api::auth::register_user))
        .with_state(app_state);

    // Try to register same email again
    let request2 = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "duplicate@example.com",
                "password": "different456789"
            })
            .to_string(),
        ))
        .unwrap();

    let response2 = app2.oneshot(request2).await.unwrap();

    assert_eq!(response2.status(), StatusCode::CONFLICT);

    pool.close().await;
}

#[tokio::test]
async fn test_user_registration_invalid_email() {
    let (app, _pool) = create_test_app().await;

    let invalid_emails = vec!["", "notanemail", "missing@domain", "@nodomain.com", "no-at-sign.com"];

    for email in invalid_emails {
        let request = Request::builder()
            .uri("/api/v1/auth/register-user")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": email,
                    "password": "validpassword123"
                })
                .to_string(),
            ))
            .unwrap();

        let response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Email '{}' should be rejected",
            email
        );
    }
}

#[tokio::test]
async fn test_user_registration_weak_password() {
    let (app, _pool) = create_test_app().await;

    let weak_passwords = vec!["short", "11char", "abc"];

    for password in weak_passwords {
        let request = Request::builder()
            .uri("/api/v1/auth/register-user")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "user@example.com",
                    "password": password
                })
                .to_string(),
            ))
            .unwrap();

        let response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "Weak password should be rejected (too short)"
        );
    }
}

#[tokio::test]
async fn test_admin_login_success() {
    let (app, _pool) = create_test_app().await;

    // Default admin account should exist (from migrations)
    let request = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "admin@localhost",
                "password": "changeme"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["sessionId"].is_string());
    assert!(body["expiresAt"].is_string());
    assert_eq!(body["user"]["email"], "admin@localhost");
    assert_eq!(body["user"]["isAdmin"], true);
}

#[tokio::test]
async fn test_admin_login_wrong_password() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "admin@localhost",
                "password": "wrongpassword"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_admin_login_nonexistent_user() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "nonexistent@example.com",
                "password": "anypassword"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_device_registration_requires_approved_user() {
    let (app, pool) = create_test_app().await;

    // Register a new user (will be unapproved)
    let register_request = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "unapproved@example.com",
                "password": "securepassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let register_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(register_request)
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);

    // Try to register a device for unapproved user
    let device_request = Request::builder()
        .uri("/api/v1/auth/register-device")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "unapproved@example.com",
                "password": "securepassword123",
                "deviceName": "Test Device",
                "deviceType": "cli"
            })
            .to_string(),
        ))
        .unwrap();

    let device_response = app.oneshot(device_request).await.unwrap();

    // Should fail because user is not approved
    assert_eq!(device_response.status(), StatusCode::FORBIDDEN);

    // The 403 status code itself indicates the user is forbidden from accessing the resource
    // (in this case because they're not approved yet)

    pool.close().await;
}

#[tokio::test]
async fn test_device_registration_approved_user_success() {
    let (app, pool) = create_test_app().await;

    // Register a new user
    let register_request = Request::builder()
        .uri("/api/v1/auth/register-user")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "approved@example.com",
                "password": "securepassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let register_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(register_request)
        .await
        .unwrap();

    assert_eq!(register_response.status(), StatusCode::CREATED);
    let register_body = parse_json_response(register_response.into_body()).await;
    let user_id = register_body["userId"].as_str().unwrap();

    // Manually approve the user in the database
    sqlx::query!("UPDATE users SET approved = 1 WHERE id = ?", user_id)
        .execute(&pool)
        .await
        .expect("Failed to approve user");

    // Now try to register a device
    let device_request = Request::builder()
        .uri("/api/v1/auth/register-device")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "approved@example.com",
                "password": "securepassword123",
                "deviceName": "Test Device",
                "deviceType": "cli"
            })
            .to_string(),
        ))
        .unwrap();

    let device_response = app.oneshot(device_request).await.unwrap();

    assert_eq!(device_response.status(), StatusCode::CREATED);

    let body = parse_json_response(device_response.into_body()).await;
    assert!(body["apiKey"].is_string());
    assert!(body["clientId"].is_string());
    assert_eq!(body["deviceName"], "Test Device");

    pool.close().await;
}

#[tokio::test]
async fn test_session_management() {
    let (app, _pool) = create_test_app().await;

    // Login as admin to get session
    let login_request = Request::builder()
        .uri("/api/v1/auth/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "email": "admin@localhost",
                "password": "changeme"
            })
            .to_string(),
        ))
        .unwrap();

    let login_response = app.oneshot(login_request).await.unwrap();

    assert_eq!(login_response.status(), StatusCode::OK);

    let body = parse_json_response(login_response.into_body()).await;
    let session_id = body["sessionId"].as_str().unwrap();
    let expires_at = body["expiresAt"].as_str().unwrap();

    // Verify session ID is UUID v4 format
    assert!(
        session_id.len() == 36 && session_id.matches('-').count() == 4,
        "Session ID should be UUID v4 format"
    );

    // Verify expiry is in the future
    let expires = chrono::DateTime::parse_from_rfc3339(expires_at).unwrap();
    let now = chrono::Utc::now();
    assert!(expires > now, "Session should expire in the future");

    // Verify expiry is approximately 7 days from now (default)
    let duration = expires.signed_duration_since(now);
    let days = duration.num_days();
    assert!(days >= 6 && days <= 8, "Session should expire in approximately 7 days, got {} days", days);
}

#[cfg(test)]
mod workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_user_approval_workflow() {
        let (app, pool) = create_test_app().await;

        // Step 1: User registers
        let register_request = Request::builder()
            .uri("/api/v1/auth/register-user")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "workflow@example.com",
                    "password": "securepassword123"
                })
                .to_string(),
            ))
            .unwrap();

        let register_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
            .await
            .unwrap()
            .call(register_request)
            .await
            .unwrap();

        assert_eq!(register_response.status(), StatusCode::CREATED);
        let register_body = parse_json_response(register_response.into_body()).await;
        assert_eq!(register_body["status"], "pending_approval");
        let user_id = register_body["userId"].as_str().unwrap();

        // Step 2: Device registration should fail (user not approved)
        let device_request1 = Request::builder()
            .uri("/api/v1/auth/register-device")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "workflow@example.com",
                    "password": "securepassword123",
                    "deviceName": "Device 1",
                    "deviceType": "cli"
                })
                .to_string(),
            ))
            .unwrap();

        let device_response1 = ServiceExt::<Request<Body>>::ready(&mut app.clone())
            .await
            .unwrap()
            .call(device_request1)
            .await
            .unwrap();

        assert_eq!(device_response1.status(), StatusCode::FORBIDDEN);

        // Step 3: Admin approves user
        sqlx::query!("UPDATE users SET approved = 1 WHERE id = ?", user_id)
            .execute(&pool)
            .await
            .expect("Failed to approve user");

        // Step 4: Device registration should now succeed
        let device_request2 = Request::builder()
            .uri("/api/v1/auth/register-device")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "workflow@example.com",
                    "password": "securepassword123",
                    "deviceName": "Device 1",
                    "deviceType": "cli"
                })
                .to_string(),
            ))
            .unwrap();

        let device_response2 = app.oneshot(device_request2).await.unwrap();

        assert_eq!(device_response2.status(), StatusCode::CREATED);
        let device_body = parse_json_response(device_response2.into_body()).await;
        assert!(device_body["apiKey"].is_string());

        pool.close().await;
    }
}
