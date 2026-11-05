// Devices and users must not be able to destroy or reach each other's data.

mod common;

use axum::http::StatusCode;
use base64::Engine;
use common::*;
use jottery_server::services::AdminService;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

const LATER: &str = "2026-06-01T00:00:00Z";
const EARLIER: &str = "2026-02-01T00:00:00Z";

struct Account {
    id: String,
    key: String,
    device: String,
    session: String,
}

async fn account(app: &axum::Router, pool: &SqlitePool, name: &str) -> Account {
    let password = random_password();
    let email = format!("{name}-{}@example.com", new_id());
    let id = create_user(pool, &email, &password).await;
    let (device, key) = create_device(pool, &id).await;
    let (status, body) = call(
        app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    Account {
        id,
        key,
        device,
        session: body["sessionId"].as_str().unwrap().to_string(),
    }
}

async fn attachments_of(app: &axum::Router, key: &str, note_id: &str) -> Vec<String> {
    let pulled = pull_all(app, key).await;
    let note = pulled["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == note_id)
        .unwrap_or_else(|| panic!("note {note_id} not in pull"));
    ids(&note["attachments"])
}

fn decoded_blob(pulled: &Value, id: &str) -> Vec<u8> {
    let entry = pulled["attachments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id)
        .unwrap_or_else(|| panic!("blob {id} not in pull"));
    base64::engine::general_purpose::STANDARD
        .decode(entry["data"].as_str().unwrap())
        .unwrap()
}

/// Device A first-pushes two notes, one with an attachment and a version, and
/// device B edits one of them.
async fn notes_first_pushed_by_a(app: &axum::Router, a_key: &str, b_key: &str) -> (String, String, String) {
    let (n1, n2, attachment) = (new_id(), new_id(), new_id());
    push(
        app,
        a_key,
        json!({
            "notes": [
                note_with_attachments(&n1, "from a", EARLIER, &[&attachment]),
                note(&n2, "also from a", EARLIER),
            ],
            "attachments": [blob(&attachment, b"blob")],
            "versions": [version(&n1, 1, "from a")],
        }),
    )
    .await;
    push(app, b_key, json!({ "notes": [note(&n1, "edited on b", LATER)] })).await;
    (n1, n2, attachment)
}

async fn assert_vault_intact(app: &axum::Router, key: &str, n1: &str, n2: &str, attachment: &str) {
    let pulled = pull_all(app, key).await;
    let mut expected = vec![n1.to_string(), n2.to_string()];
    expected.sort();
    assert_eq!(ids(&pulled["notes"]), expected, "notes must survive the device");
    assert_eq!(ids(&pulled["attachments"]), vec![attachment.to_string()]);
    assert_eq!(pulled["versions"].as_array().unwrap().len(), 1);
    assert_eq!(pulled["deletions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn deleting_a_revoked_device_through_the_portal_keeps_its_notes() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&app, &db.pool, "alice").await;
    let (_, other_key) = create_device(&db.pool, &user.id).await;
    let (n1, n2, attachment) = notes_first_pushed_by_a(&app, &user.key, &other_key).await;

    // Revoke, then delete: exactly what the devices page does
    let uri = format!("/api/v1/user/devices/{}", user.device);
    for _ in 0..2 {
        let (status, _) = call(&app, "DELETE", &uri, Some(&user.session), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let remaining: i64 = sqlx::query("SELECT COUNT(*) FROM clients WHERE id = ?")
        .bind(&user.device)
        .fetch_one(&db.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(remaining, 0, "the device row itself should be gone");

    assert_vault_intact(&app, &other_key, &n1, &n2, &attachment).await;
}

#[tokio::test]
async fn deleting_a_device_through_the_admin_api_keeps_its_notes() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&app, &db.pool, "alice").await;
    let (_, other_key) = create_device(&db.pool, &user.id).await;
    let (n1, n2, attachment) = notes_first_pushed_by_a(&app, &user.key, &other_key).await;

    let admin = account(&app, &db.pool, "admin").await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&admin.id)
        .execute(&db.pool)
        .await
        .unwrap();

    let uri = format!("/api/v1/admin/devices/{}", user.device);
    for _ in 0..2 {
        let (status, _) = call(&app, "DELETE", &uri, Some(&admin.session), None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    assert_vault_intact(&app, &other_key, &n1, &n2, &attachment).await;
}

#[tokio::test]
async fn deleting_a_device_through_the_admin_cli_keeps_its_notes() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&app, &db.pool, "alice").await;
    let (_, other_key) = create_device(&db.pool, &user.id).await;
    let (n1, n2, attachment) = notes_first_pushed_by_a(&app, &user.key, &other_key).await;

    AdminService::delete_device(&db.pool, &user.device).await.unwrap();

    assert_vault_intact(&app, &other_key, &n1, &n2, &attachment).await;
}

#[tokio::test]
async fn a_shared_note_id_does_not_expose_the_other_users_attachments() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, attacker) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "attacker").await,
    );
    let (note_id, attachment) = (new_id(), new_id());

    push(
        &app,
        &victim.key,
        json!({
            "notes": [note_with_attachments(&note_id, "secret", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"secret bytes")],
        }),
    )
    .await;
    push(&app, &attacker.key, json!({ "notes": [note(&note_id, "dummy", LATER)] })).await;

    let pulled = pull_all(&app, &attacker.key).await;
    assert_eq!(pulled["notes"][0]["attachments"], json!([]));
    assert_eq!(pulled["attachments"], json!([]));

    let (status, _) = call(
        &app,
        "GET",
        &format!("/api/v1/sync/attachments/{attachment}"),
        Some(&attacker.key),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_conflict_response_only_lists_the_callers_attachments() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, attacker) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "attacker").await,
    );
    let (note_id, attachment) = (new_id(), new_id());

    push(
        &app,
        &victim.key,
        json!({ "notes": [note_with_attachments(&note_id, "secret", EARLIER, &[&attachment])] }),
    )
    .await;
    push(&app, &attacker.key, json!({ "notes": [note(&note_id, "mine", LATER)] })).await;

    // An older edit conflicts with the attacker's own note
    let response = push(
        &app,
        &attacker.key,
        json!({ "notes": [note(&note_id, "stale", EARLIER)] }),
    )
    .await;
    assert_eq!(response["rejected"][0]["id"], note_id.as_str());
    assert_eq!(response["rejected"][0]["serverAttachments"], json!([]));
}

