// A pull must be bounded whatever the client asks for: one page used to hold every
// blob of every note on it, several times over, and limit=-1 meant no limit at all.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::api::sync::PULL_BLOB_BUDGET_BYTES;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashSet;

const TIME: &str = "2026-02-01T00:00:00Z";

async fn setup() -> (TestDb, axum::Router, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    (db, app, key)
}

/// Give an attachment a blob of `size` bytes without pushing it all.
async fn store_blob(pool: &SqlitePool, attachment: &str, size: usize) {
    sqlx::query(
        "INSERT INTO attachments_data (id, data, created_at)
         SELECT id, zeroblob(?), ? FROM attachments_meta WHERE attachment_id = ?",
    )
    .bind(size as i64)
    .bind(TIME)
    .bind(attachment)
    .execute(pool)
    .await
    .unwrap();
}

/// Notes that each hold one attachment of `size` bytes. Returns their ids in order.
async fn notes_with_blobs(
    db: &TestDb,
    app: &axum::Router,
    key: &str,
    count: usize,
    size: usize,
) -> Vec<String> {
    let mut note_ids = Vec::new();
    for _ in 0..count {
        let (id, attachment) = (new_id(), new_id());
        push(
            app,
            key,
            json!({ "notes": [note_with_attachments(&id, "text", TIME, &[&attachment])] }),
        )
        .await;
        store_blob(&db.pool, &attachment, size).await;
        note_ids.push(id);
    }
    note_ids
}

