// A device that has been revoked, or whose account has gone, must stop hearing when
// the user syncs. A ping carries no content, but it does reveal activity.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
    Router,
};
use common::*;
use http_body_util::BodyExt;
use jottery_server::config::Config;
use std::time::Duration;
use tower::ServiceExt;

struct Fixture {
    db: TestDb,
    app: Router,
    user: String,
    device: String,
    key: String,
}

async fn setup() -> Fixture {
    let db = migrated_db().await;
    let config = Config { sse_recheck_secs: 1, ..test_config() };
    let app = app_with(&db.pool, config);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (device, key) = create_device(&db.pool, &user).await;
    Fixture { db, app, user, device, key }
}

impl Fixture {
    async fn token(&self) -> String {
        let (status, body) =
            call(&self.app, "GET", "/api/v1/sync/events/token", Some(&self.key), None).await;
        assert_eq!(status, StatusCode::OK);
        body["token"].as_str().unwrap().to_string()
    }

    async fn connect(&self, token: &str) -> Response {
        let uri = format!("/api/v1/sync/events?token={token}");
        let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
        self.app.clone().oneshot(request).await.unwrap()
    }

    async fn run(&self, sql: &str, id: &str) {
        sqlx::query(sql).bind(id).execute(&self.db.pool).await.unwrap();
    }
}

async fn assert_stream_ends(response: Response) {
    assert_eq!(response.status(), StatusCode::OK);
    tokio::time::timeout(Duration::from_secs(10), response.into_body().collect())
        .await
        .expect("the stream outlived the device")
        .unwrap();
}

#[tokio::test]
async fn a_token_stops_working_once_the_device_is_revoked() {
    let f = setup().await;
    let token = f.token().await;
    f.run("UPDATE clients SET is_active = 0 WHERE id = ?", &f.device).await;

    assert_eq!(f.connect(&token).await.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_token_stops_working_once_the_account_is_deactivated() {
    let f = setup().await;
    let token = f.token().await;
    f.run("UPDATE users SET is_active = 0 WHERE id = ?", &f.user).await;

    assert_eq!(f.connect(&token).await.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_token_stops_working_once_the_device_is_deleted() {
    let f = setup().await;
    let token = f.token().await;
    f.run("DELETE FROM clients WHERE id = ?", &f.device).await;

    assert_eq!(f.connect(&token).await.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_token_can_still_reconnect() {
    // A browser's EventSource reconnects with the same URL, so a token is not
    // single-use: the published rule is five minutes
    let f = setup().await;
    let token = f.token().await;

    assert_eq!(f.connect(&token).await.status(), StatusCode::OK);
    assert_eq!(f.connect(&token).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_open_stream_ends_when_the_device_is_revoked() {
    let f = setup().await;
    let token = f.token().await;
    let stream = f.connect(&token).await;

    f.run("UPDATE clients SET is_active = 0 WHERE id = ?", &f.device).await;

    assert_stream_ends(stream).await;
}

#[tokio::test]
async fn an_open_stream_ends_when_the_account_is_deactivated() {
    let f = setup().await;
    let token = f.token().await;
    let stream = f.connect(&token).await;

    f.run("UPDATE users SET is_active = 0 WHERE id = ?", &f.user).await;

    assert_stream_ends(stream).await;
}

#[tokio::test]
async fn an_open_stream_ends_when_the_account_is_deleted() {
    let f = setup().await;
    let token = f.token().await;
    let stream = f.connect(&token).await;

    f.run("DELETE FROM users WHERE id = ?", &f.user).await;

    assert_stream_ends(stream).await;
}

#[tokio::test]
async fn a_healthy_stream_stays_open() {
    let f = setup().await;
    let token = f.token().await;
    let stream = f.connect(&token).await;

    let outcome = tokio::time::timeout(Duration::from_millis(2500), stream.into_body().collect()).await;

    assert!(outcome.is_err(), "the stream ended by itself");
}
