// Sync API endpoint integration tests
//
// Tests for sync endpoints:
// - GET /api/v1/sync/status
// - POST /api/v1/sync/push
// - POST /api/v1/sync/pull
// - DELETE /api/v1/sync/notes/:id

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::Digest;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::{Service, util::ServiceExt};

// Helper to create test database with migrations
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

// Helper to create test app with sync routes
async fn create_test_app() -> (axum::Router, SqlitePool) {
    let pool = setup_test_db().await;

    let config = jottery_server::config::Config {
        database_url: "sqlite::memory:".to_string(),
        port: 3030,
        max_payload_size: 5_242_880,
        cors_allowed_origins: None,
        session_expiry_days: 7,
        default_admin_email: "admin@localhost".to_string(),
        default_admin_password: "changeme".to_string(),
        argon2_m_cost: 19456,
        argon2_t_cost: 2,
        argon2_p_cost: 1,
        default_storage_quota_mb: 1000,
        default_max_upload_size_mb: 5,
    };

    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        config,
    });

    // Build router with sync routes
    let app = axum::Router::new()
        .route("/api/v1/sync/status", axum::routing::get(jottery_server::api::sync::get_status))
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .route("/api/v1/sync/pull", axum::routing::post(jottery_server::api::sync::pull))
        .route("/api/v1/sync/notes/:id", axum::routing::delete(jottery_server::api::sync::delete_note))
        .route("/api/v1/auth/register-user", axum::routing::post(jottery_server::api::auth::register_user))
        .route("/api/v1/auth/register-device", axum::routing::post(jottery_server::api::auth::register_device))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            jottery_server::api::middleware::auth_middleware,
        ))
        .with_state(app_state);

    (app, pool)
}

// Helper to parse JSON response
async fn parse_json_response(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// Helper to create a test user and device, returns API key
async fn create_test_user_and_device(pool: &SqlitePool) -> String {
    use jottery_server::utils::password::hash_password_with_params;

    let user_id = uuid::Uuid::new_v4().to_string();
    let email = format!("testuser{}@example.com", uuid::Uuid::new_v4());
    let password_hash = hash_password_with_params("testpassword123", 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    // Create approved user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_active, created_at)
           VALUES (?, ?, ?, 1, 1, ?)"#,
        user_id,
        email,
        password_hash,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create test user");

    // Create device with API key
    let device_id = uuid::Uuid::new_v4().to_string();
    let api_key = format!("{:x}", uuid::Uuid::new_v4());
    let api_key_hash = format!("{:x}", sha2::Sha256::digest(api_key.as_bytes()));

    sqlx::query!(
        r#"INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        device_id,
        user_id,
        api_key_hash,
        "Test Device",
        "cli",
        now,
        now
    )
    .execute(pool)
    .await
    .expect("Failed to create test device");

    api_key
}

#[tokio::test]
async fn test_sync_status_requires_authentication() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/sync/status")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Should fail without API key
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_sync_status_with_valid_api_key() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let request = Request::builder()
        .uri("/api/v1/sync/status")
        .method("GET")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert!(body["clientId"].is_string());
    assert!(body["serverLastModified"].is_string());
    assert_eq!(body["noteCount"], 0); // No notes yet
    assert!(body["lastSyncedAt"].is_null()); // Never synced

    pool.close().await;
}

#[tokio::test]
async fn test_sync_push_requires_authentication() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "notes": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_sync_push_new_note() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Encrypted note content",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["accepted"].as_array().unwrap().len(), 1);
    assert_eq!(body["rejected"].as_array().unwrap().len(), 0);
    assert_eq!(body["errors"].as_array().unwrap().len(), 0);

    // Verify note was actually created
    let note = sqlx::query!(
        r#"SELECT id, content FROM notes WHERE id = ?"#,
        note_id
    )
    .fetch_one(&pool)
    .await
    .expect("Note should be in database");

    assert_eq!(note.content, "Encrypted note content");

    pool.close().await;
}

