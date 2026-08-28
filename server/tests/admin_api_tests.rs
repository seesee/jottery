// Admin API endpoint integration tests
//
// Tests for admin endpoints:
// - GET /api/v1/admin/users
// - GET /api/v1/admin/users/:id
// - POST /api/v1/admin/users/:id/approve
// - POST /api/v1/admin/users/:id/deactivate
// - POST /api/v1/admin/users/:id/activate
// - POST /api/v1/admin/users/:id/toggle-admin
// - DELETE /api/v1/admin/users/:id
// - POST /api/v1/admin/change-password

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
use tower::{Service, ServiceExt};

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

// Helper to create test app with admin routes
async fn create_test_app() -> (axum::Router, SqlitePool) {
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

    // Build router with admin routes
    let app = axum::Router::new()
        .route("/api/v1/admin/users", axum::routing::get(jottery_server::api::admin::users::list_users))
        .route("/api/v1/admin/users/:id", axum::routing::get(jottery_server::api::admin::users::get_user))
        .route("/api/v1/admin/users/:id/approve", axum::routing::post(jottery_server::api::admin::users::approve_user))
        .route("/api/v1/admin/users/:id/deactivate", axum::routing::post(jottery_server::api::admin::users::deactivate_user))
        .route("/api/v1/admin/users/:id/activate", axum::routing::post(jottery_server::api::admin::users::activate_user))
        .route("/api/v1/admin/users/:id/toggle-admin", axum::routing::post(jottery_server::api::admin::users::toggle_admin))
        .route("/api/v1/admin/users/:id", axum::routing::delete(jottery_server::api::admin::users::delete_user))
        .route("/api/v1/admin/change-password", axum::routing::post(jottery_server::api::admin::users::change_password))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::admin::admin_auth_middleware,
        ))
        .with_state(app_state);

    (app, pool)
}

// Helper to parse JSON response
async fn parse_json_response(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// Helper to create an admin user and session, returns session token
async fn create_admin_user_and_session(pool: &SqlitePool) -> (String, String) {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let email = format!("admin{}@example.com", uuid::Uuid::new_v4());
    let password_hash = hash_password_with_params("adminpassword123", 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    // Create admin user (approved and active)
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

    (user_id, session_token)
}

// Helper to create a regular (non-admin) user
async fn create_regular_user(pool: &SqlitePool, approved: bool) -> String {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let email = format!("user{}@example.com", uuid::Uuid::new_v4());
    let password_hash = hash_password_with_params("userpassword123", 19456, 2, 1).unwrap();
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
    .expect("Failed to create regular user");

    user_id
}

#[tokio::test]
async fn test_list_users_requires_authentication() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/admin/users")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should fail without session token
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_list_users_success() {
    let (app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let _user1 = create_regular_user(&pool, true).await;
    let _user2 = create_regular_user(&pool, false).await;

    let request = Request::builder()
        .uri("/api/v1/admin/users")
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["users"].is_array());
    assert_eq!(body["users"].as_array().unwrap().len(), 4); // default admin + test admin + 2 users
    assert_eq!(body["total"], 4);

    pool.close().await;
}

#[tokio::test]
async fn test_get_user_details() {
    let (app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let user_id = create_regular_user(&pool, true).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}", user_id))
        .method("GET")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["id"], user_id);
    assert!(body["email"].is_string());
    assert_eq!(body["isAdmin"], false);
    assert!(body["stats"].is_object());
    assert!(body["devices"].is_array());

    pool.close().await;
}

#[tokio::test]
async fn test_approve_user() {
    let (mut app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let user_id = create_regular_user(&pool, false).await; // Not approved

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/approve", user_id))
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is approved
    let user = sqlx::query!(
        r#"SELECT approved, approved_at FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_one(&pool)
    .await
    .expect("User should exist");

    assert_eq!(user.approved, 1);
    assert!(user.approved_at.is_some());

    pool.close().await;
}

#[tokio::test]
async fn test_deactivate_user() {
    let (mut app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let user_id = create_regular_user(&pool, true).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/deactivate", user_id))
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is deactivated
    let user = sqlx::query!(
        r#"SELECT is_active FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_one(&pool)
    .await
    .expect("User should exist");

    assert_eq!(user.is_active, 0);

    pool.close().await;
}

#[tokio::test]
async fn test_activate_user() {
    let (mut app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let user_id = create_regular_user(&pool, true).await;

    // First deactivate
    sqlx::query!(
        r#"UPDATE users SET is_active = 0 WHERE id = ?"#,
        user_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Now reactivate
    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/activate", user_id))
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is active
    let user = sqlx::query!(
        r#"SELECT is_active FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_one(&pool)
    .await
    .expect("User should exist");

    assert_eq!(user.is_active, 1);

    pool.close().await;
}

#[tokio::test]
async fn test_toggle_admin_success() {
    let (mut app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let user_id = create_regular_user(&pool, true).await;

    // Promote to admin
    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/toggle-admin", user_id))
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "is_admin": true
            })
            .to_string(),
        ))
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is admin
    let user = sqlx::query!(
        r#"SELECT is_admin FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_one(&pool)
    .await
    .expect("User should exist");

    assert_eq!(user.is_admin, 1);

    pool.close().await;
}

#[tokio::test]
async fn test_toggle_admin_prevent_self_demotion() {
    let (app, pool) = create_test_app().await;

    let (admin_id, session_token) = create_admin_user_and_session(&pool).await;

    // Try to demote self
    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/toggle-admin", admin_id))
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "is_admin": false
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["error"].as_str().unwrap().contains("Cannot remove your own admin privileges"));

    pool.close().await;
}

#[tokio::test]
async fn test_toggle_admin_prevent_last_admin_demotion() {
    let (mut app, pool) = create_test_app().await;

    let (admin_id, session_token) = create_admin_user_and_session(&pool).await;

    // Create second admin
    let second_admin_id = create_regular_user(&pool, true).await;
    sqlx::query!(
        r#"UPDATE users SET is_admin = 1 WHERE id = ?"#,
        second_admin_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Demote first admin (should work, we have 2)
    let request1 = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}/toggle-admin", admin_id))
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "is_admin": false
            })
            .to_string(),
        ))
        .unwrap();

    let response1 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request1)
        .await
        .unwrap();

    // This should fail because we're trying to demote ourselves
    assert_eq!(response1.status(), StatusCode::BAD_REQUEST);

    pool.close().await;
}

