// Attachment sync and version-based conflict resolution integration tests
//
// Tests for:
// - Pushing notes with base64-encoded attachments
// - Pulling notes with attachment data
// - Conflict detection and version creation
// - Version history retrieval
// - Large attachment handling

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::Digest;
use sqlx::SqlitePool;
use tokio::sync::broadcast;
use std::sync::Arc;
use tower::{Service, ServiceExt};

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
            default_inbox_max_items: 100,
            default_inbox_max_size_mb: 10,
        password_complexity: "none".to_string(),
        enable_hsts: false,
        max_device_name_length: 255,
        max_inbox_content_size: 1_048_576,
        max_note_content_size: 10_485_760,
        max_tag_length: 100,
        max_tags_per_note: 50,
    };

    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool: pool.clone(),
        sync_broadcast,
        sse_tokens,
        config,
    });

    // Build router with sync routes
    let app = axum::Router::new()
        .route("/api/v1/sync/push", axum::routing::post(jottery_server::api::sync::push))
        .route("/api/v1/sync/pull", axum::routing::post(jottery_server::api::sync::pull))
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

// ========== Attachment Tests ==========

#[tokio::test]
async fn test_push_note_with_attachment() {
    let (mut app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Create sample attachment data (small text file)
    let attachment_data = "Hello, this is a test attachment!";
    let attachment_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        attachment_data.as_bytes()
    );

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
                        "content": "Note with attachment",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [
                            {
                                "id": attachment_id,
                                "filename": "test.txt",
                                "mimeType": "text/plain",
                                "size": attachment_data.len(),
                                "data": attachment_id
                            }
                        ],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [
                    {
                        "id": attachment_id,
                        "data": attachment_base64
                    }
                ],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = parse_json_response(response.into_body()).await;
    assert_eq!(body["accepted"].as_array().unwrap().len(), 1);
    assert_eq!(body["rejected"].as_array().unwrap().len(), 0);

    // Verify attachment metadata was stored
    let attachment_meta = sqlx::query!(
        r#"SELECT id, filename, mime_type, size FROM attachments_meta WHERE id = ?"#,
        attachment_id
    )
    .fetch_one(&pool)
    .await
    .expect("Attachment metadata should be stored");

    assert_eq!(attachment_meta.filename, "test.txt");
    assert_eq!(attachment_meta.mime_type, "text/plain");
    assert_eq!(attachment_meta.size, attachment_data.len() as i64);

    // Verify attachment data was stored
    let attachment_data_record = sqlx::query!(
        r#"SELECT id, data FROM attachments_data WHERE id = ?"#,
        attachment_id
    )
    .fetch_one(&pool)
    .await
    .expect("Attachment data should be stored");

    let decoded_data = String::from_utf8(attachment_data_record.data).unwrap();
    assert_eq!(decoded_data, attachment_data);

    pool.close().await;
}

#[tokio::test]
async fn test_pull_note_with_attachment() {
    let (mut app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let attachment_data = "Test attachment content for pull";
    let attachment_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        attachment_data.as_bytes()
    );

    // First push a note with attachment
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
                        "content": "Note with attachment",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [
                            {
                                "id": attachment_id,
                                "filename": "document.txt",
                                "mimeType": "text/plain",
                                "size": attachment_data.len(),
                                "data": attachment_id
                            }
                        ],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [
                    {
                        "id": attachment_id,
                        "data": attachment_base64
                    }
                ],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // Now pull the note - should include attachment
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

    let pull_response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(pull_request)
        .await
        .unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);

    let body = parse_json_response(pull_response.into_body()).await;

    // Verify note includes attachment reference
    let notes = body["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1);

    let attachments = notes[0]["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0]["id"], attachment_id);
    assert_eq!(attachments[0]["filename"], "document.txt");
    assert_eq!(attachments[0]["mimeType"], "text/plain");

    // Verify attachment data is included
    let attachment_data_array = body["attachments"].as_array().unwrap();
    assert_eq!(attachment_data_array.len(), 1);
    assert_eq!(attachment_data_array[0]["id"], attachment_id);

    // Decode and verify attachment data
    let returned_data_base64 = attachment_data_array[0]["data"].as_str().unwrap();
    let returned_data = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        returned_data_base64
    ).unwrap();
    assert_eq!(String::from_utf8(returned_data).unwrap(), attachment_data);

    pool.close().await;
}

