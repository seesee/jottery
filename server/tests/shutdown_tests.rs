// Stopping cleanly: `docker stop` sends SIGTERM and waits ten seconds. Without
// handling it the server dies mid-request, and with graceful shutdown alone an
// open event stream would hold it up until it gets killed anyway.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::*;
use http_body_util::BodyExt;
use jottery_server::{api, app::build_app, AppState};
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tower::ServiceExt;

#[tokio::test]
async fn an_event_stream_ends_when_the_server_stops() {
    let db = migrated_db().await;
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, api_key) = create_device(&db.pool, &user).await;
    let (sync_broadcast, _) = broadcast::channel(100);
    let state = Arc::new(AppState {
        pool: db.pool.clone(),
        config: test_config(),
        sync_broadcast,
        sse_tokens: api::sse::create_token_store(),
        login_throttle: Default::default(),
        shutdown: Default::default(),
    });
    let app = build_app(state.clone());

    let (status, token) = call(&app, "GET", "/api/v1/sync/events/token", Some(&api_key), None).await;
    assert_eq!(status, StatusCode::OK);
    let uri = format!("/api/v1/sync/events?token={}", token["token"].as_str().unwrap());
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let stream = tokio::spawn(async move { response.into_body().collect().await });
    state.shutdown.trigger();

    tokio::time::timeout(Duration::from_secs(10), stream)
        .await
        .expect("the stream outlived the server")
        .unwrap()
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_stops_the_server_cleanly() {
    let dir = std::env::temp_dir().join(format!("jottery-shutdown-{}", new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let mut server = tokio::process::Command::new(env!("CARGO_BIN_EXE_jottery-server"))
        .current_dir(&dir)
        .env("DATABASE_URL", format!("sqlite:{}?mode=rwc", dir.join("jottery.db").display()))
        .env("PORT", port.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    let health = format!("http://127.0.0.1:{port}/health");
    let mut ready = false;
    for _ in 0..200 {
        if reqwest::get(&health).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "the server never came up");

    let pid = server.id().unwrap().to_string();
    let killed = std::process::Command::new("kill").args(["-TERM", &pid]).status().unwrap();
    assert!(killed.success());

    let status = tokio::time::timeout(Duration::from_secs(10), server.wait())
        .await
        .expect("the server ignored SIGTERM")
        .unwrap();
    assert_eq!(status.code(), Some(0), "exited with {status:?}");

    let _ = std::fs::remove_dir_all(&dir);
}
