// A stored version is what a 3-way merge uses as its base, so its hash has to
// describe its content. Two devices that diverge from one version both produce the
// same key, because the key is only noteId:version.

mod common;

use common::*;
use serde_json::json;
use sqlx::{Row, SqlitePool};

const TIME: &str = "2026-02-01T00:00:00Z";

async fn stored(pool: &SqlitePool, note_id: &str) -> (String, String) {
    let row = sqlx::query("SELECT content, content_hash FROM note_versions WHERE note_id = ?")
        .bind(note_id)
        .fetch_one(pool)
        .await
        .unwrap();
    (row.get(0), row.get(1))
}

async fn setup() -> (TestDb, axum::Router, String) {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    (db, app, key)
}

#[tokio::test]
async fn a_colliding_version_cannot_relabel_stored_content() {
    let (db, app, key) = setup().await;
    let id = new_id();

    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "base", TIME)], "versions": [version(&id, 6, "from-a")] }),
    )
    .await;
    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "base", TIME)], "versions": [version(&id, 6, "from-b")] }),
    )
    .await;

    assert_eq!(
        stored(&db.pool, &id).await,
        ("from-a".to_string(), "hash-from-a".to_string())
    );
}

#[tokio::test]
async fn the_same_version_pushed_again_is_announced_again() {
    let (db, app, key) = setup().await;
    let id = new_id();
    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "base", TIME)], "versions": [version(&id, 6, "same")] }),
    )
    .await;
    let first = pull(&app, &key, json!({ "cursor": "" })).await;

    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "base", TIME)], "versions": [version(&id, 6, "same")] }),
    )
    .await;

    let next = pull(&app, &key, json!({ "cursor": first["nextCursor"] })).await;
    assert_eq!(next["versions"].as_array().unwrap().len(), 1);
    assert_eq!(stored(&db.pool, &id).await.1, "hash-same");
}

#[tokio::test]
async fn a_hash_is_never_added_to_content_it_was_not_stored_with() {
    let (db, app, key) = setup().await;
    let id = new_id();
    let mut legacy = version(&id, 6, "legacy");
    legacy["contentHash"] = json!(null);
    push(&app, &key, json!({ "notes": [note(&id, "base", TIME)], "versions": [legacy] })).await;

    push(
        &app,
        &key,
        json!({ "notes": [note(&id, "base", TIME)], "versions": [version(&id, 6, "newer")] }),
    )
    .await;

    let row = sqlx::query("SELECT content, content_hash FROM note_versions WHERE note_id = ?")
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>(0), "legacy");
    assert_eq!(row.get::<Option<String>, _>(1), None);
}
