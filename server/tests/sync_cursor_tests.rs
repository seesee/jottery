// The pull cursor, and the timestamp filters older clients still rely on.

mod common;

use axum::http::StatusCode;
use chrono::{Duration, Utc};
use common::*;
use serde_json::{json, Value};
use sqlx::SqlitePool;
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

fn cursor_of(response: &Value) -> String {
    response["nextCursor"].as_str().expect("expected nextCursor").to_string()
}

/// Follow a cursor until the server has nothing more, gathering what arrived.
async fn pull_to_end(app: &axum::Router, key: &str, start: &str, limit: i64) -> (Vec<Value>, String) {
    let mut cursor = start.to_string();
    let mut pages = Vec::new();
    loop {
        let page = pull(app, key, json!({ "cursor": cursor, "limit": limit })).await;
        cursor = cursor_of(&page);
        let more = page["hasMore"].as_bool().unwrap();
        pages.push(page);
        if !more {
            return (pages, cursor);
        }
    }
}

fn note_ids_in(pages: &[Value]) -> Vec<String> {
    pages.iter().flat_map(|page| ids(&page["notes"])).collect()
}

/// Rewrite a row's server time, to stage an old row without waiting.
async fn age_note(pool: &SqlitePool, id: &str, minutes: i64) {
    let stamp = (Utc::now() - Duration::minutes(minutes)).to_rfc3339();
    sqlx::query("UPDATE notes SET server_modified_at = ? WHERE id = ?")
        .bind(stamp)
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn pages_deliver_every_change_exactly_once() {
    let (_db, app, key, _) = setup().await;
    let mut expected: Vec<String> = (0..7).map(|_| new_id()).collect();
    let notes: Vec<Value> = expected.iter().map(|id| note(id, "a", EARLIER)).collect();
    push(&app, &key, json!({ "notes": notes })).await;

    let (pages, _) = pull_to_end(&app, &key, "", 3).await;

    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0]["notes"].as_array().unwrap().len(), 3);
    assert_eq!(pages[0]["hasMore"], true);
    assert_eq!(pages[2]["notes"].as_array().unwrap().len(), 1);
    assert_eq!(pages[2]["hasMore"], false);

    let mut delivered = note_ids_in(&pages);
    delivered.sort();
    expected.sort();
    assert_eq!(delivered, expected);
}

