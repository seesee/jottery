// Finding out where an account stands. GET answers for any address, so anyone can
// test who has an account: it stays only for released clients. POST answers only
// someone who holds the password.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use sqlx::SqlitePool;

const STATUS: &str = "/api/v1/user/status";

async fn account(pool: &SqlitePool) -> (String, String, String) {
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let id = create_user(pool, &email, &password).await;
    (id, email, password)
}

async fn set(pool: &SqlitePool, column: &str, value: i64, id: &str) {
    sqlx::query(&format!("UPDATE users SET {column} = ? WHERE id = ?"))
        .bind(value)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn check(app: &axum::Router, email: &str, password: &str) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        STATUS,
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await
}

#[tokio::test]
async fn the_right_password_gets_the_real_state() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, email, password) = account(&db.pool).await;

    let (status, body) = check(&app, &email, &password).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "exists": true, "isApproved": true, "isActive": true }));

    set(&db.pool, "approved", 0, &id).await;
    let (_, body) = check(&app, &email, &password).await;
    assert_eq!(body, json!({ "exists": true, "isApproved": false, "isActive": true }));

    set(&db.pool, "is_active", 0, &id).await;
    let (_, body) = check(&app, &email, &password).await;
    assert_eq!(body, json!({ "exists": true, "isApproved": false, "isActive": false }));
}

#[tokio::test]
async fn a_stranger_cannot_tell_an_account_from_no_account() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (_, email, _) = account(&db.pool).await;

    let (known_status, known_body) = check(&app, &email, &random_password()).await;
    let unknown = format!("{}@example.com", new_id());
    let (unknown_status, unknown_body) = check(&app, &unknown, &random_password()).await;

    assert_eq!(known_status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_status, StatusCode::UNAUTHORIZED);
    assert_eq!(known_body, unknown_body);
}

#[tokio::test]
async fn guessing_here_counts_against_the_address() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (_, email, password) = account(&db.pool).await;

    for _ in 0..6 {
        check(&app, &email, &random_password()).await;
    }

    let (status, body) = check(&app, &email, &password).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], json!("rate_limited"));
}

#[tokio::test]
async fn released_clients_can_still_poll_with_get() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, email, _) = account(&db.pool).await;
    set(&db.pool, "approved", 0, &id).await;

    let uri = format!("{STATUS}?email={email}");
    let (status, body) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["isApproved"], json!(false));

    set(&db.pool, "approved", 1, &id).await;
    let (_, body) = call(&app, "GET", &uri, None, None).await;
    assert_eq!(body["isApproved"], json!(true));
}
