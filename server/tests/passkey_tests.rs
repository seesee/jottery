//! Phase 1 passkey tests.
//!
//! We deliberately don't round-trip a full WebAuthn register/authenticate
//! flow here — that needs a software authenticator (webauthn-authenticator-rs)
//! which we don't currently depend on, and the value of testing the
//! middleware/endpoint glue is higher than testing the upstream
//! crate's verification logic. The tests here cover:
//!
//! - MFA-pending sessions are rejected on protected endpoints
//! - MFA-pending sessions ARE accepted on the passkey-authenticate routes
//! - Login returns `mfaRequired: true` when the user has ≥ 1 passkey
//! - Login returns `mfaRequired: false` when they don't
//! - Passkey endpoints return 404 when WebAuthn isn't configured
//! - Listing and deleting passkeys respects user ownership

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::{delete, get, post},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower::ServiceExt;

async fn setup_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

fn test_config() -> jottery_server::config::Config {
    jottery_server::config::Config {
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
        webauthn_rp_id: None,
        webauthn_rp_origin: None,
        webauthn_rp_name: None,
    }
}

fn test_webauthn_config() -> jottery_server::config::Config {
    jottery_server::config::Config {
        webauthn_rp_id: Some("localhost".to_string()),
        webauthn_rp_origin: Some("http://localhost:5174".to_string()),
        webauthn_rp_name: Some("Jottery Test".to_string()),
        ..test_config()
    }
}

fn build_webauthn(cfg: &jottery_server::config::Config) -> Option<Arc<webauthn_rs::Webauthn>> {
    use webauthn_rs::prelude::*;
    let url = Url::parse(cfg.webauthn_rp_origin.as_ref()?).ok()?;
    let builder = WebauthnBuilder::new(cfg.webauthn_rp_id.as_ref()?, &url).ok()?;
    Some(Arc::new(builder.rp_name(cfg.webauthn_rp_name.as_ref()?).build().ok()?))
}

async fn build_app(pool: SqlitePool, enable_webauthn: bool) -> Router {
    let config = if enable_webauthn { test_webauthn_config() } else { test_config() };
    let webauthn = if enable_webauthn { build_webauthn(&config) } else { None };
    let (sync_broadcast, _) = broadcast::channel(100);
    let sse_tokens = jottery_server::api::sse::create_token_store();
    let app_state = Arc::new(jottery_server::AppState {
        pool,
        sync_broadcast,
        sse_tokens,
        config,
        webauthn,
    });

    let protected = Router::new()
        .route("/api/v1/user/account", get(jottery_server::api::user::get_account_info))
        .route("/api/v1/user/passkeys/register/begin", post(jottery_server::api::passkeys::begin_registration))
        .route("/api/v1/user/passkeys", get(jottery_server::api::passkeys::list_passkeys))
        .route("/api/v1/user/passkeys/:id", delete(jottery_server::api::passkeys::delete_passkey))
        .layer(middleware::from_fn_with_state(app_state.clone(), jottery_server::api::middleware::user_auth_middleware));

    let mfa_pending = Router::new()
        .route("/api/v1/user/passkeys/authenticate/begin", post(jottery_server::api::passkeys::begin_authentication))
        .layer(middleware::from_fn_with_state(app_state.clone(), jottery_server::api::middleware::user_mfa_pending_middleware));

    Router::new()
        .route("/api/v1/user/login", post(jottery_server::api::user::login))
        .merge(protected)
        .merge(mfa_pending)
        .with_state(app_state)
}

async fn make_user(pool: &SqlitePool, email: &str, password: &str) -> String {
    use jottery_server::utils::password::hash_password_with_params;
    let user_id = uuid::Uuid::new_v4().to_string();
    let hash = hash_password_with_params(password, 19456, 2, 1).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, is_admin, is_active, created_at)
           VALUES (?, ?, ?, 1, 0, 1, ?)"#,
        user_id, email, hash, now,
    ).execute(pool).await.unwrap();
    user_id
}

/// Insert a session row directly. mfa_pending: if true, mfa_verified=0.
async fn make_session(pool: &SqlitePool, user_id: &str, mfa_pending: bool) -> String {
    let token: String = (0..32).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
    let id = uuid::Uuid::new_v4().to_string();
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    let now = chrono::Utc::now().to_rfc3339();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();
    let mfa_verified: i64 = if mfa_pending { 0 } else { 1 };
    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at, last_used_at, mfa_verified)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id, user_id, token_hash, expires_at, now, now, mfa_verified,
    ).execute(pool).await.unwrap();
    token
}

/// Insert a fake user_credentials row. The `passkey_json` blob doesn't
/// need to be a real verifiable passkey for these tests — we're only
/// asserting that the server sees "user has ≥ 1 credential".
async fn stub_passkey(pool: &SqlitePool, user_id: &str, nickname: &str) -> String {
    let cred_id = format!("stub-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query!(
        r#"INSERT INTO user_credentials (id, user_id, passkey_json, nickname, created_at)
           VALUES (?, ?, '{}', ?, ?)"#,
        cred_id, user_id, nickname, now,
    ).execute(pool).await.unwrap();
    cred_id
}

async fn response_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}))
}