fn decoded_size(page: &Value) -> usize {
    page["attachments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["data"].as_str().unwrap().len() / 4 * 3)
        .sum()
}

async fn add_notes(app: &axum::Router, key: &str, count: usize) {
    let notes: Vec<Value> = (0..count).map(|_| note(&new_id(), "text", TIME)).collect();
    push(app, key, json!({ "notes": notes })).await;
}

#[tokio::test]
async fn a_negative_limit_does_not_mean_unlimited() {
    let (_db, app, key) = setup().await;
    add_notes(&app, &key, 3).await;

    let page = pull(&app, &key, json!({ "limit": -1 })).await;

    assert_eq!(page["notes"].as_array().unwrap().len(), 1);
    assert_eq!(page["hasMore"], json!(true));
}

#[tokio::test]
async fn a_huge_limit_is_cut_to_the_maximum() {
    let (_db, app, key) = setup().await;
    add_notes(&app, &key, 501).await;

    let page = pull(&app, &key, json!({ "limit": 1_000_000 })).await;

    assert_eq!(page["notes"].as_array().unwrap().len(), 500);
    assert_eq!(page["hasMore"], json!(true));
}

#[tokio::test]
async fn a_negative_offset_starts_at_the_beginning() {
    let (_db, app, key) = setup().await;
    add_notes(&app, &key, 2).await;

    let (status, page) = call(
        &app,
        "POST",
        "/api/v1/sync/pull",
        Some(&key),
        Some(json!({ "knownNoteIds": [], "knownAttachmentIds": [], "limit": 10, "offset": -5 })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["notes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn timestamp_pages_stop_at_the_blob_budget_and_lose_nothing() {
    let (db, app, key) = setup().await;
    let size = PULL_BLOB_BUDGET_BYTES / 2 + 1;
    let all = notes_with_blobs(&db, &app, &key, 3, size).await;

    // Released clients advance by the notes they received, and use hasMore
    let (mut offset, mut seen, mut pages) = (0, HashSet::new(), 0);
    loop {
        let page = pull(&app, &key, json!({ "limit": 100, "offset": offset })).await;
        let notes = page["notes"].as_array().unwrap();
        assert!(!notes.is_empty(), "a client stops at an empty page");
        assert!(decoded_size(&page) <= PULL_BLOB_BUDGET_BYTES + 3, "page too large");
        offset += notes.len();
        seen.extend(notes.iter().map(|n| n["id"].as_str().unwrap().to_string()));
        pages += 1;
        if page["hasMore"] != json!(true) {
            break;
        }
    }

    assert_eq!(seen, all.into_iter().collect::<HashSet<_>>());
    assert!(pages > 1, "the budget should have split the page");
}

#[tokio::test]
async fn cursor_pages_stop_at_the_blob_budget_and_lose_nothing() {
    let (db, app, key) = setup().await;
    let size = PULL_BLOB_BUDGET_BYTES / 2 + 1;
    let all = notes_with_blobs(&db, &app, &key, 3, size).await;

    let (mut cursor, mut seen, mut pages) = (String::new(), HashSet::new(), 0);
    loop {
        let page = pull(&app, &key, json!({ "cursor": cursor, "limit": 100 })).await;
        assert!(decoded_size(&page) <= PULL_BLOB_BUDGET_BYTES + 3, "page too large");
        for note in page["notes"].as_array().unwrap() {
            assert!(
                seen.insert(note["id"].as_str().unwrap().to_string()),
                "a note came twice"
            );
        }
        cursor = page["nextCursor"].as_str().unwrap().to_string();
        pages += 1;
        if page["hasMore"] != json!(true) {
            break;
        }
    }

    assert_eq!(seen, all.into_iter().collect::<HashSet<_>>());
    assert!(pages > 1, "the budget should have split the page");
}

#[tokio::test]
async fn cursor_pages_keep_everything_in_their_range() {
    // A page that stops early must not skip the versions and tombstones that sit
    // after its last note, or claim them before they're delivered.
    let (db, app, key) = setup().await;
    let size = PULL_BLOB_BUDGET_BYTES / 2 + 1;
    let notes = notes_with_blobs(&db, &app, &key, 3, size).await;
    let extra = new_id();
    push(&app, &key, json!({ "notes": [note(&extra, "plain", TIME)] })).await;
    push(
        &app,
        &key,
        json!({ "notes": [note(&extra, "plain", TIME)], "versions": [version(&extra, 1, "plain")] }),
    )
    .await;

    let (mut cursor, mut versions, mut ids) = (String::new(), 0, HashSet::new());
    loop {
        let page = pull(&app, &key, json!({ "cursor": cursor, "limit": 100 })).await;
        versions += page["versions"].as_array().unwrap().len();
        ids.extend(page["notes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap().to_string()));
        cursor = page["nextCursor"].as_str().unwrap().to_string();
        if page["hasMore"] != json!(true) {
            break;
        }
    }

    assert_eq!(versions, 1);
    assert!(ids.contains(&extra));
    assert!(notes.iter().all(|id| ids.contains(id)));
}

#[tokio::test]
async fn one_blob_over_the_budget_still_arrives() {
    let (db, app, key) = setup().await;
    let all = notes_with_blobs(&db, &app, &key, 2, PULL_BLOB_BUDGET_BYTES + 1).await;

    let first = pull(&app, &key, json!({ "cursor": "" })).await;
    assert_eq!(first["notes"].as_array().unwrap().len(), 1);
    assert_eq!(first["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(first["hasMore"], json!(true));

    let second = pull(&app, &key, json!({ "cursor": first["nextCursor"] })).await;
    assert_eq!(second["notes"].as_array().unwrap().len(), 1);
    assert_ne!(first["notes"][0]["id"], second["notes"][0]["id"]);
    assert_eq!(second["hasMore"], json!(false));
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn blobs_the_client_already_has_do_not_count() {
    let (db, app, key) = setup().await;
    let size = PULL_BLOB_BUDGET_BYTES / 2 + 1;
    notes_with_blobs(&db, &app, &key, 3, size).await;
    let known: Vec<String> = sqlx::query_scalar("SELECT attachment_id FROM attachments_meta")
        .fetch_all(&db.pool)
        .await
        .unwrap();

    let page = pull(&app, &key, json!({ "cursor": "", "knownAttachmentIds": known })).await;

    assert_eq!(page["notes"].as_array().unwrap().len(), 3);
    assert_eq!(page["hasMore"], json!(false));
}
