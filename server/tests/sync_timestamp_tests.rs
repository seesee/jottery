// Client timestamps decide which edit wins, so a wrong clock or a malformed value
// must not be able to win. Times are stored as sent, because clients read them back,
// but compared as instants.

mod common;

use common::*;
use serde_json::{json, Value};

const TIME: &str = "2026-02-01T00:00:00Z";

async fn setup() -> (TestDb, axum::Router, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    (db, app, key)
}

fn hours_from_now(hours: i64) -> String {
    (chrono::Utc::now() + chrono::Duration::hours(hours)).to_rfc3339()
}

fn accepted(response: &Value) -> Vec<String> {
    ids(&response["accepted"])
}

#[tokio::test]
async fn a_note_with_an_unreadable_time_is_skipped_and_the_rest_still_sync() {
    let (_db, app, key) = setup().await;
    let (bad, good) = (new_id(), new_id());

    let response = push(
        &app,
        &key,
        json!({ "notes": [note(&bad, "bad", "yesterday-ish"), note(&good, "good", TIME)] }),
    )
    .await;

    assert_eq!(accepted(&response), vec![good.clone()]);
    assert_eq!(response["errors"].as_array().unwrap().len(), 1);
    assert!(response["errors"][0].as_str().unwrap().contains(&bad));
    assert_eq!(ids(&pull_all(&app, &key).await["notes"]), vec![good]);
}

#[tokio::test]
async fn every_time_on_a_note_is_checked() {
    let (_db, app, key) = setup().await;

    for field in ["createdAt", "modifiedAt", "deletedAt", "archivedAt", "lockedAt"] {
        let mut bad = note(&new_id(), "text", TIME);
        bad[field] = json!("not a time");

        let response = push(&app, &key, json!({ "notes": [bad] })).await;

        assert!(accepted(&response).is_empty(), "{field}");
    }
}

#[tokio::test]
async fn an_empty_optional_time_is_the_same_as_none() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    let mut pushed = note(&id, "text", TIME);
    pushed["archivedAt"] = json!("");

    let response = push(&app, &key, json!({ "notes": [pushed] })).await;

    assert_eq!(accepted(&response), vec![id]);
}

#[tokio::test]
async fn every_format_clients_send_is_readable() {
    let (_db, app, key) = setup().await;

    for time in [
        "2026-02-01T10:00:00Z",
        "2026-02-01T10:00:00.123Z",
        "2026-02-01T10:00:00.123456789Z",
        "2026-02-01T10:00:00+00:00",
        "2026-02-01T10:00:00.123456789+00:00",
        "2026-02-01T10:00:00-05:00",
    ] {
        let id = new_id();
        let response = push(&app, &key, json!({ "notes": [note(&id, "text", time)] })).await;
        assert_eq!(accepted(&response), vec![id], "{time}");
    }
}

#[tokio::test]
async fn times_are_stored_as_sent() {
    // Clients read these back, and some compare them as text
    let (_db, app, key) = setup().await;
    let id = new_id();
    let time = "2026-02-01T10:00:00.123Z";

    push(&app, &key, json!({ "notes": [note(&id, "text", time)] })).await;

    assert_eq!(pull_all(&app, &key).await["notes"][0]["modifiedAt"], json!(time));
}

#[tokio::test]
async fn a_clock_far_in_the_future_is_refused() {
    let (_db, app, key) = setup().await;
    let id = new_id();

    let response = push(&app, &key, json!({ "notes": [note(&id, "text", &hours_from_now(25))] })).await;

    assert!(accepted(&response).is_empty());
    assert_eq!(response["errors"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_clock_a_little_ahead_is_tolerated() {
    let (_db, app, key) = setup().await;
    let id = new_id();

    let response = push(&app, &key, json!({ "notes": [note(&id, "text", &hours_from_now(23))] })).await;

    assert_eq!(accepted(&response), vec![id]);
}

#[tokio::test]
async fn a_future_clock_cannot_undo_a_deletion() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "text", TIME)] })).await;
    push(&app, &key, json!({ "deletions": [{ "id": id, "deletedAt": "2026-03-01T00:00:00Z" }] })).await;

    let response = push(&app, &key, json!({ "notes": [note(&id, "stale", &hours_from_now(24 * 365))] })).await;

    assert!(accepted(&response).is_empty());
    assert!(ids(&pull_all(&app, &key).await["notes"]).is_empty());
}

#[tokio::test]
async fn a_deletion_from_a_future_clock_does_not_beat_every_edit() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "text", TIME)] })).await;

    let response = push(
        &app,
        &key,
        json!({ "deletions": [{ "id": id, "deletedAt": hours_from_now(24 * 365) }] }),
    )
    .await;

    assert_eq!(response["rejectedDeletions"], json!([id]));
    assert_eq!(ids(&pull_all(&app, &key).await["notes"]), vec![id]);
}

#[tokio::test]
async fn timestamps_are_compared_as_instants_not_as_text() {
    // As text, "09:30:00Z" sorts before "10:00:00+01:00", which is half an hour earlier
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "older", "2026-02-01T10:00:00+01:00")] })).await;

    let response = push(&app, &key, json!({ "notes": [note(&id, "newer", "2026-02-01T09:30:00Z")] })).await;

    assert_eq!(accepted(&response), vec![id]);
    assert_eq!(pull_all(&app, &key).await["notes"][0]["content"], json!("newer"));
}

#[tokio::test]
async fn an_older_edit_is_still_refused_whatever_its_format() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "newer", "2026-02-01T09:30:00Z")] })).await;

    let response = push(&app, &key, json!({ "notes": [note(&id, "older", "2026-02-01T10:00:00+01:00")] })).await;

    assert!(accepted(&response).is_empty());
    assert_eq!(ids(&response["rejected"]), vec![id]);
}

#[tokio::test]
async fn a_stored_time_that_cannot_be_read_falls_back_to_comparing_text() {
    let (db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "first", TIME)] })).await;
    sqlx::query("UPDATE notes SET modified_at = '2020-garbage'")
        .execute(&db.pool)
        .await
        .unwrap();

    let response = push(&app, &key, json!({ "notes": [note(&id, "second", TIME)] })).await;

    assert_eq!(accepted(&response), vec![id]);
}

#[tokio::test]
async fn saved_searches_follow_the_same_rules() {
    let (db, app, key) = setup().await;
    let (search, bad) = (new_id(), new_id());
    push(
        &app,
        &key,
        json!({ "savedSearches": [saved_search(&search, "old", "2026-02-01T10:00:00+01:00")] }),
    )
    .await;

    let response = push(
        &app,
        &key,
        json!({ "savedSearches": [
            saved_search(&search, "new", "2026-02-01T09:30:00Z"),
            saved_search(&bad, "bad", &hours_from_now(48)),
        ] }),
    )
    .await;

    assert_eq!(response["errors"].as_array().unwrap().len(), 1);
    let name: String = sqlx::query_scalar("SELECT name FROM saved_searches WHERE id = ?")
        .bind(&search)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(name, "new");
    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM saved_searches WHERE id = ?")
        .bind(&bad)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(stored, 0);
}