#[tokio::test]
async fn test_login_without_passkeys_returns_mfa_false() {
    let pool = setup_db().await;
    make_user(&pool, "plain@example.com", "secretpw12345").await;
    let app = build_app(pool.clone(), true).await;

    let req = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(json!({"email": "plain@example.com", "password": "secretpw12345"}).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp.into_body()).await;
    // mfaRequired is skipped when false (serde skip_serializing_if)
    assert!(body.get("mfaRequired").is_none() || body["mfaRequired"] == false);
    pool.close().await;
}

#[tokio::test]
async fn test_login_with_passkeys_returns_mfa_true() {
    let pool = setup_db().await;
    let user_id = make_user(&pool, "has-passkey@example.com", "secretpw12345").await;
    stub_passkey(&pool, &user_id, "laptop").await;
    let app = build_app(pool.clone(), true).await;

    let req = Request::builder()
        .uri("/api/v1/user/login")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(json!({"email": "has-passkey@example.com", "password": "secretpw12345"}).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp.into_body()).await;
    assert_eq!(body["mfaRequired"], true);

    // The session that was just issued should be in mfa_verified = 0 state.
    let token = body["sessionId"].as_str().unwrap();
    let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
    let mfa: i64 = sqlx::query_scalar!(
        "SELECT mfa_verified FROM sessions WHERE token_hash = ?",
        token_hash,
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(mfa, 0);

    pool.close().await;
}

#[tokio::test]
async fn test_mfa_pending_session_rejected_on_protected_endpoint() {
    let pool = setup_db().await;
    let user_id = make_user(&pool, "staged@example.com", "secretpw12345").await;
    let token = make_session(&pool, &user_id, /* mfa_pending = */ true).await;
    let app = build_app(pool.clone(), true).await;

    let req = Request::builder()
        .uri("/api/v1/user/account")
        .method("GET")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED,
        "mfa-pending session must not reach protected endpoints");
    pool.close().await;
}

#[tokio::test]
async fn test_mfa_pending_session_accepted_on_authenticate_begin() {
    let pool = setup_db().await;
    let user_id = make_user(&pool, "staged2@example.com", "secretpw12345").await;
    stub_passkey(&pool, &user_id, "device").await;
    let token = make_session(&pool, &user_id, /* mfa_pending = */ true).await;
    let app = build_app(pool.clone(), true).await;

    let req = Request::builder()
        .uri("/api/v1/user/passkeys/authenticate/begin")
        .method("POST")
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // The stub passkey isn't a valid webauthn-rs Passkey JSON so the
    // handler won't manage to construct the challenge, but it MUST get
    // past the middleware — the status must not be UNAUTHORIZED.
    assert_ne!(resp.status(), StatusCode::UNAUTHORIZED,
        "authenticate endpoints must accept mfa-pending sessions");
    pool.close().await;
}

#[tokio::test]
async fn test_passkey_endpoints_404_when_webauthn_disabled() {
    let pool = setup_db().await;
    let user_id = make_user(&pool, "nopasskey@example.com", "secretpw12345").await;
    let token = make_session(&pool, &user_id, false).await;
    let app = build_app(pool.clone(), /* enable_webauthn = */ false).await;

    let req = Request::builder()
        .uri("/api/v1/user/passkeys")
        .method("GET")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    pool.close().await;
}

#[tokio::test]
async fn test_list_passkeys_only_returns_callers_credentials() {
    let pool = setup_db().await;
    let alice = make_user(&pool, "alice@example.com", "secretpw12345").await;
    let bob = make_user(&pool, "bob@example.com", "secretpw12345").await;
    stub_passkey(&pool, &alice, "alice-phone").await;
    stub_passkey(&pool, &alice, "alice-laptop").await;
    stub_passkey(&pool, &bob, "bob-phone").await;

    let alice_token = make_session(&pool, &alice, false).await;
    let app = build_app(pool.clone(), true).await;

    let req = Request::builder()
        .uri("/api/v1/user/passkeys")
        .method("GET")
        .header("authorization", format!("Bearer {}", alice_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp.into_body()).await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2, "Alice should see her own two passkeys, not Bob's");
    let nicknames: Vec<&str> = items.iter().map(|i| i["nickname"].as_str().unwrap()).collect();
    assert!(nicknames.contains(&"alice-phone"));
    assert!(nicknames.contains(&"alice-laptop"));
    pool.close().await;
}

#[tokio::test]
async fn test_delete_passkey_refuses_to_touch_another_users_credential() {
    let pool = setup_db().await;
    let alice = make_user(&pool, "alice2@example.com", "secretpw12345").await;
    let bob = make_user(&pool, "bob2@example.com", "secretpw12345").await;
    let bobs_cred = stub_passkey(&pool, &bob, "bob-phone").await;
    let alice_token = make_session(&pool, &alice, false).await;
    let app = build_app(pool.clone(), true).await;

    let req = Request::builder()
        .uri(format!("/api/v1/user/passkeys/{}", bobs_cred))
        .method("DELETE")
        .header("authorization", format!("Bearer {}", alice_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND,
        "Alice must not be able to delete Bob's credential");

    // And confirm Bob's credential is still there.
    let count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM user_credentials WHERE id = ?", bobs_cred
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    pool.close().await;
}
