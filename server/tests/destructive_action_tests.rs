// Deleting an account or every note can't be undone, so a session alone shouldn't be
// enough: it might be stolen, or left open in an unattended browser.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use sqlx::SqlitePool;

const NOTES: &str = "/api/v1/user/notes";
const DELETE_ACCOUNT: &str = "/api/v1/user/account?mode=delete";
const DEACTIVATE_ACCOUNT: &str = "/api/v1/user/account?mode=deactivate";

struct Fixture {
    db: TestDb,
    app: axum::Router,
    user: String,
    password: String,
    token: String,
}

async fn setup() -> Fixture {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let user = create_user(&db.pool, &email, &password).await;
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = body["sessionId"].as_str().unwrap().to_string();
    Fixture { db, app, user, password, token }
}

impl Fixture {
    /// Pretend this session started a while ago.
    async fn age_session(&self) {
        let long_ago = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        sqlx::query("UPDATE sessions SET created_at = ? WHERE user_id = ?")
            .bind(long_ago)
            .bind(&self.user)
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    async fn delete(&self, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        call(&self.app, "DELETE", uri, Some(&self.token), body).await
    }

    async fn user_exists(&self) -> bool {
        user_count(&self.db.pool, &self.user).await > 0
    }
}

async fn user_count(pool: &SqlitePool, id: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_old_session_alone_cannot_delete_anything() {
    let f = setup().await;
    f.age_session().await;

    for uri in [NOTES, DELETE_ACCOUNT, DEACTIVATE_ACCOUNT] {
        let (status, body) = f.delete(uri, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(body["code"], json!("password_required"), "{uri}");
    }

    assert!(f.user_exists().await);
}

#[tokio::test]
async fn the_right_password_deletes() {
    let f = setup().await;
    f.age_session().await;

    let (status, _) = f.delete(DELETE_ACCOUNT, Some(json!({ "currentPassword": f.password }))).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!f.user_exists().await);
}

#[tokio::test]
async fn the_right_password_deletes_notes() {
    let f = setup().await;
    let (_, key) = create_device(&f.db.pool, &f.user).await;
    push(&f.app, &key, json!({ "notes": [note(&new_id(), "text", "2026-02-01T00:00:00Z")] })).await;
    f.age_session().await;

    let (status, _) = f.delete(NOTES, Some(json!({ "currentPassword": f.password }))).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let notes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(notes, 0);
}

#[tokio::test]
async fn a_wrong_password_gets_a_401_with_a_body() {
    // A dashboard tells this from an expired session by the body, which the session
    // middleware's own 401 doesn't have
    let f = setup().await;

    for uri in [NOTES, DELETE_ACCOUNT, DEACTIVATE_ACCOUNT] {
        let (status, body) = f.delete(uri, Some(json!({ "currentPassword": random_password() }))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert!(body["error"].is_string(), "{uri}: {body}");
    }

    assert!(f.user_exists().await);
}

#[tokio::test]
async fn a_wrong_password_is_refused_even_on_a_new_session() {
    let f = setup().await;

    let (status, _) = f.delete(DELETE_ACCOUNT, Some(json!({ "currentPassword": random_password() }))).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(f.user_exists().await);
}

#[tokio::test]
async fn guessing_here_is_throttled() {
    let f = setup().await;

    for _ in 0..6 {
        f.delete(NOTES, Some(json!({ "currentPassword": random_password() }))).await;
    }

    let (status, body) = f.delete(NOTES, Some(json!({ "currentPassword": f.password }))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], json!("rate_limited"));
}

#[tokio::test]
async fn a_client_that_has_just_signed_in_needs_no_password_here() {
    // Released iOS clients ask for the password, sign in and delete, in that order,
    // and don't send it again. That sign-in is the proof.
    let f = setup().await;

    let (status, _) = f.delete(DELETE_ACCOUNT, None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(!f.user_exists().await);
}

#[tokio::test]
async fn an_unreadable_body_counts_as_no_password() {
    let f = setup().await;
    f.age_session().await;

    let (status, _, _) = call_full(
        &f.app,
        "DELETE",
        DELETE_ACCOUNT,
        Some(&f.token),
        Some(json!("not an object")),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(f.user_exists().await);
}