#[tokio::test]
async fn test_sync_pull_requires_authentication() {
    let (app, _pool) = create_test_app().await;

    let request = Request::builder()
        .uri("/api/v1/sync/pull")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "lastSyncAt": null,
                "knownNoteIds": [],
                "knownAttachmentIds": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_sync_pull_all_notes() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    // First push a note
    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let push_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Test content",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // Now pull notes
    let pull_request = Request::builder()
        .uri("/api/v1/sync/pull")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "lastSyncAt": null,
                "knownNoteIds": [],
                "knownAttachmentIds": []
            })
            .to_string(),
        ))
        .unwrap();

    let pull_response = app.oneshot(pull_request).await.unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);

    let body = parse_json_response(pull_response.into_body()).await;
    assert!(body["notes"].is_array());
    assert_eq!(body["notes"].as_array().unwrap().len(), 1);
    assert_eq!(body["notes"][0]["id"], note_id);
    assert_eq!(body["notes"][0]["content"], "Test content");

    pool.close().await;
}

#[tokio::test]
async fn test_sync_delete_note_requires_authentication() {
    let (app, _pool) = create_test_app().await;

    let note_id = uuid::Uuid::new_v4().to_string();

    let request = Request::builder()
        .uri(&format!("/api/v1/sync/notes/{}", note_id))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_sync_delete_note_success() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    // First push a note
    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let push_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "To be deleted",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // Now delete the note
    let delete_request = Request::builder()
        .uri(&format!("/api/v1/sync/notes/{}", note_id))
        .method("DELETE")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::empty())
        .unwrap();

    let delete_response = app.oneshot(delete_request).await.unwrap();

    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Verify note was actually deleted (hard delete)
    let note = sqlx::query!(
        r#"SELECT id FROM notes WHERE id = ?"#,
        note_id
    )
    .fetch_optional(&pool)
    .await
    .expect("Query should succeed");

    assert!(note.is_none(), "Note should be deleted from database");

    pool.close().await;
}

#[tokio::test]
async fn test_user_isolation_in_sync() {
    let (app, pool) = create_test_app().await;

    // Create two users with devices
    let api_key1 = create_test_user_and_device(&pool).await;
    let api_key2 = create_test_user_and_device(&pool).await;

    // User 1 pushes a note
    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let push_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key1))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "User 1's private note",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // User 2 tries to pull notes - should not see User 1's notes
    let pull_request = Request::builder()
        .uri("/api/v1/sync/pull")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key2))
        .body(Body::from(
            json!({
                "lastSyncAt": null,
                "knownNoteIds": [],
                "knownAttachmentIds": []
            })
            .to_string(),
        ))
        .unwrap();

    let pull_response = app.oneshot(pull_request).await.unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);

    let body = parse_json_response(pull_response.into_body()).await;
    assert_eq!(body["notes"].as_array().unwrap().len(), 0); // User 2 sees no notes

    pool.close().await;
}