#[tokio::test]
async fn an_edit_during_a_multi_page_pull_skips_nothing() {
    // With offsets, this edit shifted n2 out of reach for good.
    let (_db, app, key_a, key_b) = setup().await;
    let (n1, n2, n3) = (new_id(), new_id(), new_id());
    for id in [&n1, &n2, &n3] {
        push(&app, &key_a, json!({ "notes": [note(id, "a", EARLIER)] })).await;
    }

    let first = pull(&app, &key_b, json!({ "cursor": "", "limit": 1 })).await;
    assert_eq!(ids(&first["notes"]), vec![n1.clone()]);

    push(&app, &key_a, json!({ "notes": [note(&n1, "edited", LATER)] })).await;

    let (rest, _) = pull_to_end(&app, &key_b, &cursor_of(&first), 1).await;
    let rest = note_ids_in(&rest);
    assert_eq!(rest, vec![n2, n3, n1], "n1 comes round again, and nothing is lost");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn notes_pushed_during_a_pull_all_arrive() {
    let (_db, app, pusher, puller) = setup().await;
    let all: Vec<String> = (0..400).map(|_| new_id()).collect();

    let writer_app = app.clone();
    let batches = all.clone();
    let writer = tokio::spawn(async move {
        for batch in batches.chunks(50) {
            let notes: Vec<Value> = batch.iter().map(|id| note(id, "a", EARLIER)).collect();
            push(&writer_app, &pusher, json!({ "notes": notes })).await;
        }
    });

    let mut received = HashSet::new();
    let mut cursor = String::new();
    loop {
        let finished = writer.is_finished();
        let page = pull(&app, &puller, json!({ "cursor": cursor, "limit": 37 })).await;
        cursor = cursor_of(&page);
        received.extend(ids(&page["notes"]));
        // Stop only after a pass that started once the writer had finished
        if finished && page["hasMore"] == false {
            break;
        }
    }
    writer.await.unwrap();

    assert_eq!(received.len(), all.len(), "a note pushed during a pull was skipped");
}

#[tokio::test]
async fn deletions_versions_and_saved_searches_share_the_cursor() {
    let (_db, app, key_a, key_b) = setup().await;
    let (kept, gone, search) = (new_id(), new_id(), new_id());
    push(
        &app,
        &key_a,
        json!({
            "notes": [note(&kept, "a", EARLIER), note(&gone, "b", EARLIER)],
            "versions": [version(&kept, 1, "a")],
            "savedSearches": [saved_search(&search, "s", EARLIER)],
        }),
    )
    .await;
    let first = pull(&app, &key_b, json!({ "cursor": "" })).await;
    assert_eq!(first["notes"].as_array().unwrap().len(), 2);
    assert_eq!(first["versions"].as_array().unwrap().len(), 1);
    assert_eq!(first["savedSearches"].as_array().unwrap().len(), 1);

    // Nothing is sent twice
    let cursor = cursor_of(&first);
    let idle = pull(&app, &key_b, json!({ "cursor": cursor })).await;
    for kind in ["notes", "deletions", "versions", "savedSearches"] {
        assert_eq!(idle[kind], json!([]), "{kind} was sent again");
    }
    assert_eq!(cursor_of(&idle), cursor, "an idle pull must not move the cursor");

    // Changes dated before the cursor still arrive, because order is the server's
    push(
        &app,
        &key_a,
        json!({
            "deletions": [{ "id": gone, "deletedAt": EARLIER }],
            "savedSearches": [saved_search(&search, "renamed", EARLIER)],
        }),
    )
    .await;
    let next = pull(&app, &key_b, json!({ "cursor": cursor })).await;
    assert_eq!(ids(&next["deletions"]), vec![gone]);
    assert_eq!(next["savedSearches"][0]["name"], "renamed");
}

#[tokio::test]
async fn a_page_counts_every_kind_of_change_towards_the_limit() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    let versions: Vec<Value> = (1..=5).map(|v| version(&id, v, &format!("v{v}"))).collect();
    push(&app, &key, json!({ "notes": [note(&id, "a", EARLIER)], "versions": versions })).await;

    let (pages, _) = pull_to_end(&app, &key, "", 2).await;

    assert_eq!(pages.len(), 3);
    let delivered: usize = pages
        .iter()
        .map(|p| p["versions"].as_array().unwrap().len() + p["notes"].as_array().unwrap().len())
        .sum();
    assert_eq!(delivered, 6);
}

#[tokio::test]
async fn a_blob_that_arrives_after_its_note_still_reaches_other_devices() {
    let (_db, app, key_a, key_b) = setup().await;
    let (id, attachment) = (new_id(), new_id());

    // The reference arrives without the blob, and device B takes it
    let response = push(
        &app,
        &key_a,
        json!({ "notes": [note_with_attachments(&id, "a", EARLIER, &[&attachment])] }),
    )
    .await;
    assert_eq!(response["attachmentWarnings"][0]["attachmentIds"][0], attachment.as_str());
    let first = pull(&app, &key_b, json!({ "cursor": "" })).await;
    assert_eq!(first["attachments"], json!([]));

    // A repair push supplies it
    push(&app, &key_a, json!({ "attachments": [blob(&attachment, b"data")] })).await;

    let next = pull(&app, &key_b, json!({ "cursor": cursor_of(&first) })).await;
    assert_eq!(ids(&next["notes"]), vec![id]);
    assert_eq!(ids(&next["attachments"]), vec![attachment]);
}

