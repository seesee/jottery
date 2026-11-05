// What housekeeping removes, and what it must leave.

mod common;

use common::*;
use jottery_server::maintenance;
use sqlx::SqlitePool;

async fn add_session(pool: &SqlitePool, user_id: &str, expires_at: &str) -> String {
    let id = new_id();
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(new_id())
    .bind("2026-01-01T00:00:00+00:00")
    .bind(expires_at)
    .bind("2026-01-01T00:00:00+00:00")
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn session_exists(pool: &SqlitePool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
        > 0
}

#[tokio::test]
async fn expired_sessions_are_removed_and_live_ones_kept() {
    let db = migrated_db().await;
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let expired = add_session(&db.pool, &user, "2020-01-01T00:00:00+00:00").await;
    let live = add_session(&db.pool, &user, "2999-01-01T00:00:00+00:00").await;

    maintenance::run_once(&db.pool).await.unwrap();

    assert!(!session_exists(&db.pool, &expired).await);
    assert!(session_exists(&db.pool, &live).await);
}

/// A device that signs in as `user`, with a soft-deleted note on the server.
async fn soft_deleted_note(db: &TestDb, app: &axum::Router, deleted_at: &str) -> (String, String) {
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    let id = new_id();
    let mut pushed = note(&id, "binned", "2026-02-01T00:00:00Z");
    pushed["deleted"] = serde_json::json!(true);
    pushed["deletedAt"] = serde_json::json!(deleted_at);
    push(app, &key, serde_json::json!({ "notes": [pushed] })).await;
    (id, key)
}

/// Pretend the server has held this note in the bin for a long time.
async fn age_on_server(pool: &SqlitePool, id: &str) {
    sqlx::query("UPDATE notes SET server_modified_at = '2020-01-01T00:00:00+00:00' WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn note_exists(pool: &SqlitePool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM notes WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
        > 0
}

#[tokio::test]
async fn purging_a_note_tells_devices_that_missed_the_soft_delete() {
    // A device offline while the note sat in the bin still shows it as live, and
    // with nothing to tell it otherwise, its next edit would bring the note back.
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, key) = soft_deleted_note(&db, &app, "2026-02-01T00:00:00Z").await;
    age_on_server(&db.pool, &id).await;
    let before = pull(&app, &key, serde_json::json!({ "cursor": "" })).await;

    maintenance::run_once(&db.pool).await.unwrap();

    assert!(!note_exists(&db.pool, &id).await);

    let by_cursor = pull(&app, &key, serde_json::json!({ "cursor": before["nextCursor"] })).await;
    assert_eq!(by_cursor["deletions"][0]["id"], serde_json::json!(id));
    assert_eq!(by_cursor["deletions"][0]["deletedAt"], serde_json::json!("2026-02-01T00:00:00Z"));

    let by_time = pull(&app, &key, serde_json::json!({ "lastSyncAt": before["syncedAt"] })).await;
    assert_eq!(by_time["deletions"][0]["id"], serde_json::json!(id));
}

#[tokio::test]
async fn an_edit_made_after_the_deletion_still_brings_a_purged_note_back() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, key) = soft_deleted_note(&db, &app, "2026-02-01T00:00:00Z").await;
    age_on_server(&db.pool, &id).await;
    maintenance::run_once(&db.pool).await.unwrap();

    let response = push(
        &app,
        &key,
        serde_json::json!({ "notes": [note(&id, "restored", "2026-03-01T00:00:00Z")] }),
    )
    .await;

    assert_eq!(response["accepted"][0]["id"], serde_json::json!(id));
    assert!(note_exists(&db.pool, &id).await);
}

#[tokio::test]
async fn a_client_clock_cannot_rush_the_purge() {
    // deletedAt comes from the client. A note that arrives with an old one still
    // gets its 30 days on the server, for other devices to see it.
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, _) = soft_deleted_note(&db, &app, "2000-01-01T00:00:00Z").await;

    maintenance::run_once(&db.pool).await.unwrap();

    assert!(note_exists(&db.pool, &id).await);
}

