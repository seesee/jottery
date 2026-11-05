// Attacks on the real routes. Isolation between users is in sync_isolation_tests,
// and which credentials open which route is in route_auth_tests.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::{db::SessionRepository, models::CreateSessionParams};
use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashSet;

const INJECTIONS: &[&str] = &[
    "' OR '1'='1",
    "admin@localhost' --",
    "'; DROP TABLE users; --",
    "\" OR \"\"=\"",
];

async fn table_exists(pool: &SqlitePool, name: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
        > 0
}

async fn signed_in_user(app: &axum::Router, pool: &SqlitePool) -> (String, String) {
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let id = create_user(pool, &email, &password).await;
    let (status, body) = call(
        app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    (id, body["sessionId"].as_str().unwrap().to_string())
}

#[tokio::test]
async fn sql_in_a_login_address_is_only_ever_an_address() {
    let db = migrated_db().await;
    let app = app(&db.pool);

    for email in INJECTIONS {
        for uri in ["/api/v1/auth/login", "/api/v1/user/login", "/api/v1/user/status"] {
            let (status, _) = call(
                &app,
                "POST",
                uri,
                None,
                Some(json!({ "email": email, "password": random_password() })),
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri} with {email}");
        }
    }

    assert!(table_exists(&db.pool, "users").await);
}

#[tokio::test]
async fn sql_in_note_content_is_stored_as_written() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;

    for (index, content) in INJECTIONS.iter().enumerate() {
        let id = new_id();
        let time = format!("2026-02-0{}T00:00:00Z", index + 1);
        push(&app, &key, json!({ "notes": [note(&id, content, &time)] })).await;

        let pulled = pull_all(&app, &key).await;
        let stored = pulled["notes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == id.as_str())
            .unwrap();
        assert_eq!(stored["content"], json!(content));
    }

    assert!(table_exists(&db.pool, "notes").await);
}

#[tokio::test]
async fn sql_in_a_path_is_only_ever_an_id() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;

    let (status, _) = call(
        &app,
        "GET",
        "/api/v1/sync/attachments/x%27%20OR%20%271%27%3D%271",
        Some(&key),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_regular_user_cannot_make_themselves_an_admin() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, token) = signed_in_user(&app, &db.pool).await;

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/admin/users/{id}/toggle-admin"),
        Some(&token),
        Some(json!({ "is_admin": true })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    let is_admin: i64 = sqlx::query_scalar("SELECT is_admin FROM users WHERE id = ?")
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(is_admin, 0);
}

#[tokio::test]
async fn an_unapproved_user_cannot_enrol_a_device() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let id = create_user(&db.pool, &email, &password).await;
    sqlx::query("UPDATE users SET approved = 0 WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/auth/register-device",
        None,
        Some(json!({
            "email": email,
            "password": password,
            "deviceName": "phone",
            "deviceType": "ios",
        })),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    let devices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clients WHERE user_id = ?")
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(devices, 0);
}

async fn session_expiring(pool: &SqlitePool, user_id: &str, expires_at: String) -> String {
    let token = new_id();
    SessionRepository::create(
        pool,
        CreateSessionParams {
            user_id: user_id.to_string(),
            token: token.clone(),
            expires_at,
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .unwrap();
    token
}

#[tokio::test]
async fn an_expired_session_opens_nothing_and_is_removed() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let expired = (chrono::Utc::now() - chrono::Duration::seconds(1)).to_rfc3339();
    let token = session_expiring(&db.pool, &user, expired).await;

    let (status, _) = call(&app, "GET", "/api/v1/user/account", Some(&token), None).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
        .bind(&user)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn a_session_works_until_it_expires() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let later = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
    let token = session_expiring(&db.pool, &user, later).await;

    let (status, _) = call(&app, "GET", "/api/v1/user/account", Some(&token), None).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn sessions_are_stored_hashed_and_never_repeat() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    create_user(&db.pool, &email, &password).await;

    let mut tokens = HashSet::new();
    for _ in 0..5 {
        let (_, body) = call(
            &app,
            "POST",
            "/api/v1/user/login",
            None,
            Some(json!({ "email": email, "password": password })),
        )
        .await;
        tokens.insert(body["sessionId"].as_str().unwrap().to_string());
    }
    assert_eq!(tokens.len(), 5);

    let stored: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    assert!(stored.iter().all(|hash| !tokens.contains(hash)));
}

#[tokio::test]
async fn each_device_gets_its_own_key_stored_hashed() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    create_user(&db.pool, &email, &password).await;

    let mut keys = HashSet::new();
    for name in ["one", "two", "three"] {
        let (status, body) = call(
            &app,
            "POST",
            "/api/v1/auth/register-device",
            None,
            Some(json!({
                "email": email,
                "password": password,
                "deviceName": name,
                "deviceType": "tui",
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        keys.insert(body["apiKey"].as_str().unwrap().to_string());
    }
    assert_eq!(keys.len(), 3);

    let stored: Vec<String> = sqlx::query_scalar("SELECT api_key FROM clients")
        .fetch_all(&db.pool)
        .await
        .unwrap();
    assert!(stored.iter().all(|hash| !keys.contains(hash)));
}

#[tokio::test]
async fn a_revoked_device_stays_on_record_but_stops_working() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (device, key) = create_device(&db.pool, &user).await;

    let (status, _) = call(&app, "DELETE", "/api/v1/sync/device", Some(&key), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = call(&app, "GET", "/api/v1/sync/status", Some(&key), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM clients WHERE id = ?")
        .bind(&device)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(kept, 1);
}
