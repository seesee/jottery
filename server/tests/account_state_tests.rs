// What an account's state allows: deactivating a user must cut off their
// devices, and an unapproved user gets nothing but a place in the queue.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use sqlx::SqlitePool;

struct Fixture {
    db: TestDb,
    app: axum::Router,
    user_id: String,
    email: String,
    password: String,
    api_key: String,
    admin_session: String,
}

async fn setup() -> Fixture {
    let db = migrated_db().await;
    let app = app(&db.pool);

    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let user_id = create_user(&db.pool, &email, &password).await;
    let (_, api_key) = create_device(&db.pool, &user_id).await;

    let admin_email = format!("{}@example.com", new_id());
    let admin_password = random_password();
    let admin = create_user(&db.pool, &admin_email, &admin_password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&admin)
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": admin_email, "password": admin_password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    Fixture {
        admin_session: body["sessionId"].as_str().unwrap().to_string(),
        db,
        app,
        user_id,
        email,
        password,
        api_key,
    }
}

/// Everything a device key can reach.
fn device_requests() -> Vec<(&'static str, &'static str, Option<serde_json::Value>)> {
    vec![
        ("GET", "/api/v1/sync/status", None),
        (
            "POST",
            "/api/v1/sync/pull",
            Some(json!({ "knownNoteIds": [], "knownAttachmentIds": [] })),
        ),
        (
            "POST",
            "/api/v1/sync/push",
            Some(json!({ "notes": [], "attachments": [], "versions": [] })),
        ),
        ("GET", "/api/v1/auth/wrapped-key", None),
        ("GET", "/api/v1/inbox", None),
        ("GET", "/api/v1/sync/events/token", None),
    ]
}

async fn assert_device_refused(f: &Fixture, expected: StatusCode) {
    for (method, uri, body) in device_requests() {
        let (status, _) = call(&f.app, method, uri, Some(&f.api_key), body).await;
        assert_eq!(status, expected, "{method} {uri}");
    }
}

async fn set_flag(pool: &SqlitePool, column: &str, user_id: &str, value: i64) {
    sqlx::query(&format!("UPDATE users SET {column} = ? WHERE id = ?"))
        .bind(value)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn deactivating_a_user_stops_their_devices_syncing() {
    let f = setup().await;
    let (status, _) = call(&f.app, "GET", "/api/v1/sync/status", Some(&f.api_key), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &f.app,
        "POST",
        &format!("/api/v1/admin/users/{}/deactivate", f.user_id),
        Some(&f.admin_session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_device_refused(&f, StatusCode::FORBIDDEN).await;
}

#[tokio::test]
async fn reactivating_a_user_restores_their_devices() {
    let f = setup().await;
    for action in ["deactivate", "activate"] {
        let (status, _) = call(
            &f.app,
            "POST",
            &format!("/api/v1/admin/users/{}/{action}", f.user_id),
            Some(&f.admin_session),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    let (status, _) = call(&f.app, "GET", "/api/v1/sync/status", Some(&f.api_key), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn deactivating_your_own_account_stops_your_devices() {
    let f = setup().await;
    let (_, body) = call(
        &f.app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": f.email, "password": f.password })),
    )
    .await;
    let session = body["sessionId"].as_str().unwrap();

    let (status, _) = call(
        &f.app,
        "DELETE",
        "/api/v1/user/account?mode=deactivate",
        Some(session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    assert_device_refused(&f, StatusCode::FORBIDDEN).await;
}

#[tokio::test]
async fn an_unapproved_users_devices_are_refused() {
    let f = setup().await;
    set_flag(&f.db.pool, "approved", &f.user_id, 0).await;
    assert_device_refused(&f, StatusCode::FORBIDDEN).await;
}

#[tokio::test]
async fn a_revoked_device_is_still_unauthorised_not_forbidden() {
    // Clients tell these apart, so user state mustn't change what a dead key gets
    let f = setup().await;
    sqlx::query("UPDATE clients SET is_active = 0 WHERE user_id = ?")
        .bind(&f.user_id)
        .execute(&f.db.pool)
        .await
        .unwrap();
    assert_device_refused(&f, StatusCode::UNAUTHORIZED).await;
}
