// What a new account starts with. The DEFAULT_* settings are documented, and an
// operator who sets one expects it to apply.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::config::Config;
use serde_json::json;
use sqlx::Row;

#[tokio::test]
async fn a_new_user_gets_the_configured_limits() {
    let db = migrated_db().await;
    let config = Config {
        default_storage_quota_mb: 2500,
        default_max_upload_size_mb: 75,
        default_inbox_max_items: 42,
        default_inbox_max_size_mb: 7,
        ..test_config()
    };
    let app = app_with(&db.pool, config);

    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/auth/register-user",
        None,
        Some(json!({ "email": "new@example.com", "password": random_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    let row = sqlx::query(
        "SELECT storage_quota_mb, max_upload_size_mb, inbox_max_items, inbox_max_size_mb
         FROM users WHERE email = 'new@example.com'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<i64, _>(0), 2500);
    assert_eq!(row.get::<i64, _>(1), 75);
    assert_eq!(row.get::<i64, _>(2), 42);
    assert_eq!(row.get::<i64, _>(3), 7);
}

#[tokio::test]
async fn an_existing_user_keeps_the_limits_they_have() {
    let db = migrated_db().await;
    let id = create_user(&db.pool, "old@example.com", &random_password()).await;
    sqlx::query("UPDATE users SET max_upload_size_mb = 9 WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    let app = app_with(
        &db.pool,
        Config { default_max_upload_size_mb: 75, ..test_config() },
    );

    call(
        &app,
        "POST",
        "/api/v1/auth/register-user",
        None,
        Some(json!({ "email": "other@example.com", "password": random_password() })),
    )
    .await;

    let limit: i64 = sqlx::query_scalar("SELECT max_upload_size_mb FROM users WHERE id = ?")
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(limit, 9);
}