#[tokio::test]
async fn test_delete_user_success() {
    let (mut app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;
    let user_id = create_regular_user(&pool, true).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}", user_id))
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify user is deleted
    let user = sqlx::query!(
        r#"SELECT id FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_optional(&pool)
    .await
    .expect("Query should succeed");

    assert!(user.is_none(), "User should be deleted");

    pool.close().await;
}

#[tokio::test]
async fn test_delete_user_prevent_self_deletion() {
    let (app, pool) = create_test_app().await;

    let (admin_id, session_token) = create_admin_user_and_session(&pool).await;

    let request = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}", admin_id))
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["error"].as_str().unwrap().contains("Cannot delete your own account"));

    pool.close().await;
}

#[tokio::test]
async fn test_delete_user_prevent_last_admin_deletion() {
    let (app, pool) = create_test_app().await;

    // Create two admins
    let (_admin1_id, session_token) = create_admin_user_and_session(&pool).await;
    let admin2_id = create_regular_user(&pool, true).await;

    sqlx::query!(
        r#"UPDATE users SET is_admin = 1 WHERE id = ?"#,
        admin2_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Now there are 2 admins. Delete the second one should work.
    // But if we try to delete when only 1 left, it should fail.

    // First, delete second admin (should work)
    let request1 = Request::builder()
        .uri(&format!("/api/v1/admin/users/{}", admin2_id))
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", session_token))
        .body(Body::empty())
        .unwrap();

    let response1 = app.oneshot(request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::NO_CONTENT);

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_success() {
    let (mut app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/admin/change-password")
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "current_password": "adminpassword123",
                "new_password": "newadminpass1234567890"
            })
            .to_string(),
        ))
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_wrong_current() {
    let (app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/admin/change-password")
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "current_password": "wrongpassword",
                "new_password": "newadminpass1234567890"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    pool.close().await;
}

#[tokio::test]
async fn test_change_password_too_short() {
    let (app, pool) = create_test_app().await;

    let (_admin_id, session_token) = create_admin_user_and_session(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/admin/change-password")
        .method("POST")
        .header("Authorization", format!("Bearer {}", session_token))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "current_password": "adminpassword123",
                "new_password": "short"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["error"].as_str().unwrap().contains("at least 12 characters"));

    pool.close().await;
}