#[tokio::test]
async fn deleting_all_notes_leaves_the_other_users_shared_id_alone() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, other) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "other").await,
    );
    let (note_id, attachment) = (new_id(), new_id());

    push(
        &app,
        &victim.key,
        json!({
            "notes": [note_with_attachments(&note_id, "keep me", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"keep these bytes")],
            "versions": [version(&note_id, 1, "keep me")],
        }),
    )
    .await;
    push(
        &app,
        &other.key,
        json!({
            "notes": [note(&note_id, "same id", LATER)],
            "versions": [version(&note_id, 1, "same id")],
        }),
    )
    .await;

    let (status, _) = call(&app, "DELETE", "/api/v1/user/notes", Some(&other.session), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let pulled = pull_all(&app, &victim.key).await;
    assert_eq!(ids(&pulled["notes"]), vec![note_id.clone()]);
    assert_eq!(decoded_blob(&pulled, &attachment), b"keep these bytes");
    assert_eq!(pulled["versions"].as_array().unwrap().len(), 1);
    assert_eq!(pull_all(&app, &other.key).await["notes"], json!([]));
}

#[tokio::test]
async fn deleting_an_account_leaves_the_other_users_shared_id_alone() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, other) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "other").await,
    );
    let (note_id, attachment) = (new_id(), new_id());

    push(
        &app,
        &victim.key,
        json!({
            "notes": [note_with_attachments(&note_id, "keep me", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"keep these bytes")],
        }),
    )
    .await;
    push(&app, &other.key, json!({ "notes": [note(&note_id, "same id", LATER)] })).await;

    let (status, _) = call(
        &app,
        "DELETE",
        "/api/v1/user/account?mode=delete",
        Some(&other.session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let pulled = pull_all(&app, &victim.key).await;
    assert_eq!(decoded_blob(&pulled, &attachment), b"keep these bytes");
}

#[tokio::test]
async fn account_statistics_only_count_the_callers_attachments() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (rich, other) = (
        account(&app, &db.pool, "rich").await,
        account(&app, &db.pool, "other").await,
    );
    let (note_id, attachment) = (new_id(), new_id());

    push(
        &app,
        &rich.key,
        json!({
            "notes": [note_with_attachments(&note_id, "a", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"data")],
        }),
    )
    .await;
    push(&app, &other.key, json!({ "notes": [note(&note_id, "b", LATER)] })).await;

    // One byte of content and "[]" for tags each. Only one of them has the blob.
    let (_, info) = call(&app, "GET", "/api/v1/user/account", Some(&other.session), None).await;
    assert_eq!(info["attachmentCount"], 0);
    assert_eq!(info["storageUsedBytes"], 3);

    let (_, info) = call(&app, "GET", "/api/v1/user/account", Some(&rich.session), None).await;
    assert_eq!(info["attachmentCount"], 1);
    assert_eq!(info["storageUsedBytes"], 3 + 4);
}

#[tokio::test]
async fn one_users_saved_search_cannot_overwrite_anothers() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, attacker) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "attacker").await,
    );
    let search = new_id();

    push(&app, &victim.key, json!({ "savedSearches": [saved_search(&search, "mine", EARLIER)] })).await;
    push(&app, &attacker.key, json!({ "savedSearches": [saved_search(&search, "junk", LATER)] })).await;

    assert_eq!(pull_all(&app, &victim.key).await["savedSearches"][0]["name"], "mine");
    // And the second account's row is stored, not silently dropped
    assert_eq!(pull_all(&app, &attacker.key).await["savedSearches"][0]["name"], "junk");
}

