// Changing a password has to end whatever the old one let in, and the password
// rules have to hold on every route that sets one.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::{config::Config, services::AdminService};
use serde_json::{json, Value};
use sqlx::SqlitePool;

/// Meets PASSWORD_COMPLEXITY=strong.
fn strong_password() -> String {
    format!("Aa1!{}", new_id())
}

/// Long enough, but all one class.
fn weak_password() -> String {
    new_id().replace(|c: char| !c.is_ascii_lowercase(), "x")
}

struct Account {
    id: String,
    email: String,
    password: String,
}

async fn account(pool: &SqlitePool) -> Account {
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let id = create_user(pool, &email, &password).await;
    Account { id, email, password }
}

async fn admin(pool: &SqlitePool) -> Account {
    let account = account(pool).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&account.id)
        .execute(pool)
        .await
        .unwrap();
    account
}

async fn session(app: &axum::Router, uri: &str, account: &Account) -> String {
    let (status, body) = call(
        app,
        "POST",
        uri,
        None,
        Some(json!({ "email": account.email, "password": account.password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["sessionId"].as_str().unwrap().to_string()
}

async fn portal_session(app: &axum::Router, account: &Account) -> String {
    session(app, "/api/v1/user/login", account).await
}

async fn admin_session(app: &axum::Router, account: &Account) -> String {
    session(app, "/api/v1/auth/login", account).await
}

async fn session_works(app: &axum::Router, token: &str) -> bool {
    let (status, _) = call(app, "GET", "/api/v1/user/account", Some(token), None).await;
    status == StatusCode::OK
}

async fn device_works(app: &axum::Router, key: &str) -> bool {
    let (status, _) = call(app, "GET", "/api/v1/sync/status", Some(key), None).await;
    status == StatusCode::OK
}

async fn stored_hash(pool: &SqlitePool, user_id: &str) -> String {
    sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn changing_your_password_ends_your_other_sessions() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&db.pool).await;
    let attacker = portal_session(&app, &user).await;
    let current = portal_session(&app, &user).await;

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/user/change-password",
        Some(&current),
        Some(json!({ "currentPassword": user.password, "newPassword": random_password() })),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!session_works(&app, &attacker).await, "the old session survived");
    assert!(session_works(&app, &current).await, "the caller was signed out");
}

#[tokio::test]
async fn changing_your_password_leaves_devices_syncing() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&db.pool).await;
    let (_, key) = create_device(&db.pool, &user.id).await;
    let current = portal_session(&app, &user).await;

    call(
        &app,
        "POST",
        "/api/v1/user/change-password",
        Some(&current),
        Some(json!({ "currentPassword": user.password, "newPassword": random_password() })),
    )
    .await;

    assert!(device_works(&app, &key).await);
}

#[tokio::test]
async fn another_users_sessions_are_left_alone() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&db.pool).await;
    let other = account(&db.pool).await;
    let bystander = portal_session(&app, &other).await;
    let current = portal_session(&app, &user).await;

    call(
        &app,
        "POST",
        "/api/v1/user/change-password",
        Some(&current),
        Some(json!({ "currentPassword": user.password, "newPassword": random_password() })),
    )
    .await;

    assert!(session_works(&app, &bystander).await);
}

#[tokio::test]
async fn an_admin_changing_their_password_ends_their_other_sessions() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let admin = admin(&db.pool).await;
    let attacker = admin_session(&app, &admin).await;
    let current = admin_session(&app, &admin).await;

    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/admin/change-password",
        Some(&current),
        Some(json!({ "current_password": admin.password, "new_password": random_password() })),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(!session_works(&app, &attacker).await);
    assert!(session_works(&app, &current).await);
}

#[tokio::test]
async fn an_admin_reset_ends_every_session_of_that_user() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let admin = admin(&db.pool).await;
    let token = admin_session(&app, &admin).await;
    let user = account(&db.pool).await;
    let first = portal_session(&app, &user).await;
    let second = portal_session(&app, &user).await;

    let (status, body) = call(
        &app,
        "POST",
        &format!("/api/v1/admin/users/{}/reset-password", user.id),
        Some(&token),
        Some(json!({ "new_password": random_password() })),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert!(!session_works(&app, &first).await);
    assert!(!session_works(&app, &second).await);
    assert!(session_works(&app, &token).await, "the admin's own session is untouched");
}

