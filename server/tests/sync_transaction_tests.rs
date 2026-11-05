// A push is all or nothing, and concurrent pushes and pulls can't see or
// produce a half-applied state. These run on a file database with real
// parallelism, because that's where the races are.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::maintenance;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashSet;

const EARLIER: &str = "2026-02-01T00:00:00Z";
const LATER: &str = "2026-06-01T00:00:00Z";

async fn setup() -> (TestDb, axum::Router, String, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key_a) = create_device(&db.pool, &user).await;
    let (_, key_b) = create_device(&db.pool, &user).await;
    (db, app, key_a, key_b)
}

#[tokio::test]
async fn an_invalid_note_writes_nothing_at_all() {
    let (db, app, key, _) = setup().await;
    let (good, tombstoned) = (new_id(), new_id());

    push(&app, &key, json!({ "notes": [note(&tombstoned, "doomed", EARLIER)] })).await;
    push(&app, &key, json!({ "deletions": [{ "id": tombstoned, "deletedAt": EARLIER }] })).await;

    // Used to commit the first note and clear the tombstone before failing
    let oversized = "x".repeat(10_485_761);
    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/sync/push",
        Some(&key),
        Some(json!({
            "notes": [
                note(&good, "fine", LATER),
                note(&tombstoned, "resurrected", LATER),
                note(&new_id(), &oversized, LATER),
            ],
            "attachments": [],
            "versions": [],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let notes: i64 = sqlx::query("SELECT COUNT(*) FROM notes").fetch_one(&db.pool).await.unwrap().get(0);
    assert_eq!(notes, 0);
    let tombstones: i64 = sqlx::query("SELECT COUNT(*) FROM note_deletions").fetch_one(&db.pool).await.unwrap().get(0);
    assert_eq!(tombstones, 1, "the tombstone must survive a failed push");
}

#[tokio::test]
async fn a_database_error_part_way_rolls_the_whole_push_back() {
    let (db, app, key, _) = setup().await;
    sqlx::query(
        "CREATE TRIGGER fail_on_marker BEFORE INSERT ON notes WHEN NEW.content = 'fail here'
         BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    let (first, attachment) = (new_id(), new_id());
    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/sync/push",
        Some(&key),
        Some(json!({
            "notes": [
                note_with_attachments(&first, "written first", EARLIER, &[&attachment]),
                note(&new_id(), "fail here", EARLIER),
            ],
            "attachments": [blob(&attachment, b"data")],
            "versions": [],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

    for table in ["notes", "attachments_meta", "attachments_data", "note_attachments"] {
        let rows: i64 = sqlx::query(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(rows, 0, "{table} kept rows from a failed push");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_pull_never_sees_an_attachment_reference_without_its_blob() {
    let (_db, app, pusher, puller) = setup().await;

    let notes: Vec<(String, String)> = (0..120).map(|_| (new_id(), new_id())).collect();
    let body = json!({
        "notes": notes.iter()
            .map(|(id, attachment)| note_with_attachments(id, "with attachment", EARLIER, &[attachment]))
            .collect::<Vec<_>>(),
        "attachments": notes.iter()
            .map(|(_, attachment)| blob(attachment, b"data"))
            .collect::<Vec<_>>(),
    });

    let writer_app = app.clone();
    let writer = tokio::spawn(async move { push(&writer_app, &pusher, body).await });

    let mut saw_notes = false;
    while !writer.is_finished() || !saw_notes {
        let pulled = pull_all(&app, &puller).await;
        let blobs: HashSet<String> = ids(&pulled["attachments"]).into_iter().collect();
        for pulled_note in pulled["notes"].as_array().unwrap() {
            saw_notes = true;
            for attachment in pulled_note["attachments"].as_array().unwrap() {
                let id = attachment["id"].as_str().unwrap();
                assert!(blobs.contains(id), "reference to {id} arrived before its blob");
            }
        }
    }
    writer.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn of_two_concurrent_edits_from_one_parent_exactly_one_is_accepted() {
    let (_db, app, key_a, key_b) = setup().await;

    let hashed = |id: &str, content: &str, hash: &str, parent: Option<&str>| {
        let mut value = note(id, content, LATER);
        value["contentHash"] = json!(hash);
        value["parentHash"] = json!(parent);
        value
    };

    // Many notes per push, in opposite orders, so that without one lock around
    // the read and the write the two pushes interleave note by note.
    for trial in 0..15 {
        let ids: Vec<String> = (0..40).map(|_| new_id()).collect();
        let parent = format!("parent-{trial}");
        let base: Vec<Value> = ids.iter().map(|id| hashed(id, "base", &parent, None)).collect();
        push(&app, &key_a, json!({ "notes": base })).await;

        let edits = |name: &str, hash: &str, reverse: bool| {
            let mut notes: Vec<Value> = ids.iter().map(|id| hashed(id, name, hash, Some(&parent))).collect();
            if reverse {
                notes.reverse();
            }
            json!({ "notes": notes })
        };
        let spawn_edit = |key: String, body: Value| {
            let app = app.clone();
            tokio::spawn(async move { push(&app, &key, body).await })
        };
        let first = spawn_edit(key_a.clone(), edits("edit a", "hash-a", false));
        let second = spawn_edit(key_b.clone(), edits("edit b", "hash-b", true));
        let (first, second) = (first.await.unwrap(), second.await.unwrap());

        let accepted: Vec<&str> = [&first, &second]
            .iter()
            .flat_map(|r| r["accepted"].as_array().unwrap())
            .map(|a| a["id"].as_str().unwrap())
            .collect();
        let unique: HashSet<&&str> = accepted.iter().collect();
        assert_eq!(accepted.len(), unique.len(), "trial {trial}: an edit was silently overwritten");
        assert_eq!(accepted.len(), ids.len(), "trial {trial}: every note needs one winner");
    }
}

#[tokio::test]
async fn a_bad_attachment_is_reported_without_failing_the_push() {
    let (_db, app, key, _) = setup().await;
    let (id, attachment) = (new_id(), new_id());

    let response = push(
        &app,
        &key,
        json!({
            "notes": [note_with_attachments(&id, "a", EARLIER, &[&attachment])],
            "attachments": [{ "id": attachment, "data": "not base64!" }],
        }),
    )
    .await;

    assert_eq!(response["accepted"][0]["id"], id.as_str());
    let errors = response["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].as_str().unwrap().contains(&attachment));
    assert_eq!(response["attachmentWarnings"][0]["attachmentIds"][0], attachment.as_str());
}

#[tokio::test]
async fn purging_old_deleted_notes_happens_in_housekeeping_not_in_a_request() {
    let (db, app, key, _) = setup().await;
    let id = new_id();

    let mut old = note(&id, "binned long ago", EARLIER);
    old["deleted"] = json!(true);
    old["deletedAt"] = json!("2020-01-01T00:00:00Z");
    push(&app, &key, json!({ "notes": [old] })).await;

    // Another push must not pay for the cleanup
    push(&app, &key, json!({ "notes": [note(&new_id(), "other", EARLIER)] })).await;
    let exists = |pool: sqlx::SqlitePool, id: String| async move {
        sqlx::query("SELECT COUNT(*) FROM notes WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
            .get::<i64, _>(0)
    };
    assert_eq!(exists(db.pool.clone(), id.clone()).await, 1);

    sqlx::query("UPDATE notes SET server_modified_at = '2020-01-01T00:00:00+00:00' WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();

    maintenance::run_once(&db.pool).await.unwrap();
    assert_eq!(exists(db.pool.clone(), id).await, 0);
}
