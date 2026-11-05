// Logs on a privacy-focused service: they mustn't become a record of who signs in,
// and a value a client chose mustn't be able to forge a log line.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Captured {
    type Writer = Captured;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).to_string()
    }
}

/// Capture what this test's thread logs. Keep the guard for as long as it matters.
fn capture_logs() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    (captured, guard)
}

fn distinctive_email() -> String {
    format!("logged-{}@example.com", new_id())
}

#[tokio::test]
async fn signing_in_leaves_no_address_in_the_logs() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = distinctive_email();
    let password = random_password();
    let id = create_user(&db.pool, &email, &password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    let (logs, _guard) = capture_logs();

    for uri in ["/api/v1/auth/login", "/api/v1/user/login"] {
        let (status, _) = call(
            &app,
            "POST",
            uri,
            None,
            Some(json!({ "email": email, "password": password })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = call(
            &app,
            "POST",
            uri,
            None,
            Some(json!({ "email": email, "password": random_password() })),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    let text = logs.text();
    assert!(text.contains("Login"), "nothing was captured: {text}");
    assert!(!text.contains(&email), "{text}");
}

#[tokio::test]
async fn a_password_typed_in_the_address_field_is_not_logged() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let typed = random_password();
    let (logs, _guard) = capture_logs();

    for uri in ["/api/v1/auth/login", "/api/v1/user/login", "/api/v1/auth/register-user"] {
        call(
            &app,
            "POST",
            uri,
            None,
            Some(json!({ "email": typed, "password": random_password() })),
        )
        .await;
    }

    assert!(!logs.text().contains(&typed));
}

#[tokio::test]
async fn registering_leaves_no_address_in_the_logs() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = distinctive_email();
    let password = random_password();
    let (logs, _guard) = capture_logs();

    // New, then already taken
    for expected in [StatusCode::CREATED, StatusCode::CONFLICT] {
        let (status, _) = call(
            &app,
            "POST",
            "/api/v1/auth/register-user",
            None,
            Some(json!({ "email": email, "password": password })),
        )
        .await;
        assert_eq!(status, expected);
    }

    assert!(!logs.text().contains(&email));
}

#[tokio::test]
async fn enrolling_a_device_leaves_no_address_in_the_logs() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = distinctive_email();
    let password = random_password();
    create_user(&db.pool, &email, &password).await;
    let (logs, _guard) = capture_logs();

    for (candidate, expected) in [
        (password.clone(), StatusCode::CREATED),
        (random_password(), StatusCode::UNAUTHORIZED),
    ] {
        let (status, _) = call(
            &app,
            "POST",
            "/api/v1/auth/register-device",
            None,
            Some(json!({
                "email": email,
                "password": candidate,
                "deviceName": "phone",
                "deviceType": "ios",
            })),
        )
        .await;
        assert_eq!(status, expected);
    }

    assert!(!logs.text().contains(&email));
}

#[tokio::test]
async fn a_device_name_cannot_forge_a_log_line() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = distinctive_email();
    let password = random_password();
    create_user(&db.pool, &email, &password).await;
    let (logs, _guard) = capture_logs();

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/auth/register-device",
        None,
        Some(json!({
            "email": email,
            "password": password,
            "deviceName": "phone\nERROR forged line",
            "deviceType": "ios",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let text = logs.text();
    assert!(text.contains("forged"), "the name should appear, escaped: {text}");
    assert!(
        !text.lines().any(|line| line.starts_with("ERROR forged")),
        "{text}"
    );
}

#[tokio::test]
async fn an_id_in_a_push_cannot_forge_a_log_line() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;
    let (logs, _guard) = capture_logs();

    push(
        &app,
        &key,
        json!({ "deletions": [{ "id": "x\nERROR forged line", "deletedAt": "2026-01-01T00:00:00Z" }],
                "attachments": [{ "id": "y\nERROR forged line", "data": "not base64!" }] }),
    )
    .await;

    assert!(!logs.text().lines().any(|line| line.starts_with("ERROR forged")));
}