#[tokio::test]
async fn a_reset_keeps_device_keys_unless_asked_to_revoke_them() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let admin = admin(&db.pool).await;
    let token = admin_session(&app, &admin).await;
    let user = account(&db.pool).await;
    let (_, key) = create_device(&db.pool, &user.id).await;
    let uri = format!("/api/v1/admin/users/{}/reset-password", user.id);

    let (status, _) = call(
        &app,
        "POST",
        &uri,
        Some(&token),
        Some(json!({ "new_password": random_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(device_works(&app, &key).await);

    let (status, _) = call(
        &app,
        "POST",
        &uri,
        Some(&token),
        Some(json!({ "new_password": random_password(), "revokeDevices": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!device_works(&app, &key).await);
}

async fn strict_app(pool: &SqlitePool) -> axum::Router {
    app_with(
        pool,
        Config {
            password_complexity: "strong".to_string(),
            ..test_config()
        },
    )
}

#[tokio::test]
async fn the_admin_routes_apply_the_configured_password_rules() {
    let db = migrated_db().await;
    let app = strict_app(&db.pool).await;
    let admin = admin(&db.pool).await;
    let token = admin_session(&app, &admin).await;
    let user = account(&db.pool).await;
    let before = (stored_hash(&db.pool, &admin.id).await, stored_hash(&db.pool, &user.id).await);

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/admin/change-password",
        Some(&token),
        Some(json!({ "current_password": admin.password, "new_password": weak_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let reset = format!("/api/v1/admin/users/{}/reset-password", user.id);
    let (status, _) = call(
        &app,
        "POST",
        &reset,
        Some(&token),
        Some(json!({ "new_password": weak_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let after = (stored_hash(&db.pool, &admin.id).await, stored_hash(&db.pool, &user.id).await);
    assert_eq!(before, after);

    let (status, body) = call(
        &app,
        "POST",
        &reset,
        Some(&token),
        Some(json!({ "new_password": strong_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

#[tokio::test]
async fn the_command_line_applies_them_too() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&db.pool).await;
    let session = portal_session(&app, &user).await;

    let refused =
        AdminService::reset_password(&db.pool, &user.id, &weak_password(), (1024, 1, 1), "strong").await;
    assert!(refused.is_err());
    assert!(session_works(&app, &session).await);

    AdminService::reset_password(&db.pool, &user.id, &strong_password(), (1024, 1, 1), "strong")
        .await
        .unwrap();
    assert!(!session_works(&app, &session).await);
}

fn change_with_key(user: &Account, wrapped: Value) -> Value {
    let mut body = json!({ "currentPassword": user.password, "newPassword": random_password() });
    for (key, value) in wrapped.as_object().unwrap() {
        body[key] = value.clone();
    }
    body
}

#[tokio::test]
async fn a_key_sent_with_a_password_change_is_checked_first() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&db.pool).await;
    let token = portal_session(&app, &user).await;
    let before = stored_hash(&db.pool, &user.id).await;

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/user/change-password",
        Some(&token),
        Some(change_with_key(&user, json!({ "wrappedBlob": "blob", "kdfVersion": 1, "kdfIterations": 1 }))),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(stored_hash(&db.pool, &user.id).await, before);
}

#[tokio::test]
async fn a_key_sent_with_a_password_change_is_stored_with_it() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&db.pool).await;
    let token = portal_session(&app, &user).await;
    let before = stored_hash(&db.pool, &user.id).await;

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/user/change-password",
        Some(&token),
        Some(change_with_key(&user, json!({ "wrappedBlob": "blob", "kdfVersion": 1, "kdfIterations": 1_000_000 }))),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_ne!(stored_hash(&db.pool, &user.id).await, before);
    let blob: String = sqlx::query_scalar("SELECT wrapped_blob FROM wrapped_keys WHERE user_id = ?")
        .bind(&user.id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(blob, "blob");
}
