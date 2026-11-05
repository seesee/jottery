//! Shared harness: a file-backed database opened the way `main` opens it, and
//! the real router from `build_app`.
#![allow(dead_code)]

use axum::{
    body::Body,
    http::{header::HeaderMap, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use jottery_server::{
    api,
    app::build_app,
    config::Config,
    db::init_pool,
    utils::{crypto::hash_sha256, password::hash_password_with_params},
    AppState,
};
use serde_json::Value;
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::broadcast;
use tower::ServiceExt;
use uuid::Uuid;

/// A temporary database file, removed when dropped.
pub struct TestDb {
    pub pool: SqlitePool,
    path: PathBuf,
}

impl Drop for TestDb {
    fn drop(&mut self) {
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.path.display(), suffix));
        }
    }
}

/// An empty database with no migrations applied.
pub async fn empty_db() -> TestDb {
    let path = std::env::temp_dir().join(format!("jottery-test-{}.db", Uuid::new_v4()));
    let pool = init_pool(&format!("sqlite:{}?mode=rwc", path.display()))
        .await
        .expect("open test database");
    TestDb { pool, path }
}

pub async fn migrated_db() -> TestDb {
    let db = empty_db().await;
    sqlx::migrate!("./migrations")
        .run(&db.pool)
        .await
        .expect("run migrations");
    db
}

pub fn test_config() -> Config {
    Config {
        database_url: String::new(),
        port: 0,
        max_payload_size: 104_857_600,
        cors_allowed_origins: None,
        session_expiry_days: 7,
        default_admin_email: "admin@localhost".to_string(),
        default_admin_password: None,
        // Cheap hashing: these tests aren't about Argon2 cost.
        argon2_m_cost: 1024,
        argon2_t_cost: 1,
        argon2_p_cost: 1,
        default_storage_quota_mb: 1000,
        default_max_upload_size_mb: 50,
        default_inbox_max_items: 100,
        default_inbox_max_size_mb: 10,
        password_complexity: "none".to_string(),
        enable_hsts: false,
        max_device_name_length: 255,
        max_inbox_content_size: 1_048_576,
        max_note_content_size: 10_485_760,
        max_tag_length: 100,
        max_tags_per_note: 50,
    }
}

pub fn app_with(pool: &SqlitePool, config: Config) -> Router {
    let (sync_broadcast, _) = broadcast::channel(100);
    build_app(Arc::new(AppState {
        pool: pool.clone(),
        config,
        sync_broadcast,
        sse_tokens: api::sse::create_token_store(),
    }))
}

pub fn app(pool: &SqlitePool) -> Router {
    app_with(pool, test_config())
}

/// Passwords are generated so that no credential is hard-coded in test code.
pub fn random_password() -> String {
    format!("Pw-{}", Uuid::new_v4())
}

pub async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let (status, _, value) = call_full(app, method, uri, bearer, body, &[]).await;
    (status, value)
}

/// As `call`, also taking extra request headers and returning response headers.
pub async fn call_full(
    app: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = bearer {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let body = body
        .map(|v| Body::from(v.to_string()))
        .unwrap_or_else(Body::empty);

    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let response_headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, response_headers, value)
}

/// Insert an approved, active user directly. Returns its id.
pub async fn create_user(pool: &SqlitePool, email: &str, password: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let hash = hash_password_with_params(password, 1024, 1, 1).unwrap();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, approved, is_active, created_at)
         VALUES (?, ?, ?, 1, 1, ?)",
    )
    .bind(&id)
    .bind(email)
    .bind(hash)
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Insert an active device directly. Returns (device id, API key).
pub async fn create_device(pool: &SqlitePool, user_id: &str) -> (String, String) {
    let id = Uuid::new_v4().to_string();
    let api_key = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
         VALUES (?, ?, ?, 'test device', 'web', ?, ?, 1)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(hash_sha256(&api_key))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
    (id, api_key)
}
