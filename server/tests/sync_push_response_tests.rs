// What a push tells the client, and the devices around it.

mod common;

use common::*;
use serde_json::{json, Value};

const EARLIER: &str = "2026-02-01T00:00:00Z";
const LATER: &str = "2026-03-01T00:00:00Z";

async fn setup() -> (TestDb, axum::Router, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    (db, app, key)
}

fn with_hashes(id: &str, content: &str, time: &str, parent: &str) -> Value {
    let mut value = note(id, content, time);
    value["contentHash"] = json!(format!("hash-{content}"));
    value["parentHash"] = json!(parent);
    value["hashChain"] = json!([format!("hash-{content}"), parent]);
    value
}

#[tokio::test]
async fn a_push_skipped_because_of_a_tombstone_says_so() {
    // Without this, the note is in neither list and the client re-sends it for ever
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "text", EARLIER)] })).await;
    push(&app, &key, json!({ "deletions": [{ "id": id, "deletedAt": LATER }] })).await;

    let response = push(&app, &key, json!({ "notes": [note(&id, "stale", EARLIER)] })).await;

    assert_eq!(response["skippedDeleted"], json!([id]));
    assert_eq!(response["accepted"], json!([]));
    assert_eq!(response["rejected"], json!([]));
}

#[tokio::test]
async fn the_list_is_always_there() {
    let (_db, app, key) = setup().await;

    let response = push(&app, &key, json!({ "notes": [note(&new_id(), "text", EARLIER)] })).await;

    assert_eq!(response["skippedDeleted"], json!([]));
}

#[tokio::test]
async fn an_edit_that_beats_the_tombstone_is_not_skipped() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "text", EARLIER)] })).await;
    push(&app, &key, json!({ "deletions": [{ "id": id, "deletedAt": EARLIER }] })).await;

    let response = push(&app, &key, json!({ "notes": [note(&id, "back", LATER)] })).await;

    assert_eq!(response["skippedDeleted"], json!([]));
    assert_eq!(ids(&response["accepted"]), vec![id]);
}

#[tokio::test]
async fn a_conflict_says_whether_the_server_copy_is_in_the_bin() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    let mut binned = with_hashes(&id, "binned", LATER, "hash-root");
    binned["deleted"] = json!(true);
    binned["deletedAt"] = json!(LATER);
    push(&app, &key, json!({ "notes": [binned] })).await;

    let response = push(
        &app,
        &key,
        json!({ "notes": [with_hashes(&id, "diverged", EARLIER, "hash-other")] }),
    )
    .await;

    let rejected = &response["rejected"][0];
    assert_eq!(rejected["id"], json!(id));
    assert_eq!(rejected["serverDeleted"], json!(true));
    assert_eq!(rejected["serverDeletedAt"], json!(LATER));
}

#[tokio::test]
async fn a_conflict_with_a_live_note_says_that_too() {
    let (_db, app, key) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [with_hashes(&id, "live", LATER, "hash-root")] })).await;

    let response = push(
        &app,
        &key,
        json!({ "notes": [with_hashes(&id, "diverged", EARLIER, "hash-other")] }),
    )
    .await;

    let rejected = &response["rejected"][0];
    assert_eq!(rejected["serverDeleted"], json!(false));
    assert_eq!(rejected["serverDeletedAt"], Value::Null);
}
