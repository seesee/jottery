// Migrations run against real data on a live server, so these start from a
// populated database, not an empty one.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::db::{run_migrations, MIGRATOR};
use serde_json::json;
use sqlx::{migrate::Migrator, Row, SqlitePool};
use std::borrow::Cow;

const ADMIN_ID: &str = "00000000-0000-0000-0000-000000000001";

/// Apply migrations up to and including `version`, as an old server would have.
async fn migrate_to(pool: &SqlitePool, version: i64) {
    let migrations: Vec<_> = MIGRATOR
        .iter()
        .filter(|m| m.version <= version)
        .cloned()
        .collect();
    let migrator = Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    };
    migrator.run(pool).await.expect("migrate to old version");
}

async fn exec(pool: &SqlitePool, sql: &str, binds: &[&str]) {
    let mut query = sqlx::query(sql);
    for bind in binds {
        query = query.bind(*bind);
    }
    query.execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query(&format!("SELECT COUNT(*) AS c FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
        .get("c")
}

struct Seeded {
    alice_key: String,
    alice_device_a: String,
    alice_device_b: String,
    note_one: String,
    note_two: String,
    attachment: String,
    second_attachment: String,
    blob_bytes: Vec<u8>,
}

/// A database at version 15 holding every kind of row, written the way the old
/// server wrote it.
async fn seed_v15(pool: &SqlitePool) -> Seeded {
    let alice = create_user(pool, "alice@example.com", &random_password()).await;
    let bob = create_user(pool, "bob@example.com", &random_password()).await;
    let (alice_device_a, alice_key) = create_device(pool, &alice).await;
    let (alice_device_b, _) = create_device(pool, &alice).await;
    let (bob_device, _) = create_device(pool, &bob).await;

    let (note_one, note_two, bob_note) = (new_id(), new_id(), new_id());
    for (id, user, client, stamp) in [
        (&note_one, &alice, &alice_device_a, "2026-03-01T10:00:00+00:00"),
        (&note_two, &alice, &alice_device_b, "2026-03-02T10:00:00.123456789+00:00"),
        (&bob_note, &bob, &bob_device, "2026-03-03T10:00:00Z"),
    ] {
        exec(
            pool,
            "INSERT INTO notes (id, user_id, client_id, created_at, modified_at, server_modified_at,
                                content, tags, pinned, archived, archived_at, locked, locked_at,
                                deleted, deleted_at, version, server_version, word_wrap,
                                syntax_language, show_preview, color, content_hash, parent_hash, hash_chain)
             VALUES (?1, ?2, ?3, '2026-01-01T00:00:00Z', '2026-02-01T00:00:00Z', ?4,
                     'ciphertext', '[\"tag\"]', 1, 1, '2026-02-02T00:00:00Z', 1, '2026-02-03T00:00:00Z',
                     0, NULL, 3, 7, 1, 'rust', 1, 'blue', 'hash-c', 'hash-p', '[\"hash-a\"]')",
            &[id, user, client, stamp],
        )
        .await;
    }

    let (attachment, second_attachment) = (new_id(), new_id());
    let blob_bytes: Vec<u8> = (0..=255).collect();
    for id in [&attachment, &second_attachment] {
        exec(
            pool,
            "INSERT INTO attachments_meta (id, note_id, note_user_id, filename, mime_type, size, created_at)
             VALUES (?1, ?2, ?3, 'enc-name', 'image/png', 256, '2026-03-01T00:00:00Z')",
            &[id, &note_one, &alice],
        )
        .await;
        sqlx::query("INSERT INTO attachments_data (id, data, created_at) VALUES (?, ?, '2026-03-01T00:00:00Z')")
            .bind(id)
            .bind(&blob_bytes)
            .execute(pool)
            .await
            .unwrap();
    }

    let version_key = format!("{note_one}:2");
    exec(
        pool,
        "INSERT INTO note_versions (version_key, note_id, user_id, client_id, version, created_at,
                                    synced_at, server_synced_at, content, tags, attachments,
                                    syntax_language, word_wrap, show_preview, color, reason,
                                    content_hash, parent_hash)
         VALUES (?1, ?2, ?3, ?4, 2, '2026-02-01T00:00:00Z', '2026-02-01T00:00:00Z',
                 '2026-02-15T00:00:00Z', 'old ciphertext', '[]', '[]', 'plain', 1, 0, NULL,
                 'sync', 'hash-p', NULL)",
        &[&version_key, &note_one, &alice, &alice_device_a],
    )
    .await;

    exec(
        pool,
        "INSERT INTO note_deletions (id, user_id, deleted_at, synced_from_client_id, expires_at)
         VALUES (?1, ?2, '2026-02-20T00:00:00Z', ?3, '2099-01-01T00:00:00Z')",
        &[&new_id(), &alice, &alice_device_a],
    )
    .await;

    for synced_at in [Some("2026-02-25T00:00:00Z"), None] {
        let id = new_id();
        sqlx::query(
            r#"INSERT INTO saved_searches (id, user_id, name, query, "order", created_at, modified_at, synced_at)
               VALUES (?, ?, 'search', '#tag', 0, '2026-01-01T00:00:00Z', '2026-02-24T00:00:00Z', ?)"#,
        )
        .bind(&id)
        .bind(&alice)
        .bind(synced_at)
        .execute(pool)
        .await
        .unwrap();
    }

    Seeded {
        alice_key,
        alice_device_a,
        alice_device_b,
        note_one,
        note_two,
        attachment,
        second_attachment,
        blob_bytes,
    }
}