#[tokio::test]
async fn a_client_can_detect_cursor_support() {
    let (_db, app, key, _) = setup().await;
    push(&app, &key, json!({ "notes": [note(&new_id(), "a", EARLIER)] })).await;

    let legacy = pull(&app, &key, json!({ "lastSyncAt": null })).await;
    assert!(legacy.get("nextCursor").is_none(), "old clients see an unchanged response");

    let modern = pull(&app, &key, json!({ "cursor": "" })).await;
    assert!(modern["nextCursor"].is_string());
    assert!(modern["syncedAt"].is_string(), "clients keep lastSyncAt for older servers");
}

#[tokio::test]
async fn an_empty_cursor_with_a_timestamp_continues_from_it() {
    let (db, app, key, _) = setup().await;
    let (old, recent) = (new_id(), new_id());
    push(
        &app,
        &key,
        json!({ "notes": [note(&old, "old", EARLIER), note(&recent, "recent", EARLIER)] }),
    )
    .await;
    age_note(&db.pool, &old, 120).await;

    // Moving from lastSyncAt to a cursor must not re-download the vault
    let last_sync = (Utc::now() - Duration::minutes(60)).to_rfc3339();
    let page = pull(&app, &key, json!({ "cursor": "", "lastSyncAt": last_sync })).await;
    assert_eq!(ids(&page["notes"]), vec![recent]);

    let idle = pull(&app, &key, json!({ "cursor": cursor_of(&page) })).await;
    assert_eq!(idle["notes"], json!([]));
}

#[tokio::test]
async fn an_empty_cursor_with_no_new_changes_starts_at_the_end() {
    let (db, app, key, _) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "old", EARLIER)] })).await;
    age_note(&db.pool, &id, 120).await;

    let last_sync = (Utc::now() - Duration::minutes(60)).to_rfc3339();
    let page = pull(&app, &key, json!({ "cursor": "", "lastSyncAt": last_sync })).await;
    assert_eq!(page["notes"], json!([]));

    let fresh = new_id();
    push(&app, &key, json!({ "notes": [note(&fresh, "new", EARLIER)] })).await;
    let next = pull(&app, &key, json!({ "cursor": cursor_of(&page) })).await;
    assert_eq!(ids(&next["notes"]), vec![fresh]);
}

#[tokio::test]
async fn an_unusable_cursor_restarts_rather_than_missing_changes() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    push(&app, &key, json!({ "notes": [note(&id, "a", EARLIER)] })).await;

    // Garbage, and a position ahead of the sequence, as after restoring a backup
    for cursor in ["not a cursor", "v1.-4", "v1.999999", "v9.1"] {
        let page = pull(&app, &key, json!({ "cursor": cursor })).await;
        assert_eq!(ids(&page["notes"]), vec![id.clone()], "cursor {cursor:?}");
    }
}

#[tokio::test]
async fn cursors_are_private_to_a_user() {
    let (db, app, key, _) = setup().await;
    let other = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, other_key) = create_device(&db.pool, &other).await;

    push(&app, &key, json!({ "notes": [note(&new_id(), "mine", EARLIER)] })).await;
    let mine = cursor_of(&pull(&app, &key, json!({ "cursor": "" })).await);

    // Another account's activity neither reaches this user nor moves their cursor
    let notes: Vec<Value> = (0..5).map(|_| note(&new_id(), "theirs", EARLIER)).collect();
    push(&app, &other_key, json!({ "notes": notes })).await;

    let page = pull(&app, &key, json!({ "cursor": mine })).await;
    assert_eq!(page["notes"], json!([]));
    assert_eq!(cursor_of(&page), mine);
}

#[tokio::test]
async fn a_cursor_page_is_never_larger_than_the_cap() {
    let (_db, app, key, _) = setup().await;
    let notes: Vec<Value> = (0..3).map(|_| note(&new_id(), "a", EARLIER)).collect();
    push(&app, &key, json!({ "notes": notes })).await;

    for limit in [0, -1] {
        let page = pull(&app, &key, json!({ "cursor": "", "limit": limit })).await;
        assert_eq!(page["notes"].as_array().unwrap().len(), 1, "limit {limit} clamps to 1");
    }
}

