// Removing an attachment, and when its blob is finally deleted.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::maintenance;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

const EARLIER: &str = "2026-02-01T00:00:00Z";
const LATER: &str = "2026-06-01T00:00:00Z";
const LATEST: &str = "2026-09-01T00:00:00Z";

async fn setup() -> (TestDb, axum::Router, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    (db, app, key)
}

async fn push_with_blob(app: &axum::Router, key: &str, note_id: &str, attachment: &str) {
    push(
        app,
        key,
        json!({
            "notes": [note_with_attachments(note_id, "a", EARLIER, &[attachment])],
            "attachments": [blob(attachment, b"data")],
        }),
    )
    .await;
}

fn attachment_ids(pulled: &Value, note_id: &str) -> Vec<String> {
    let note = pulled["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == note_id)
        .unwrap_or_else(|| panic!("note {note_id} not in pull"));
    ids(&note["attachments"])
}

async fn stored_blobs(pool: &SqlitePool) -> i64 {
    sqlx::query("SELECT COUNT(*) FROM attachments_data")
        .fetch_one(pool)
        .await
        .unwrap()
        .get(0)
}

/// Pretend the grace period has passed for everything that's waiting.
async fn expire_grace(pool: &SqlitePool) {
    sqlx::query("UPDATE attachments_meta SET unreferenced_at = '2000-01-01T00:00:00Z' WHERE unreferenced_at IS NOT NULL")
        .execute(pool)
        .await
        .unwrap();
}

/// A housekeeping pass, the grace period passing, then another pass.
async fn run_purge(pool: &SqlitePool) {
    maintenance::run_once(pool).await.unwrap();
    expire_grace(pool).await;
    maintenance::run_once(pool).await.unwrap();
}

#[tokio::test]
async fn an_authoritative_push_removes_the_attachment() {
    let (_db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push_with_blob(&app, &key, &id, &attachment).await;

    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;

    let pulled = pull_all(&app, &key).await;
    assert_eq!(attachment_ids(&pulled, &id), Vec::<String>::new());
    assert_eq!(pulled["attachments"], json!([]), "the blob must not be sent either");
}

#[tokio::test]
async fn an_authoritative_push_keeps_what_it_lists() {
    let (_db, app, key) = setup().await;
    let (id, removed, kept) = (new_id(), new_id(), new_id());
    push(
        &app,
        &key,
        json!({
            "notes": [note_with_attachments(&id, "a", EARLIER, &[&removed, &kept])],
            "attachments": [blob(&removed, b"gone"), blob(&kept, b"kept")],
        }),
    )
    .await;

    push(
        &app,
        &key,
        json!({
            "notes": [note_with_attachments(&id, "a", LATER, &[&kept])],
            "attachmentsAuthoritative": true,
        }),
    )
    .await;

    assert_eq!(attachment_ids(&pull_all(&app, &key).await, &id), vec![kept]);
}

#[tokio::test]
async fn without_the_flag_an_empty_list_removes_nothing() {
    // The TUI bug in jottery-xjzo sends empty lists from released clients.
    let (_db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push_with_blob(&app, &key, &id, &attachment).await;

    push(&app, &key, json!({ "notes": [note(&id, "a", LATER)] })).await;

    let pulled = pull_all(&app, &key).await;
    assert_eq!(attachment_ids(&pulled, &id), vec![attachment.clone()]);
    assert_eq!(ids(&pulled["attachments"]), vec![attachment]);
}

#[tokio::test]
async fn a_rejected_note_prunes_nothing() {
    let (_db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push(
        &app,
        &key,
        json!({
            "notes": [note_with_attachments(&id, "a", LATER, &[&attachment])],
            "attachments": [blob(&attachment, b"data")],
        }),
    )
    .await;

    let response = push(
        &app,
        &key,
        json!({ "notes": [note(&id, "stale", EARLIER)], "attachmentsAuthoritative": true }),
    )
    .await;
    assert_eq!(response["rejected"][0]["id"], id.as_str());

    assert_eq!(attachment_ids(&pull_all(&app, &key).await, &id), vec![attachment]);
}

#[tokio::test]
async fn removal_reaches_other_devices() {
    let (db, app, key) = setup().await;
    let user: String = sqlx::query("SELECT user_id FROM clients LIMIT 1")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    let (_, other) = create_device(&db.pool, &user).await;
    let (id, attachment) = (new_id(), new_id());
    push_with_blob(&app, &key, &id, &attachment).await;

    let first = pull(&app, &other, json!({ "cursor": "" })).await;
    assert_eq!(attachment_ids(&first, &id), vec![attachment]);
    let cursor = first["nextCursor"].as_str().unwrap().to_string();

    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;

    let next = pull(&app, &other, json!({ "cursor": cursor })).await;
    assert_eq!(attachment_ids(&next, &id), Vec::<String>::new());
}

#[tokio::test]
async fn the_blob_is_deleted_after_the_grace_period_and_not_before() {
    let (db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push_with_blob(&app, &key, &id, &attachment).await;
    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;

    maintenance::run_once(&db.pool).await.unwrap();
    assert_eq!(stored_blobs(&db.pool).await, 1, "still inside the grace period");

    expire_grace(&db.pool).await;
    maintenance::run_once(&db.pool).await.unwrap();
    assert_eq!(stored_blobs(&db.pool).await, 0);

    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/v1/sync/attachments/{attachment}"),
        Some(&key),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_blob_is_kept_while_a_version_snapshot_references_it() {
    let (db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());

    let mut snapshot = version(&id, 1, "old");
    snapshot["attachments"] = json!([attachment_ref(&attachment)]);
    push(
        &app,
        &key,
        json!({
            "notes": [note_with_attachments(&id, "a", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"data")],
            "versions": [snapshot],
        }),
    )
    .await;
    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;

    run_purge(&db.pool).await;
    assert_eq!(stored_blobs(&db.pool).await, 1, "restoring the version needs the blob");

    let (status, body) = call(
        &app,
        "GET",
        &format!("/api/v1/sync/attachments/{attachment}"),
        Some(&key),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"].is_string());

    // Once the version is gone, so is the reason to keep it
    sqlx::query("DELETE FROM note_versions").execute(&db.pool).await.unwrap();
    run_purge(&db.pool).await;
    assert_eq!(stored_blobs(&db.pool).await, 0);
}

#[tokio::test]
async fn a_blob_is_kept_while_another_note_references_it() {
    let (db, app, key) = setup().await;
    let (first, second, attachment) = (new_id(), new_id(), new_id());
    push(
        &app,
        &key,
        json!({
            "notes": [
                note_with_attachments(&first, "a", EARLIER, &[&attachment]),
                note_with_attachments(&second, "b", EARLIER, &[&attachment]),
            ],
            "attachments": [blob(&attachment, b"data")],
        }),
    )
    .await;

    push(
        &app,
        &key,
        json!({ "notes": [note(&first, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;

    run_purge(&db.pool).await;
    assert_eq!(stored_blobs(&db.pool).await, 1);
    assert_eq!(attachment_ids(&pull_all(&app, &key).await, &second), vec![attachment]);
}

#[tokio::test]
async fn relinking_during_the_grace_period_keeps_the_blob() {
    let (db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push_with_blob(&app, &key, &id, &attachment).await;
    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;
    maintenance::run_once(&db.pool).await.unwrap();

    // A device that hadn't caught up pushes the reference again
    push(
        &app,
        &key,
        json!({ "notes": [note_with_attachments(&id, "a", LATEST, &[&attachment])] }),
    )
    .await;

    expire_grace(&db.pool).await;
    maintenance::run_once(&db.pool).await.unwrap();
    assert_eq!(stored_blobs(&db.pool).await, 1);
    let pending: i64 = sqlx::query("SELECT COUNT(*) FROM attachments_meta WHERE unreferenced_at IS NOT NULL")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(pending, 0, "relinking must stop the clock");
}

#[tokio::test]
async fn hard_deleting_a_note_frees_its_blob() {
    let (db, app, key) = setup().await;
    let (id, attachment) = (new_id(), new_id());
    push_with_blob(&app, &key, &id, &attachment).await;

    push(&app, &key, json!({ "deletions": [{ "id": id, "deletedAt": LATEST }] })).await;

    run_purge(&db.pool).await;
    assert_eq!(stored_blobs(&db.pool).await, 0);
}

#[tokio::test]
async fn housekeeping_does_not_touch_another_users_copy_of_an_id() {
    let (db, app, key) = setup().await;
    let other = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, other_key) = create_device(&db.pool, &other).await;
    let (id, other_note, attachment) = (new_id(), new_id(), new_id());

    push_with_blob(&app, &key, &id, &attachment).await;
    push_with_blob(&app, &other_key, &other_note, &attachment).await;
    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "a", LATER)], "attachmentsAuthoritative": true }),
    )
    .await;

    run_purge(&db.pool).await;

    assert_eq!(stored_blobs(&db.pool).await, 1);
    let pulled = pull_all(&app, &other_key).await;
    assert_eq!(attachment_ids(&pulled, &other_note), vec![attachment.clone()]);
    assert_eq!(ids(&pulled["attachments"]), vec![attachment]);
}

/// Combining notes reuses the originals' attachment ids and bins the originals, so
/// the combined note has to hold its own claim on each attachment.
#[tokio::test]
async fn a_combined_note_keeps_attachments_it_shares_with_an_original() {
    let (db, app, key) = setup().await;
    let (original, combined, attachment) = (new_id(), new_id(), new_id());
    push_with_blob(&app, &key, &original, &attachment).await;

    // The client sends no blob here: the server already has it
    push(
        &app,
        &key,
        json!({ "notes": [note_with_attachments(&combined, "both", LATER, &[&attachment])] }),
    )
    .await;

    let pulled = pull_all(&app, &key).await;
    assert_eq!(attachment_ids(&pulled, &combined), vec![attachment.clone()]);
    assert_eq!(ids(&pulled["attachments"]), vec![attachment.clone()]);

    // Emptying the bin afterwards takes the original and leaves the blob
    let deleted = push(
        &app,
        &key,
        json!({ "deletions": [{ "id": original, "deletedAt": chrono::Utc::now().to_rfc3339() }] }),
    )
    .await;
    assert_eq!(deleted["rejectedDeletions"], json!([]));
    run_purge(&db.pool).await;

    assert_eq!(stored_blobs(&db.pool).await, 1);
    let pulled = pull_all(&app, &key).await;
    assert_eq!(attachment_ids(&pulled, &combined), vec![attachment.clone()]);
    assert_eq!(ids(&pulled["attachments"]), vec![attachment]);
}