#[tokio::test]
async fn test_pull_excludes_known_attachments() {
    let (mut app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let attachment_data = "Test attachment";
    let attachment_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        attachment_data.as_bytes()
    );

    // Push note with attachment
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
                        "content": "Note",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [
                            {
                                "id": attachment_id,
                                "filename": "file.txt",
                                "mimeType": "text/plain",
                                "size": attachment_data.len(),
                                "data": attachment_id
                            }
                        ],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [
                    {
                        "id": attachment_id,
                        "data": attachment_base64
                    }
                ],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // Pull with knownAttachmentIds - should NOT return attachment data
    let pull_request = Request::builder()
        .uri("/api/v1/sync/pull")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "lastSyncAt": null,
                "knownNoteIds": [],
                "knownAttachmentIds": [attachment_id]
            })
            .to_string(),
        ))
        .unwrap();

    let pull_response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(pull_request)
        .await
        .unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);

    let body = parse_json_response(pull_response.into_body()).await;

    // Note should still include attachment reference
    let attachments = body["notes"][0]["attachments"].as_array().unwrap();
    assert_eq!(attachments.len(), 1);

    // But attachment data array should be empty
    let attachment_data_array = body["attachments"].as_array().unwrap();
    assert_eq!(attachment_data_array.len(), 0, "Should skip known attachments");

    pool.close().await;
}

#[tokio::test]
async fn test_multiple_attachments_per_note() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment1_id = uuid::Uuid::new_v4().to_string();
    let attachment2_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let data1 = "First attachment";
    let data2 = "Second attachment";
    let base64_1 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data1.as_bytes());
    let base64_2 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data2.as_bytes());

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
                        "content": "Note with multiple attachments",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [
                            {
                                "id": attachment1_id,
                                "filename": "first.txt",
                                "mimeType": "text/plain",
                                "size": data1.len(),
                                "data": attachment1_id
                            },
                            {
                                "id": attachment2_id,
                                "filename": "second.txt",
                                "mimeType": "text/plain",
                                "size": data2.len(),
                                "data": attachment2_id
                            }
                        ],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [
                    {
                        "id": attachment1_id,
                        "data": base64_1
                    },
                    {
                        "id": attachment2_id,
                        "data": base64_2
                    }
                ],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify both attachments were stored
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM attachments_meta WHERE note_id = ?"
    )
    .bind(&note_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count, 2, "Should have 2 attachments");

    pool.close().await;
}

// ========== Conflict Resolution & Versioning Tests ==========

#[tokio::test]
async fn test_conflict_older_update_rejected() {
    let (mut app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let time1 = now.to_rfc3339();
    let time2 = (now + chrono::Duration::seconds(60)).to_rfc3339();

    // Push note at time2 (newer)
    let push1 = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Newer version",
                        "createdAt": time1,
                        "modifiedAt": time2,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 2,
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

    let response1 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push1)
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = parse_json_response(response1.into_body()).await;
    assert_eq!(body1["accepted"].as_array().unwrap().len(), 1);

    // Try to push older version at time1 (should be rejected)
    let push2 = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Older version",
                        "createdAt": time1,
                        "modifiedAt": time1,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
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

    let response2 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push2)
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = parse_json_response(response2.into_body()).await;

    assert_eq!(body2["accepted"].as_array().unwrap().len(), 0);
    assert_eq!(body2["rejected"].as_array().unwrap().len(), 1);

    let rejected = &body2["rejected"][0];
    assert_eq!(rejected["id"], note_id);
    assert!(rejected["reason"].as_str().unwrap().contains("newer"));

    // Verify server still has the newer version
    let note = sqlx::query!(
        r#"SELECT content, modified_at FROM notes WHERE id = ?"#,
        note_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(note.content, "Newer version");
    assert_eq!(note.modified_at, time2);

    pool.close().await;
}

