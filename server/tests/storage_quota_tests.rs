// Storage quotas. They were shown and editable, and nothing ever checked them.
// Enforcing them is opt-in: an account can already be over a quota that was never
// enforced, and turning that on by surprise would stop its sync.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::{config::Config, maintenance};
use serde_json::{json, Value};
use sqlx::SqlitePool;

const TIME: &str = "2026-02-01T00:00:00Z";
const MB: usize = 1024 * 1024;

struct Fixture {
    db: TestDb,
    app: axum::Router,
    user: String,
    key: String,
}

async fn fixture(enforce: bool) -> Fixture {
    let db = migrated_db().await;
    let config = Config {
        enforce_storage_quota: enforce,
        ..test_config()
    };
    let app = app_with(&db.pool, config);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    sqlx::query("UPDATE users SET storage_quota_mb = 1, max_upload_size_mb = 50 WHERE id = ?")
        .bind(&user)
        .execute(&db.pool)
        .await
        .unwrap();
    Fixture { db, app, user, key }
}

fn big_note(id: &str, size: usize) -> Value {
    note(id, &"x".repeat(size), TIME)
}

async fn raw_push(f: &Fixture, body: Value) -> (StatusCode, Value) {
    let mut request = json!({ "notes": [], "attachments": [], "versions": [] });
    for (key, value) in body.as_object().unwrap() {
        request[key] = value.clone();
    }
    call(&f.app, "POST", "/api/v1/sync/push", Some(&f.key), Some(request)).await
}

async fn stored_used(pool: &SqlitePool, user: &str) -> i64 {
    sqlx::query_scalar("SELECT storage_used_bytes FROM users WHERE id = ?")
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn note_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notes").fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn quotas_are_not_enforced_unless_asked() {
    let f = fixture(false).await;

    let (status, _) = raw_push(&f, json!({ "notes": [big_note(&new_id(), 2 * MB)] })).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_push_that_would_pass_the_quota_is_refused_whole() {
    let f = fixture(true).await;

    let (status, body) = raw_push(&f, json!({ "notes": [big_note(&new_id(), 2 * MB)] })).await;

    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
    assert_eq!(body["code"], json!("storage_quota_exceeded"));
    assert_eq!(note_count(&f.db.pool).await, 0);
}

#[tokio::test]
async fn a_push_within_the_quota_is_accepted() {
    let f = fixture(true).await;

    let (status, _) = raw_push(&f, json!({ "notes": [big_note(&new_id(), MB / 2)] })).await;

    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn repeated_pushes_count_towards_the_quota() {
    // Each is under the limit for one request, which is all that used to be checked
    let f = fixture(true).await;

    let (first, _) = raw_push(&f, json!({ "notes": [big_note(&new_id(), 600 * 1024)] })).await;
    let (second, _) = raw_push(&f, json!({ "notes": [big_note(&new_id(), 600 * 1024)] })).await;

    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::INSUFFICIENT_STORAGE);
}

#[tokio::test]
async fn versions_and_blobs_count_too() {
    let f = fixture(true).await;
    let id = new_id();

    let (status, _) = raw_push(
        &f,
        json!({
            "notes": [note(&id, "small", TIME)],
            "versions": [version(&id, 1, &"x".repeat(2 * MB))],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);

    let attachment = new_id();
    let (status, _) = raw_push(
        &f,
        json!({
            "notes": [note_with_attachments(&id, "small", TIME, &[&attachment])],
            "attachments": [blob(&attachment, &vec![0; 2 * MB])],
        }),
    )
    .await;
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
}

#[tokio::test]
async fn a_full_account_can_still_make_room() {
    let f = fixture(true).await;
    let (first, second) = (new_id(), new_id());
    raw_push(&f, json!({ "notes": [big_note(&first, 400 * 1024), big_note(&second, 400 * 1024)] })).await;
    sqlx::query("UPDATE users SET storage_quota_mb = 0 WHERE id = ?")
        .bind(&f.user)
        .execute(&f.db.pool)
        .await
        .unwrap();

    // Permanent deletion
    let (status, body) = raw_push(
        &f,
        json!({ "deletions": [{ "id": first, "deletedAt": chrono::Utc::now().to_rfc3339() }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["rejectedDeletions"], json!([]));

    // Moving a note to the bin
    let mut binned = note(&second, "binned", TIME);
    binned["deleted"] = json!(true);
    binned["deletedAt"] = json!(TIME);
    let (status, body) = raw_push(&f, json!({ "notes": [binned] })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn lowering_a_quota_takes_effect() {
    let f = fixture(true).await;
    raw_push(&f, json!({ "notes": [big_note(&new_id(), MB / 2)] })).await;

    sqlx::query("UPDATE users SET storage_quota_mb = 0 WHERE id = ?")
        .bind(&f.user)
        .execute(&f.db.pool)
        .await
        .unwrap();

    let (status, _) = raw_push(&f, json!({ "notes": [note(&new_id(), "tiny", TIME)] })).await;
    assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
}

#[tokio::test]
async fn housekeeping_counts_what_is_really_stored() {
    let f = fixture(false).await;
    let (id, attachment) = (new_id(), new_id());
    push(
        &f.app,
        &f.key,
        json!({
            "notes": [note_with_attachments(&id, "12345", TIME, &[&attachment])],
            "attachments": [blob(&attachment, &[0; 100])],
            "versions": [version(&id, 1, "1234567")],
        }),
    )
    .await;
    sqlx::query("UPDATE users SET storage_used_bytes = 999999 WHERE id = ?")
        .bind(&f.user)
        .execute(&f.db.pool)
        .await
        .unwrap();

    maintenance::run_once(&f.db.pool).await.unwrap();

    // Content, plus "[]" for tags in the note and in the version
    assert_eq!(stored_used(&f.db.pool, &f.user).await, 5 + 2 + 7 + 2 + 100);
}

#[tokio::test]
async fn usage_shown_to_the_user_is_what_is_really_stored() {
    let f = fixture(false).await;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = ?")
        .bind(&f.user)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    let password = random_password();
    let hash = jottery_server::utils::password::hash_password_with_params(&password, 1024, 1, 1).unwrap();
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(&f.user)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let (_, login) = call(
        &f.app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    push(&f.app, &f.key, json!({ "notes": [note(&new_id(), "12345", TIME)] })).await;

    let (_, account) = call(
        &f.app,
        "GET",
        "/api/v1/user/account",
        login["sessionId"].as_str(),
        None,
    )
    .await;

    assert_eq!(account["storageUsedBytes"], json!(7));
}