#[tokio::test]
async fn one_users_attachment_reference_cannot_overwrite_anothers() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, attacker) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "attacker").await,
    );
    let (victim_note, attacker_note, attachment) = (new_id(), new_id(), new_id());

    push(
        &app,
        &victim.key,
        json!({
            "notes": [note_with_attachments(&victim_note, "a", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"real bytes")],
        }),
    )
    .await;

    let mut forged = attachment_ref(&attachment);
    forged["filename"] = json!("junk");
    forged["size"] = json!(999);
    let mut attacker_copy = note(&attacker_note, "b", LATER);
    attacker_copy["attachments"] = json!([forged]);
    push(
        &app,
        &attacker.key,
        json!({ "notes": [attacker_copy], "attachments": [blob(&attachment, b"fake bytes")] }),
    )
    .await;

    let victims = pull_all(&app, &victim.key).await;
    let reference = &victims["notes"][0]["attachments"][0];
    assert_eq!(reference["filename"], "encrypted-name");
    assert_eq!(reference["size"], 4);
    assert_eq!(decoded_blob(&victims, &attachment), b"real bytes");

    // The same id in a second account is that account's own attachment
    let attackers = pull_all(&app, &attacker.key).await;
    assert_eq!(attackers["notes"][0]["attachments"][0]["filename"], "junk");
    assert_eq!(decoded_blob(&attackers, &attachment), b"fake bytes");
}

#[tokio::test]
async fn one_users_version_cannot_overwrite_anothers() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (victim, attacker) = (
        account(&app, &db.pool, "victim").await,
        account(&app, &db.pool, "attacker").await,
    );
    let note_id = new_id();

    push(
        &app,
        &victim.key,
        json!({ "notes": [note(&note_id, "a", EARLIER)], "versions": [version(&note_id, 1, "mine")] }),
    )
    .await;
    push(
        &app,
        &attacker.key,
        json!({ "notes": [note(&note_id, "b", LATER)], "versions": [version(&note_id, 1, "junk")] }),
    )
    .await;

    let versions = &pull_all(&app, &victim.key).await["versions"];
    assert_eq!(versions[0]["contentHash"], "hash-mine");
    assert_eq!(versions[0]["content"], "mine");

    let versions = &pull_all(&app, &attacker.key).await["versions"];
    assert_eq!(versions[0]["content"], "junk", "the second account's version must be stored");
}

#[tokio::test]
async fn an_attachment_shared_by_two_notes_appears_on_both() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&app, &db.pool, "alice").await;
    let (original, combined, attachment) = (new_id(), new_id(), new_id());

    push(
        &app,
        &user.key,
        json!({
            "notes": [note_with_attachments(&original, "a", EARLIER, &[&attachment])],
            "attachments": [blob(&attachment, b"shared")],
        }),
    )
    .await;
    push(
        &app,
        &user.key,
        json!({ "notes": [note_with_attachments(&combined, "b", LATER, &[&attachment])] }),
    )
    .await;

    assert_eq!(attachments_of(&app, &user.key, &original).await, vec![attachment.clone()]);
    assert_eq!(attachments_of(&app, &user.key, &combined).await, vec![attachment.clone()]);
}

#[tokio::test]
async fn hard_deleting_one_note_keeps_a_blob_another_note_needs() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = account(&app, &db.pool, "alice").await;
    let (original, combined, attachment) = (new_id(), new_id(), new_id());

    push(
        &app,
        &user.key,
        json!({
            "notes": [
                note_with_attachments(&original, "a", EARLIER, &[&attachment]),
                note_with_attachments(&combined, "b", EARLIER, &[&attachment]),
            ],
            "attachments": [blob(&attachment, b"shared")],
        }),
    )
    .await;

    push(
        &app,
        &user.key,
        json!({ "deletions": [{ "id": original, "deletedAt": LATER }] }),
    )
    .await;

    let pulled = pull_all(&app, &user.key).await;
    assert_eq!(ids(&pulled["notes"]), vec![combined.clone()]);
    assert_eq!(decoded_blob(&pulled, &attachment), b"shared");
}

#[tokio::test]
async fn deleting_a_user_through_the_admin_api_removes_their_blobs() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (user, other) = (
        account(&app, &db.pool, "leaving").await,
        account(&app, &db.pool, "staying").await,
    );
    let (note_id, attachment, other_note) = (new_id(), new_id(), new_id());

    for (key, id) in [(&user.key, &note_id), (&other.key, &other_note)] {
        push(
            &app,
            key,
            json!({
                "notes": [note_with_attachments(id, "a", EARLIER, &[&attachment])],
                "attachments": [blob(&attachment, b"data")],
            }),
        )
        .await;
    }

    let admin = account(&app, &db.pool, "admin").await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&admin.id)
        .execute(&db.pool)
        .await
        .unwrap();

    let (status, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/admin/users/{}", user.id),
        Some(&admin.session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    for table in ["attachments_meta", "attachments_data", "note_attachments"] {
        let rows: i64 = sqlx::query(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(rows, 1, "{table} must keep only the other user's row");
    }
    assert_eq!(decoded_blob(&pull_all(&app, &other.key).await, &attachment), b"data");
}