// --- lastSyncAt, for clients that haven't moved to the cursor ---

#[tokio::test]
async fn a_tombstone_pushed_late_reaches_a_device_that_synced_meanwhile() {
    let (_db, app, key_a, key_b) = setup().await;
    let id = new_id();
    push(&app, &key_a, json!({ "notes": [note(&id, "a", EARLIER)] })).await;

    // B finishes a sync, then A pushes a deletion it made well before that
    let last_sync = pull(&app, &key_b, json!({})).await["syncedAt"].as_str().unwrap().to_string();
    let deleted_at = (Utc::now() - Duration::hours(1)).to_rfc3339();
    push(&app, &key_a, json!({ "deletions": [{ "id": id, "deletedAt": deleted_at }] })).await;

    let next = pull(&app, &key_b, json!({ "lastSyncAt": last_sync })).await;
    assert_eq!(ids(&next["deletions"]), vec![id], "a client timestamp must not hide the tombstone");
}

#[tokio::test]
async fn a_saved_search_edited_offline_reaches_a_device_that_synced_meanwhile() {
    let (_db, app, key_a, key_b) = setup().await;
    let search = new_id();

    let last_sync = pull(&app, &key_b, json!({})).await["syncedAt"].as_str().unwrap().to_string();
    let edited_at = (Utc::now() - Duration::hours(1)).to_rfc3339();
    push(&app, &key_a, json!({ "savedSearches": [saved_search(&search, "offline", &edited_at)] })).await;

    let next = pull(&app, &key_b, json!({ "lastSyncAt": last_sync })).await;
    assert_eq!(next["savedSearches"][0]["name"], "offline");
}

#[tokio::test]
async fn last_sync_reaches_back_over_the_commit_window() {
    let (db, app, key, _) = setup().await;
    let (just_before, long_before) = (new_id(), new_id());
    push(
        &app,
        &key,
        json!({ "notes": [note(&just_before, "a", EARLIER), note(&long_before, "b", EARLIER)] }),
    )
    .await;
    // Stamped before the client's lastSyncAt but committed after it
    age_note(&db.pool, &just_before, 1).await;
    age_note(&db.pool, &long_before, 30).await;

    let page = pull(&app, &key, json!({ "lastSyncAt": Utc::now().to_rfc3339() })).await;
    assert_eq!(ids(&page["notes"]), vec![just_before]);
}

#[tokio::test]
async fn synced_at_is_taken_before_the_rows_are_read() {
    let (_db, app, key, _) = setup().await;
    let id = new_id();
    let before = Utc::now();
    push(&app, &key, json!({ "notes": [note(&id, "a", EARLIER)] })).await;

    let page = pull(&app, &key, json!({})).await;
    let synced_at = chrono::DateTime::parse_from_rfc3339(page["syncedAt"].as_str().unwrap()).unwrap();
    assert!(synced_at >= before);
    assert_eq!(page["notes"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn offset_paging_still_works_for_old_clients() {
    let (_db, app, key, _) = setup().await;
    let notes: Vec<Value> = (0..5).map(|_| note(&new_id(), "a", EARLIER)).collect();
    push(&app, &key, json!({ "notes": notes })).await;

    let first = pull(&app, &key, json!({ "limit": 2, "offset": 0 })).await;
    assert_eq!(first["notes"].as_array().unwrap().len(), 2);
    assert_eq!(first["totalCount"], 5);
    assert_eq!(first["hasMore"], true);

    let last = pull(&app, &key, json!({ "limit": 2, "offset": 4 })).await;
    assert_eq!(last["notes"].as_array().unwrap().len(), 1);
    assert_eq!(last["totalCount"], 5);
    assert_eq!(last["hasMore"], false);
}

#[tokio::test]
async fn pulling_requires_only_the_fields_a_cursor_client_needs() {
    let (_db, app, key, _) = setup().await;
    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/sync/pull",
        Some(&key),
        Some(json!({ "cursor": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