/// A stable text form of every column that must survive, for before/after checks.
async fn note_snapshot(pool: &SqlitePool) -> Vec<String> {
    sqlx::query(
        "SELECT json_array(id, user_id, client_id, created_at, modified_at, server_modified_at,
                           content, tags, pinned, archived, archived_at, locked, locked_at, deleted,
                           deleted_at, version, server_version, word_wrap, syntax_language,
                           show_preview, color, content_hash, parent_hash, hash_chain) AS row
         FROM notes ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get("row"))
    .collect()
}

#[tokio::test]
async fn rebuild_keeps_every_row_of_a_populated_database() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    seed_v15(&db.pool).await;
    let notes_before = note_snapshot(&db.pool).await;

    run_migrations(&db.pool).await.expect("migrate");

    assert_eq!(count(&db.pool, "notes").await, 3);
    assert_eq!(count(&db.pool, "note_versions").await, 1);
    assert_eq!(count(&db.pool, "attachments_meta").await, 2);
    assert_eq!(count(&db.pool, "attachments_data").await, 2);
    assert_eq!(count(&db.pool, "note_attachments").await, 2);
    assert_eq!(count(&db.pool, "saved_searches").await, 2);
    assert_eq!(count(&db.pool, "note_deletions").await, 1);
    assert_eq!(note_snapshot(&db.pool).await, notes_before);

    let violations = sqlx::query("PRAGMA foreign_key_check").fetch_all(&db.pool).await.unwrap();
    assert!(violations.is_empty(), "migration left dangling foreign keys");
}

#[tokio::test]
async fn rebuild_assigns_each_user_one_sequence_in_server_time_order() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    seed_v15(&db.pool).await;

    run_migrations(&db.pool).await.expect("migrate");

    let alice: String = sqlx::query("SELECT id FROM users WHERE email = 'alice@example.com'")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);

    // Alice has two notes, a version, a tombstone and two saved searches
    let rows: Vec<i64> = sqlx::query(
        "SELECT change_seq FROM notes WHERE user_id = ?1
         UNION ALL SELECT change_seq FROM note_versions WHERE user_id = ?1
         UNION ALL SELECT change_seq FROM note_deletions WHERE user_id = ?1
         UNION ALL SELECT change_seq FROM saved_searches WHERE user_id = ?1
         ORDER BY 1",
    )
    .bind(&alice)
    .fetch_all(&db.pool)
    .await
    .unwrap()
    .iter()
    .map(|r| r.get(0))
    .collect();
    assert_eq!(rows, (1..=6).collect::<Vec<i64>>(), "every row gets its own sequence number");

    let last: i64 = sqlx::query("SELECT last_seq FROM sync_state WHERE user_id = ?")
        .bind(&alice)
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get("last_seq");
    assert_eq!(last, 6);

    // The version predates the notes, so it must sort before them.
    let version_seq: i64 = sqlx::query("SELECT change_seq FROM note_versions")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    let first_note_seq: i64 = sqlx::query("SELECT MIN(change_seq) FROM notes WHERE user_id = ?")
        .bind(&alice)
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert!(version_seq < first_note_seq);
}