#[tokio::test]
async fn test_sync_push_conflict_returns_full_server_data() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    // First push a note
    let note_id = uuid::Uuid::new_v4().to_string();
    let old_time = "2024-01-01T10:00:00Z";

    let push_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Original content",
                        "createdAt": old_time,
                        "modifiedAt": old_time,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": ["tag1"],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let push_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    assert_eq!(push_response.status(), StatusCode::OK);

    // Simulate another client updating the note by modifying the database directly
    let server_time = "2024-01-02T15:00:00Z";
    sqlx::query!(
        r#"UPDATE notes SET content = ?, modified_at = ?, pinned = ?, tags = ?, server_version = ?, syntax_language = ?, word_wrap = ?
           WHERE id = ?"#,
        "Server updated content",
        server_time,
        true,
        "[\"server-tag1\", \"server-tag2\"]",
        2i64,
        "markdown",
        true,
        note_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update note on server");

    // Now try to push outdated client version - should be rejected
    let client_time = "2024-01-01T12:00:00Z"; // Earlier than server's modified_at
    let conflict_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Client outdated content",
                        "createdAt": old_time,
                        "modifiedAt": client_time,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": ["client-tag"],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": false,
                        "syntaxLanguage": "plain"
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let conflict_response = app.oneshot(conflict_request).await.unwrap();

    assert_eq!(conflict_response.status(), StatusCode::OK);

    let body = parse_json_response(conflict_response.into_body()).await;

    // Verify rejection occurred
    assert_eq!(body["accepted"].as_array().unwrap().len(), 0);
    assert_eq!(body["rejected"].as_array().unwrap().len(), 1);

    // Verify rejected entry includes full server data for conflict resolution
    let rejected = &body["rejected"][0];
    assert_eq!(rejected["id"], note_id);
    assert!(rejected["reason"].as_str().unwrap().contains("newer"));

    // Verify server note data is included
    assert_eq!(rejected["serverContent"], "Server updated content");
    assert_eq!(rejected["serverModifiedAt"], server_time);
    assert_eq!(rejected["serverVersion"], 2);
    assert_eq!(rejected["serverPinned"], true);
    assert_eq!(rejected["serverSyntaxLanguage"], "markdown");
    assert_eq!(rejected["serverWordWrap"], true);

    // Verify server tags are included
    let server_tags = rejected["serverTags"].as_array().unwrap();
    assert_eq!(server_tags.len(), 2);
    assert!(server_tags.iter().any(|t| t == "server-tag1"));
    assert!(server_tags.iter().any(|t| t == "server-tag2"));

    // Verify server attachments array is present (empty in this case)
    assert!(rejected["serverAttachments"].is_array());

    pool.close().await;
}

#[tokio::test]
async fn test_sync_push_conflict_with_attachments() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    // First push a note
    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment_id = uuid::Uuid::new_v4().to_string();
    let old_time = "2024-01-01T10:00:00Z";

    let push_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Original content",
                        "createdAt": old_time,
                        "modifiedAt": old_time,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app.clone())
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // Get user_id from the note we just pushed
    let note_row = sqlx::query!(
        r#"SELECT user_id FROM notes WHERE id = ?"#,
        note_id
    )
    .fetch_one(&pool)
    .await
    .expect("Note should exist");
    let user_id = note_row.user_id;

    // Simulate another client updating the note with an attachment
    let server_time = "2024-01-02T15:00:00Z";
    let now = chrono::Utc::now().to_rfc3339();

    // Update note content and modified_at
    sqlx::query!(
        r#"UPDATE notes SET content = ?, modified_at = ?, version = ?
           WHERE id = ? AND user_id = ?"#,
        "Server content with attachment",
        server_time,
        2i64,
        note_id,
        user_id
    )
    .execute(&pool)
    .await
    .expect("Failed to update note on server");

    // Insert attachment into attachments_meta table (separate from notes table)
    sqlx::query!(
        r#"INSERT INTO attachments_meta (id, note_id, note_user_id, filename, mime_type, size, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        attachment_id,
        note_id,
        user_id,
        "server-file.pdf",
        "application/pdf",
        1024i64,
        now
    )
    .execute(&pool)
    .await
    .expect("Failed to insert attachment metadata");

    // Try to push outdated client version
    let client_time = "2024-01-01T12:00:00Z";
    let conflict_request = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Client content",
                        "createdAt": old_time,
                        "modifiedAt": client_time,
                        "deleted": false,
                        "deletedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let conflict_response = app.oneshot(conflict_request).await.unwrap();

    assert_eq!(conflict_response.status(), StatusCode::OK);

    let body = parse_json_response(conflict_response.into_body()).await;

    // Verify rejection with attachment data
    let rejected = &body["rejected"][0];
    let server_attachments = rejected["serverAttachments"].as_array().unwrap();
    assert_eq!(server_attachments.len(), 1);
    assert_eq!(server_attachments[0]["id"], attachment_id);
    assert_eq!(server_attachments[0]["filename"], "server-file.pdf");
    assert_eq!(server_attachments[0]["mimeType"], "application/pdf");
    assert_eq!(server_attachments[0]["size"], 1024);

    pool.close().await;
}
