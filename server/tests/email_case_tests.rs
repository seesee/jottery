// An address is one mailbox whatever case it's typed in. A phone keyboard that
// capitalises the first letter shouldn't lock anyone out, or make a second account.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::{db::run_migrations, services::AdminService};
use serde_json::json;
use sqlx::SqlitePool;

async fn login(app: &axum::Router, uri: &str, email: &str, password: &str) -> StatusCode {
    call(
        app,
        "POST",
        uri,
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await
    .0
}

async fn register(app: &axum::Router, email: &str) -> (StatusCode, serde_json::Value) {
    call(
        app,
        "POST",
        "/api/v1/auth/register-user",
        None,
        Some(json!({ "email": email, "password": random_password() })),
    )
    .await
}

async fn accounts_like(pool: &SqlitePool, email: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = ? COLLATE NOCASE")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn an_address_works_in_any_case() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let password = random_password();
    create_user(&db.pool, "chris@example.com", &password).await;

    for email in ["chris@example.com", "Chris@Example.com", "CHRIS@EXAMPLE.COM"] {
        assert_eq!(login(&app, "/api/v1/user/login", email, &password).await, StatusCode::OK, "{email}");
    }
}

#[tokio::test]
async fn an_account_registered_with_capitals_keeps_working() {
    // Stored as it was typed, before addresses were tidied on the way in
    let db = migrated_db().await;
    let app = app(&db.pool);
    let password = random_password();
    create_user(&db.pool, "Chris@Example.com", &password).await;

    for email in ["Chris@Example.com", "chris@example.com"] {
        assert_eq!(login(&app, "/api/v1/user/login", email, &password).await, StatusCode::OK, "{email}");
    }
}

#[tokio::test]
async fn spaces_around_an_address_are_ignored() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let password = random_password();
    create_user(&db.pool, "chris@example.com", &password).await;

    assert_eq!(login(&app, "/api/v1/user/login", " chris@example.com ", &password).await, StatusCode::OK);
}

#[tokio::test]
async fn every_route_that_takes_an_address_ignores_case() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let password = random_password();
    let id = create_user(&db.pool, "chris@example.com", &password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&id)
        .execute(&db.pool)
        .await
        .unwrap();
    let typed = "Chris@Example.com";

    assert_eq!(login(&app, "/api/v1/auth/login", typed, &password).await, StatusCode::OK);
    assert_eq!(login(&app, "/api/v1/user/status", typed, &password).await, StatusCode::OK);

    let (status, body) = call(
        &app,
        "GET",
        "/api/v1/user/status?email=Chris%40Example.com",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["exists"], json!(true));

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/auth/register-device",
        None,
        Some(json!({ "email": typed, "password": password, "deviceName": "phone", "deviceType": "ios" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let found = AdminService::get_user_info(&db.pool, typed).await.unwrap();
    assert_eq!(found.user.id, id);
}

#[tokio::test]
async fn a_new_address_is_stored_tidied() {
    let db = migrated_db().await;
    let app = app(&db.pool);

    let (status, body) = register(&app, " New.User@Example.com ").await;

    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["email"], json!("new.user@example.com"));
}

#[tokio::test]
async fn a_second_account_differing_only_by_case_is_refused() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    assert_eq!(register(&app, "someone@example.com").await.0, StatusCode::CREATED);

    assert_eq!(register(&app, "Someone@Example.com").await.0, StatusCode::CONFLICT);

    assert_eq!(accounts_like(&db.pool, "someone@example.com").await, 1);
}

#[tokio::test]
async fn an_existing_account_with_capitals_blocks_a_lookalike() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    create_user(&db.pool, "Someone@Example.com", &random_password()).await;

    assert_eq!(register(&app, "someone@example.com").await.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn racing_registrations_make_one_account() {
    let db = migrated_db().await;
    let app = app(&db.pool);

    let (first, second) = tokio::join!(
        register(&app, "racing@example.com"),
        register(&app, "Racing@example.com"),
    );

    let mut outcomes = [first.0, second.0];
    outcomes.sort();
    assert_eq!(outcomes, [StatusCode::CREATED, StatusCode::CONFLICT]);
    assert_eq!(accounts_like(&db.pool, "racing@example.com").await, 1);
}

#[tokio::test]
async fn accounts_that_already_collide_each_keep_their_own_password() {
    // The exact address decides, or one of them would be locked out
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (upper, lower) = (random_password(), random_password());
    let upper_id = create_user(&db.pool, "Twin@Example.com", &upper).await;
    let lower_id = create_user(&db.pool, "twin@example.com", &lower).await;
    assert_ne!(upper_id, lower_id);

    assert_eq!(login(&app, "/api/v1/user/login", "Twin@Example.com", &upper).await, StatusCode::OK);
    assert_eq!(login(&app, "/api/v1/user/login", "twin@example.com", &lower).await, StatusCode::OK);
    assert_eq!(
        login(&app, "/api/v1/user/login", "Twin@Example.com", &lower).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn migrating_a_database_with_colliding_accounts_succeeds() {
    let db = empty_db().await;
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&db.pool).await.unwrap();
    create_user(&db.pool, "Twin@Example.com", &random_password()).await;
    create_user(&db.pool, "twin@example.com", &random_password()).await;

    // Once more, the way the server starts, which also reports them
    run_migrations(&db.pool).await.expect("start with colliding accounts");

    assert_eq!(accounts_like(&db.pool, "twin@example.com").await, 2);
}