#[tokio::test]
async fn rebuild_backfills_the_server_times() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    seed_v15(&db.pool).await;

    run_migrations(&db.pool).await.expect("migrate");

    let received: String = sqlx::query("SELECT received_at FROM note_deletions")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(received, "2026-02-20T00:00:00Z");

    let missing: i64 = sqlx::query("SELECT COUNT(*) FROM saved_searches WHERE synced_at IS NULL")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(missing, 0);
}

#[tokio::test]
async fn a_migrated_database_serves_its_data_and_accepts_new_devices_pushes() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    let seeded = seed_v15(&db.pool).await;
    run_migrations(&db.pool).await.expect("migrate");

    let app = app(&db.pool);
    let pulled = pull_all(&app, &seeded.alice_key).await;

    let mut expected = vec![seeded.note_one.clone(), seeded.note_two.clone()];
    expected.sort();
    assert_eq!(ids(&pulled["notes"]), expected);

    let first = pulled["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == seeded.note_one.as_str())
        .unwrap();
    assert_eq!(first["attachments"].as_array().unwrap().len(), 2);
    assert_eq!(first["pinned"], true);
    assert_eq!(first["archived"], true);
    assert_eq!(first["locked"], true);
    assert_eq!(first["color"], "blue");
    assert_eq!(first["contentHash"], "hash-c");

    // Blobs keep their bytes and are reachable by the client's id
    let mut blob_ids = vec![seeded.attachment.clone(), seeded.second_attachment.clone()];
    blob_ids.sort();
    assert_eq!(ids(&pulled["attachments"]), blob_ids);

    use base64::Engine;
    let (status, fetched) = call(
        &app,
        "GET",
        &format!("/api/v1/sync/attachments/{}", seeded.attachment),
        Some(&seeded.alice_key),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(fetched["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(decoded, seeded.blob_bytes);

    // New writes continue the sequence rather than restarting it
    push(
        &app,
        &seeded.alice_key,
        json!({ "notes": [note(&new_id(), "new", "2026-04-01T00:00:00Z")] }),
    )
    .await;
    assert_eq!(count(&db.pool, "notes").await, 4);
}

#[tokio::test]
async fn deleting_a_device_after_migration_keeps_its_notes() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    let seeded = seed_v15(&db.pool).await;
    run_migrations(&db.pool).await.expect("migrate");

    sqlx::query("DELETE FROM clients WHERE id = ?")
        .bind(&seeded.alice_device_a)
        .execute(&db.pool)
        .await
        .unwrap();

    assert_eq!(count(&db.pool, "notes").await, 3);
    assert_eq!(count(&db.pool, "note_versions").await, 1);
    assert_eq!(count(&db.pool, "attachments_data").await, 2);

    let client: Option<String> = sqlx::query("SELECT client_id FROM notes WHERE id = ?")
        .bind(&seeded.note_one)
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(client, None);

    let untouched: Option<String> = sqlx::query("SELECT client_id FROM notes WHERE id = ?")
        .bind(&seeded.note_two)
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(untouched.as_deref(), Some(seeded.alice_device_b.as_str()));
}

#[tokio::test]
async fn foreign_keys_are_back_on_for_every_pooled_connection() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    seed_v15(&db.pool).await;
    run_migrations(&db.pool).await.expect("migrate");

    // Hold every connection at once so the one the migrator used is among them.
    let mut connections = Vec::new();
    for _ in 0..db.pool.options().get_max_connections() {
        connections.push(db.pool.acquire().await.unwrap());
    }
    for connection in &mut connections {
        let enabled: i64 = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&mut **connection)
            .await
            .unwrap()
            .get(0);
        assert_eq!(enabled, 1);
    }
}

