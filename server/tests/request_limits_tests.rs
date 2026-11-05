// What a request can cost the server. Anonymous routes used to accept 100 MB bodies,
// and nothing bounded how many requests ran, or for how long.

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    routing::{get, post},
    Router,
};
use common::*;
use jottery_server::request_limits::{enforce, RequestLimits};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::Notify;
use tower::ServiceExt;

const OVER_SMALL_LIMIT: usize = 65 * 1024;

fn padding(size: usize) -> String {
    "x".repeat(size)
}

async fn send(app: &Router, uri: &str) -> StatusCode {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn anonymous_routes_refuse_a_large_body() {
    let db = migrated_db().await;
    let app = app(&db.pool);

    for uri in [
        "/api/v1/auth/login",
        "/api/v1/user/login",
        "/api/v1/user/status",
        "/api/v1/auth/register-user",
        "/api/v1/auth/register-device",
        "/api/v1/auth/clone-device",
    ] {
        let body = json!({ "email": "a@example.com", "password": padding(OVER_SMALL_LIMIT) });
        let (status, _) = call(&app, "POST", uri, None, Some(body)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{uri}");
    }
}

#[tokio::test]
async fn account_and_admin_routes_refuse_a_large_body() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let id = create_user(&db.pool, &email, &password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    let (_, login) = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    let token = login["sessionId"].as_str().unwrap();

    for uri in [
        "/api/v1/user/change-password".to_string(),
        "/api/v1/admin/change-password".to_string(),
        format!("/api/v1/admin/users/{id}/reset-password"),
    ] {
        let body = json!({ "new_password": padding(OVER_SMALL_LIMIT) });
        let (status, _) = call(&app, "POST", &uri, Some(token), Some(body)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{uri}");
    }
}

#[tokio::test]
async fn a_push_may_still_be_large() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    let id = new_id();

    let response = push(
        &app,
        &key,
        json!({ "notes": [note(&id, &padding(1024 * 1024), "2026-02-01T00:00:00Z")] }),
    )
    .await;

    assert_eq!(response["accepted"][0]["id"], json!(id));
}

#[tokio::test]
async fn a_pull_may_list_many_known_attachments() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    let known: Vec<String> = (0..5000).map(|_| new_id()).collect();

    pull(&app, &key, json!({ "knownAttachmentIds": known })).await;
}

#[tokio::test]
async fn the_inbox_takes_the_largest_item_it_allows() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let token = new_id();
    sqlx::query("UPDATE users SET inbox_token_hash = ? WHERE id = ?")
        .bind(jottery_server::utils::crypto::hash_sha256(&token))
        .bind(&user)
        .execute(&db.pool)
        .await
        .unwrap();

    // All quotes, so the JSON is twice the size of the content
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/inbox",
        Some(&token),
        Some(json!({ "content": "\"".repeat(1_048_576), "tags": [] })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[tokio::test]
async fn a_new_password_has_an_upper_limit() {
    let db = migrated_db().await;
    let app = app(&db.pool);

    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/auth/register-user",
        None,
        Some(json!({ "email": "long@example.com", "password": padding(1025) })),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("at most 1024"), "{body}");
}

#[tokio::test]
async fn a_long_password_set_before_the_limit_still_signs_in() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = padding(5000);
    create_user(&db.pool, &email, &password).await;

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
}

/// A router whose `/wait` and `/api/v1/sync/push` hold until `release` is notified.
fn held_router(limits: RequestLimits, release: Arc<Notify>) -> Router {
    let held = {
        let release = release.clone();
        move || {
            let release = release.clone();
            async move {
                release.notified().await;
                "done"
            }
        }
    };

    Router::new()
        .route("/wait", get(held.clone()))
        .route("/api/v1/sync/push", post(held))
        .route("/health", get(|| async { "OK" }))
        .route("/quick", get(|| async { "OK" }))
        .layer(axum::middleware::from_fn_with_state(limits, enforce))
}

const LONG: Duration = Duration::from_secs(60);

#[tokio::test]
async fn requests_over_the_limit_are_refused() {
    let release = Arc::new(Notify::new());
    let app = held_router(RequestLimits::new(1, 1, LONG, LONG), release.clone());

    let first = tokio::spawn({
        let app = app.clone();
        async move { send(&app, "/wait").await }
    });

    // The first request holds the only place until it's let go
    let mut refused = None;
    for _ in 0..1000 {
        let request = Request::builder().uri("/quick").body(Body::empty()).unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            refused = Some(response);
            break;
        }
        tokio::task::yield_now().await;
    }
    let refused = refused.expect("the second request was let through");
    assert!(refused.headers().contains_key(header::RETRY_AFTER));

    release.notify_one();
    assert_eq!(first.await.unwrap(), StatusCode::OK);
    assert_eq!(send(&app, "/quick").await, StatusCode::OK);
}

#[tokio::test]
async fn a_health_check_is_answered_however_busy_the_server_is() {
    let release = Arc::new(Notify::new());
    let app = held_router(RequestLimits::new(0, 0, LONG, LONG), release);

    assert_eq!(send(&app, "/quick").await, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(send(&app, "/health").await, StatusCode::OK);
}

#[tokio::test]
async fn a_request_that_takes_too_long_is_ended() {
    let release = Arc::new(Notify::new());
    let app = held_router(
        RequestLimits::new(10, 10, Duration::from_millis(20), LONG),
        release,
    );

    assert_eq!(send(&app, "/wait").await, StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn bulk_requests_wait_for_their_turn() {
    let release = Arc::new(Notify::new());
    let entered = Arc::new(AtomicUsize::new(0));
    let handler = {
        let (release, entered) = (release.clone(), entered.clone());
        move || {
            let (release, entered) = (release.clone(), entered.clone());
            async move {
                entered.fetch_add(1, Ordering::SeqCst);
                release.notified().await;
                "done"
            }
        }
    };
    let app = Router::new()
        .route("/api/v1/sync/push", post(handler))
        .layer(axum::middleware::from_fn_with_state(
            RequestLimits::new(10, 1, LONG, LONG),
            enforce,
        ));
    let push_once = |app: Router| async move {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/sync/push")
            .body(Body::empty())
            .unwrap();
        app.oneshot(request).await.unwrap().status()
    };

    let first = tokio::spawn(push_once(app.clone()));
    let second = tokio::spawn(push_once(app.clone()));

    for _ in 0..1000 {
        tokio::task::yield_now().await;
    }
    assert_eq!(entered.load(Ordering::SeqCst), 1, "only one may run at a time");

    // Let them go until both are done: neither may be refused
    for _ in 0..100_000 {
        if first.is_finished() && second.is_finished() {
            break;
        }
        release.notify_one();
        tokio::task::yield_now().await;
    }
    assert_eq!(first.await.unwrap(), StatusCode::OK);
    assert_eq!(second.await.unwrap(), StatusCode::OK);
    assert_eq!(entered.load(Ordering::SeqCst), 2);
}