#[tokio::test]
async fn test_conflict_newer_update_accepted() {
    let (mut app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let time1 = now.to_rfc3339();
    let time2 = (now + chrono::Duration::seconds(60)).to_rfc3339();

    // Push older version first
    let push1 = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Older version",
                        "createdAt": time1,
                        "modifiedAt": time1,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
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

    let response1 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push1)
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);

    // Push newer version (should be accepted and overwrite)
    let push2 = Request::builder()
        .uri("/api/v1/sync/push")
        .method("POST")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .body(Body::from(
            json!({
                "notes": [
                    {
                        "id": note_id,
                        "content": "Newer version",
                        "createdAt": time1,
                        "modifiedAt": time2,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 2,
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

    let response2 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push2)
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = parse_json_response(response2.into_body()).await;

    assert_eq!(body2["accepted"].as_array().unwrap().len(), 1);
    assert_eq!(body2["rejected"].as_array().unwrap().len(), 0);

    // Verify server has the newer version
    let note = sqlx::query!(
        r#"SELECT content, modified_at, server_version FROM notes WHERE id = ?"#,
        note_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(note.content, "Newer version");
    assert_eq!(note.modified_at, time2);
    assert_eq!(note.server_version, 2); // Server version incremented

    pool.close().await;
}

#[tokio::test]
async fn test_version_history_creation() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let version_key = format!("{}:1", note_id);

    // Push note with version history
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
                        "content": "Current content",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": ["tag1"],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": [
                    {
                        "versionKey": version_key,
                        "noteId": note_id,
                        "version": 1,
                        "createdAt": now,
                        "syncedAt": now,
                        "content": "Historical content",
                        "tags": ["old-tag"],
                        "attachments": [],
                        "syntaxLanguage": null,
                        "wordWrap": null,
                        "reason": "manual-snapshot"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify version was stored
    let version = sqlx::query!(
        r#"SELECT version_key, content, reason FROM note_versions WHERE version_key = ?"#,
        version_key
    )
    .fetch_one(&pool)
    .await
    .expect("Version should be stored");

    assert_eq!(version.content, "Historical content");
    assert_eq!(version.reason, "manual-snapshot");

    pool.close().await;
}

#[tokio::test]
async fn test_pull_includes_versions() {
    let (mut app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let version_key = format!("{}:1", note_id);

    // Push note with version
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
                        "content": "Current",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [],
                "versions": [
                    {
                        "versionKey": version_key,
                        "noteId": note_id,
                        "version": 1,
                        "createdAt": now,
                        "syncedAt": now,
                        "content": "Version content",
                        "tags": [],
                        "attachments": [],
                        "syntaxLanguage": null,
                        "wordWrap": null,
                        "reason": "sync"
                    }
                ]
            })
            .to_string(),
        ))
        .unwrap();

    let _push_response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(push_request)
        .await
        .unwrap();

    // Pull should include versions
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

    let pull_response = ServiceExt::<Request<Body>>::ready(&mut app)
        .await
        .unwrap()
        .call(pull_request)
        .await
        .unwrap();

    assert_eq!(pull_response.status(), StatusCode::OK);

    let body = parse_json_response(pull_response.into_body()).await;

    // Verify versions are included
    let versions = body["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["versionKey"], version_key);
    assert_eq!(versions[0]["content"], "Version content");
    assert_eq!(versions[0]["reason"], "sync");

    pool.close().await;
}

#[tokio::test]
async fn test_large_attachment_handling() {
    let (app, pool) = create_test_app().await;

    let api_key = create_test_user_and_device(&pool).await;

    let note_id = uuid::Uuid::new_v4().to_string();
    let attachment_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Create a 1MB attachment
    let large_data = vec![b'A'; 1024 * 1024];
    let large_base64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &large_data
    );

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
                        "content": "Note with large attachment",
                        "createdAt": now,
                        "modifiedAt": now,
                        "deleted": false,
                        "deletedAt": null,
                        "archived": false,
                        "archivedAt": null,
                        "tags": [],
                        "attachments": [
                            {
                                "id": attachment_id,
                                "filename": "large.bin",
                                "mimeType": "application/octet-stream",
                                "size": large_data.len(),
                                "data": attachment_id
                            }
                        ],
                        "pinned": false,
                        "version": 1,
                        "wordWrap": null,
                        "syntaxLanguage": null
                    }
                ],
                "attachments": [
                    {
                        "id": attachment_id,
                        "data": large_base64
                    }
                ],
                "versions": []
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify attachment was stored correctly
    let stored = sqlx::query!(
        r#"SELECT data FROM attachments_data WHERE id = ?"#,
        attachment_id
    )
    .fetch_one(&pool)
    .await
    .expect("Large attachment should be stored");

    assert_eq!(stored.data.len(), 1024 * 1024);

    pool.close().await;
}