#[tokio::test]
async fn running_with_foreign_keys_on_rolls_back_instead_of_losing_data() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    let seeded = seed_v15(&db.pool).await;

    // The plain migrator leaves foreign keys on, so DROP TABLE cascades.
    let result = MIGRATOR.run(&db.pool).await;
    let error = result.expect_err("the row count guard must refuse this").to_string();
    assert!(error.contains("rows_preserved"), "unexpected error: {error}");

    assert_eq!(count(&db.pool, "notes").await, 3);
    assert_eq!(count(&db.pool, "note_versions").await, 1);
    assert_eq!(count(&db.pool, "attachments_data").await, 2);

    // Nothing was recorded, so the safe path can still finish the job.
    run_migrations(&db.pool).await.expect("migrate");
    assert_eq!(count(&db.pool, "attachments_data").await, 2);
    let pulled = pull_all(&app(&db.pool), &seeded.alice_key).await;
    assert_eq!(pulled["notes"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn upgrading_adds_the_audit_log_and_measures_existing_accounts() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    seed_v15(&db.pool).await;
    let users = count(&db.pool, "users").await;

    run_migrations(&db.pool).await.expect("migrate");

    assert_eq!(count(&db.pool, "users").await, users);
    assert_eq!(count(&db.pool, "admin_audit_log").await, 0);

    let used = |pool: SqlitePool| async move {
        sqlx::query_scalar::<_, i64>("SELECT storage_used_bytes FROM users WHERE email = 'alice@example.com'")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    assert_eq!(used(db.pool.clone()).await, 0);

    jottery_server::maintenance::run_once(&db.pool).await.expect("housekeeping");

    assert!(used(db.pool.clone()).await > 0);
}

#[tokio::test]
async fn migrating_twice_changes_nothing() {
    let db = empty_db().await;
    migrate_to(&db.pool, 15).await;
    seed_v15(&db.pool).await;

    run_migrations(&db.pool).await.expect("first run");
    let snapshot = note_snapshot(&db.pool).await;
    run_migrations(&db.pool).await.expect("second run");
    assert_eq!(note_snapshot(&db.pool).await, snapshot);
}

#[tokio::test]
async fn upgrading_from_before_the_multi_user_migrations_keeps_notes_and_attachments() {
    // Migrations 003 and 004 rebuild tables too, and used to empty them.
    let db = empty_db().await;
    migrate_to(&db.pool, 2).await;

    let (client, note_id, attachment) = (new_id(), new_id(), new_id());
    exec(
        &db.pool,
        "INSERT INTO clients (id, api_key, device_name, device_type, created_at, last_seen_at)
         VALUES (?1, 'hashed-key', 'old device', 'web', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        &[&client],
    )
    .await;
    exec(
        &db.pool,
        "INSERT INTO notes (id, client_id, created_at, modified_at, server_modified_at, content, tags)
         VALUES (?1, ?2, '2024-01-01T00:00:00Z', '2024-01-02T00:00:00Z', '2024-01-02T00:00:00Z', 'ciphertext', '[]')",
        &[&note_id, &client],
    )
    .await;
    exec(
        &db.pool,
        "INSERT INTO attachments_meta (id, note_id, filename, mime_type, size, created_at)
         VALUES (?1, ?2, 'enc-name', 'image/png', 4, '2024-01-01T00:00:00Z')",
        &[&attachment, &note_id],
    )
    .await;
    sqlx::query("INSERT INTO attachments_data (id, data, created_at) VALUES (?, ?, '2024-01-01T00:00:00Z')")
        .bind(&attachment)
        .bind(vec![1u8, 2, 3, 4])
        .execute(&db.pool)
        .await
        .unwrap();
    let version_key = format!("{note_id}:1");
    exec(
        &db.pool,
        "INSERT INTO note_versions (version_key, note_id, client_id, version, created_at, synced_at,
                                    server_synced_at, content, tags, attachments, reason)
         VALUES (?1, ?2, ?3, 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z',
                 'old', '[]', '[]', 'sync')",
        &[&version_key, &note_id, &client],
    )
    .await;

    run_migrations(&db.pool).await.expect("migrate");

    assert_eq!(count(&db.pool, "notes").await, 1);
    assert_eq!(count(&db.pool, "note_versions").await, 1);
    assert_eq!(count(&db.pool, "attachments_meta").await, 1);
    assert_eq!(count(&db.pool, "attachments_data").await, 1);
    assert_eq!(count(&db.pool, "note_attachments").await, 1);

    let owner: String = sqlx::query("SELECT user_id FROM notes")
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(owner, ADMIN_ID);
}