#[tokio::test]
async fn a_binned_note_with_no_deletion_time_is_still_purged() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, key) = soft_deleted_note(&db, &app, "2026-02-01T00:00:00Z").await;
    sqlx::query("UPDATE notes SET deleted_at = NULL WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    age_on_server(&db.pool, &id).await;

    maintenance::run_once(&db.pool).await.unwrap();

    assert!(!note_exists(&db.pool, &id).await);
    let pulled = pull(&app, &key, serde_json::json!({ "cursor": "" })).await;
    assert_eq!(pulled["deletions"][0]["id"], serde_json::json!(id));
}

#[tokio::test]
async fn live_notes_are_not_purged() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    let id = new_id();
    push(&app, &key, serde_json::json!({ "notes": [note(&id, "kept", "2026-02-01T00:00:00Z")] })).await;
    age_on_server(&db.pool, &id).await;

    maintenance::run_once(&db.pool).await.unwrap();

    assert!(note_exists(&db.pool, &id).await);
}

#[tokio::test]
async fn deleting_through_the_notes_route_leaves_a_tombstone() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    let id = new_id();
    push(&app, &key, serde_json::json!({ "notes": [note(&id, "gone", "2026-02-01T00:00:00Z")] })).await;
    let before = pull(&app, &key, serde_json::json!({ "cursor": "" })).await;

    let (status, _) = call(&app, "DELETE", &format!("/api/v1/sync/notes/{id}"), Some(&key), None).await;

    assert_eq!(status, axum::http::StatusCode::NO_CONTENT);
    let after = pull(&app, &key, serde_json::json!({ "cursor": before["nextCursor"] })).await;
    assert_eq!(after["deletions"][0]["id"], serde_json::json!(id));
}

async fn last_seen(pool: &SqlitePool, device: &str) -> String {
    sqlx::query_scalar("SELECT last_seen_at FROM clients WHERE id = ?")
        .bind(device)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn set_last_seen(pool: &SqlitePool, device: &str, value: &str) {
    sqlx::query("UPDATE clients SET last_seen_at = ? WHERE id = ?")
        .bind(value)
        .bind(device)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_device_is_not_written_to_on_every_request() {
    // Every authenticated request used to write, and SQLite has one write lock
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (device, key) = create_device(&db.pool, &user).await;
    let moments_ago = chrono::Utc::now().to_rfc3339();
    set_last_seen(&db.pool, &device, &moments_ago).await;

    call(&app, "GET", "/api/v1/sync/status", Some(&key), None).await;

    assert_eq!(last_seen(&db.pool, &device).await, moments_ago);
}

#[tokio::test]
async fn a_device_is_seen_again_after_a_while() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (device, key) = create_device(&db.pool, &user).await;
    set_last_seen(&db.pool, &device, "2026-01-01T00:00:00+00:00").await;

    call(&app, "GET", "/api/v1/sync/status", Some(&key), None).await;

    assert_ne!(last_seen(&db.pool, &device).await, "2026-01-01T00:00:00+00:00");
}

async fn last_used(pool: &SqlitePool, id: &str) -> String {
    sqlx::query_scalar("SELECT last_used_at FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_session_records_its_use_at_most_once_a_minute() {
    let db = migrated_db().await;
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let token = new_id();
    let session = jottery_server::db::SessionRepository::create(
        &db.pool,
        jottery_server::models::CreateSessionParams {
            user_id: user,
            token: token.clone(),
            expires_at: "2999-01-01T00:00:00+00:00".to_string(),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .unwrap();
    let validate = || jottery_server::db::SessionRepository::validate_and_get(&db.pool, &token);

    validate().await.unwrap().unwrap();
    assert_eq!(last_used(&db.pool, &session.id).await, session.last_used_at);

    sqlx::query("UPDATE sessions SET last_used_at = '2026-01-01T00:00:00+00:00' WHERE id = ?")
        .bind(&session.id)
        .execute(&db.pool)
        .await
        .unwrap();
    validate().await.unwrap().unwrap();
    assert_ne!(last_used(&db.pool, &session.id).await, "2026-01-01T00:00:00+00:00");
}
