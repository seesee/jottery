// An edit beats a deletion it postdates, whichever reaches the server first.

mod common;

use common::*;
use serde_json::json;
use sqlx::Row;

const FEBRUARY: &str = "2026-02-01T00:00:00Z";
const JUNE: &str = "2026-06-01T00:00:00Z";
const JULY: &str = "2026-07-01T00:00:00Z";

async fn setup() -> (TestDb, axum::Router, String, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key_a) = create_device(&db.pool, &user).await;
    let (_, key_b) = create_device(&db.pool, &user).await;
    (db, app, key_a, key_b)
}

#[tokio::test]
async fn a_stale_deletion_does_not_destroy_a_newer_edit() {
    let (db, app, key_a, key_b) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push(
        &app,
        &key_b,
        json!({
            "notes": [note_with_attachments(&id, "edited in june", JUNE, &[&attachment])],
            "attachments": [blob(&attachment, b"data")],
            "versions": [version(&id, 1, "edited in june")],
        }),
    )
    .await;

    let response = push(
        &app,
        &key_a,
        json!({ "deletions": [{ "id": id, "deletedAt": FEBRUARY }] }),
    )
    .await;
    assert_eq!(response["rejectedDeletions"], json!([id]));

    let pulled = pull_all(&app, &key_b).await;
    assert_eq!(ids(&pulled["notes"]), vec![id.clone()]);
    assert_eq!(pulled["notes"][0]["content"], "edited in june");
    assert_eq!(pulled["versions"].as_array().unwrap().len(), 1);
    assert_eq!(pulled["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(pulled["deletions"], json!([]), "no tombstone may reach other devices");

    let tombstones: i64 = sqlx::query("SELECT COUNT(*) FROM note_deletions")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(tombstones, 0);
}

#[tokio::test]
async fn a_newer_deletion_still_deletes() {
    let (_db, app, key_a, key_b) = setup().await;
    let id = new_id();
    push(&app, &key_b, json!({ "notes": [note(&id, "a", JUNE)] })).await;

    let response = push(&app, &key_a, json!({ "deletions": [{ "id": id, "deletedAt": JULY }] })).await;

    assert_eq!(response["rejectedDeletions"], json!([]));
    let pulled = pull_all(&app, &key_b).await;
    assert_eq!(pulled["notes"], json!([]));
    assert_eq!(ids(&pulled["deletions"]), vec![id]);
}

#[tokio::test]
async fn a_deletion_at_the_same_instant_as_the_edit_deletes() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "a", JUNE)] })).await;

    let response = push(&app, &key, json!({ "deletions": [{ "id": id, "deletedAt": JUNE }] })).await;

    assert_eq!(response["rejectedDeletions"], json!([]));
    assert_eq!(pull_all(&app, &key).await["notes"], json!([]));
}

#[tokio::test]
async fn times_are_compared_as_instants_not_text() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    // Noon in London, written so that it sorts after the deletion as text
    push(&app, &key, json!({ "notes": [note(&id, "a", "2026-06-01T20:00:00+08:00")] })).await;

    // An hour later, but sorts first as text
    let response = push(
        &app,
        &key,
        json!({ "deletions": [{ "id": id, "deletedAt": "2026-06-01T13:00:00Z" }] }),
    )
    .await;

    assert_eq!(response["rejectedDeletions"], json!([]));
    assert_eq!(pull_all(&app, &key).await["notes"], json!([]));
}

#[tokio::test]
async fn an_edit_is_compared_with_a_tombstone_as_an_instant_too() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "a", FEBRUARY)] })).await;
    push(&app, &key, json!({ "deletions": [{ "id": id, "deletedAt": "2026-06-01T13:00:00Z" }] })).await;

    // Sorts after the tombstone as text, but is an hour older
    push(&app, &key, json!({ "notes": [note(&id, "stale", "2026-06-01T20:00:00+08:00")] })).await;
    assert_eq!(pull_all(&app, &key).await["notes"], json!([]), "stale edit must stay deleted");

    // Sorts before the tombstone as text, but is an hour newer
    push(&app, &key, json!({ "notes": [note(&id, "newer", "2026-06-01T10:00:00-04:00")] })).await;
    assert_eq!(ids(&pull_all(&app, &key).await["notes"]), vec![id]);
}

#[tokio::test]
async fn a_time_that_cannot_be_read_keeps_the_note() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "a", JUNE)] })).await;

    let response = push(
        &app,
        &key,
        json!({ "deletions": [{ "id": id, "deletedAt": "not a time" }] }),
    )
    .await;

    assert_eq!(response["rejectedDeletions"], json!([id]));
    assert_eq!(ids(&pull_all(&app, &key).await["notes"]), vec![id]);
}

#[tokio::test]
async fn a_refused_deletion_returns_the_note_to_the_device_that_deleted_it() {
    let (_db, app, key_a, key_b) = setup().await;
    let id = new_id();
    push(&app, &key_b, json!({ "notes": [note(&id, "a", JUNE)] })).await;

    // Device A is fully caught up, and its copy of the note is then gone
    let cursor = pull(&app, &key_a, json!({ "cursor": "" })).await["nextCursor"]
        .as_str()
        .unwrap()
        .to_string();

    push(&app, &key_a, json!({ "deletions": [{ "id": id, "deletedAt": FEBRUARY }] })).await;

    let next = pull(&app, &key_a, json!({ "cursor": cursor })).await;
    assert_eq!(ids(&next["notes"]), vec![id]);
}

#[tokio::test]
async fn deleting_a_note_the_server_lacks_is_not_a_rejection() {
    let (_db, app, key, _) = setup().await;
    let response = push(
        &app,
        &key,
        json!({ "deletions": [{ "id": new_id(), "deletedAt": FEBRUARY }] }),
    )
    .await;
    assert_eq!(response["rejectedDeletions"], json!([]));
}

#[tokio::test]
async fn a_push_with_no_deletions_reports_an_empty_list() {
    // Its presence is how a client knows the server applies this rule.
    let (_db, app, key, _) = setup().await;
    let response = push(&app, &key, json!({})).await;
    assert_eq!(response["rejectedDeletions"], json!([]));
}
