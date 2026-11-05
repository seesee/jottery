// The seeded admin account: a fresh install must not start with a known
// password, the configured credentials must take effect, and an existing
// install still on the default must keep working but be forced to change it.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::bootstrap::{ensure_default_admin, AdminBootstrap, SEED_ADMIN_ID};
use serde_json::json;
use sqlx::SqlitePool;

const SEED_EMAIL: &str = "admin@localhost";

/// The published password from migration 003, assembled at runtime.
fn seed_password() -> String {
    ["change", "me"].concat()
}

async fn admin_login(app: &axum::Router, email: &str, password: &str) -> (StatusCode, serde_json::Value) {
    call(
        app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await
}

async fn must_change(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT must_change_password FROM users WHERE id = ?")
        .bind(SEED_ADMIN_ID)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Marks the database as an install that has already been used.
async fn make_existing_install(pool: &SqlitePool) {
    create_device(pool, SEED_ADMIN_ID).await;
}

#[tokio::test]
async fn configured_credentials_replace_the_seed_on_a_fresh_install() {
    let db = migrated_db().await;
    let password = random_password();
    let mut config = test_config();
    config.default_admin_email = "boss@example.com".to_string();
    config.default_admin_password = Some(password.clone());

    let outcome = ensure_default_admin(&db.pool, &config).await.unwrap();
    assert_eq!(outcome, AdminBootstrap::AppliedConfigured);

    let app = app(&db.pool);
    let (status, body) = admin_login(&app, "boss@example.com", &password).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mustChangePassword"], false);

    let (status, _) = admin_login(&app, SEED_EMAIL, &seed_password()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = admin_login(&app, "boss@example.com", &seed_password()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn fresh_install_without_configuration_gets_a_generated_password() {
    let db = migrated_db().await;

    let outcome = ensure_default_admin(&db.pool, &test_config()).await.unwrap();
    let AdminBootstrap::GeneratedPassword(generated) = outcome else {
        panic!("expected a generated password, got {outcome:?}");
    };
    assert!(generated.len() >= 24);

    let app = app(&db.pool);
    let (status, _) = admin_login(&app, SEED_EMAIL, &seed_password()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the published password must not work");

    let (status, body) = admin_login(&app, SEED_EMAIL, &generated).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mustChangePassword"], true);
}

#[tokio::test]
async fn the_published_password_is_not_accepted_from_the_environment() {
    let db = migrated_db().await;
    let mut config = test_config();
    config.default_admin_password = Some(seed_password());
    // Even with the policy at its weakest
    config.password_complexity = "none".to_string();

    let outcome = ensure_default_admin(&db.pool, &config).await.unwrap();
    assert!(matches!(outcome, AdminBootstrap::GeneratedPassword(_)), "got {outcome:?}");

    let (status, _) = admin_login(&app(&db.pool), SEED_EMAIL, &seed_password()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_unused_generated_password_is_replaced_on_restart() {
    let db = migrated_db().await;
    let config = test_config();

    let first = ensure_default_admin(&db.pool, &config).await.unwrap();
    let second = ensure_default_admin(&db.pool, &config).await.unwrap();
    let (AdminBootstrap::GeneratedPassword(a), AdminBootstrap::GeneratedPassword(b)) = (first, second) else {
        panic!("expected two generated passwords");
    };
    assert_ne!(a, b);

    // Once someone has signed in, restarting must leave it alone.
    let app = app(&db.pool);
    assert_eq!(admin_login(&app, SEED_EMAIL, &b).await.0, StatusCode::OK);
    let third = ensure_default_admin(&db.pool, &config).await.unwrap();
    assert_eq!(third, AdminBootstrap::AlreadyConfigured);
    assert_eq!(admin_login(&app, SEED_EMAIL, &b).await.0, StatusCode::OK);
}

#[tokio::test]
async fn an_existing_install_on_the_default_is_not_locked_out() {
    let db = migrated_db().await;
    make_existing_install(&db.pool).await;

    let outcome = ensure_default_admin(&db.pool, &test_config()).await.unwrap();
    assert_eq!(outcome, AdminBootstrap::ForcedChange);
    assert_eq!(must_change(&db.pool).await, 1);

    let (status, body) = admin_login(&app(&db.pool), SEED_EMAIL, &seed_password()).await;
    assert_eq!(status, StatusCode::OK, "the operator must still be able to sign in");
    assert_eq!(body["mustChangePassword"], true);
}

#[tokio::test]
async fn pending_registrations_do_not_make_an_install_existing() {
    let db = migrated_db().await;
    let user = create_user(&db.pool, "pending@example.com", &random_password()).await;
    sqlx::query("UPDATE users SET approved = 0 WHERE id = ?")
        .bind(&user)
        .execute(&db.pool)
        .await
        .unwrap();

    let outcome = ensure_default_admin(&db.pool, &test_config()).await.unwrap();
    assert!(matches!(outcome, AdminBootstrap::GeneratedPassword(_)), "got {outcome:?}");
}

#[tokio::test]
async fn forced_change_blocks_everything_until_the_password_is_changed() {
    let db = migrated_db().await;
    make_existing_install(&db.pool).await;
    ensure_default_admin(&db.pool, &test_config()).await.unwrap();

    let app = app(&db.pool);
    let (_, body) = admin_login(&app, SEED_EMAIL, &seed_password()).await;
    let session = body["sessionId"].as_str().unwrap().to_string();

    for (method, uri) in [
        ("GET", "/api/v1/admin/users"),
        ("GET", "/api/v1/admin/stats"),
        ("GET", "/api/v1/admin/audit"),
        ("GET", "/api/v1/user/account"),
        ("GET", "/api/v1/user/devices"),
    ] {
        let (status, body) = call(&app, method, uri, Some(&session), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        assert_eq!(body["code"], "password_change_required", "{uri}");
    }

    // A device can't be enrolled with the default either
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/auth/register-device",
        None,
        Some(json!({
            "email": SEED_EMAIL,
            "password": seed_password(),
            "deviceName": "attacker",
            "deviceType": "web",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "password_change_required");

    let new_password = random_password();
    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/admin/change-password",
        Some(&session),
        Some(json!({ "current_password": seed_password(), "new_password": new_password })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(must_change(&db.pool).await, 0);

    let (status, _) = call(&app, "GET", "/api/v1/admin/users", Some(&session), None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = admin_login(&app, SEED_EMAIL, &seed_password()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = admin_login(&app, SEED_EMAIL, &new_password).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mustChangePassword"], false);
}

#[tokio::test]
async fn devices_keep_syncing_during_a_forced_change() {
    let db = migrated_db().await;
    let (_, api_key) = create_device(&db.pool, SEED_ADMIN_ID).await;
    assert_eq!(
        ensure_default_admin(&db.pool, &test_config()).await.unwrap(),
        AdminBootstrap::ForcedChange
    );

    let (status, _) = call(&app(&db.pool), "GET", "/api/v1/sync/status", Some(&api_key), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_forced_change_cannot_keep_the_same_password() {
    let db = migrated_db().await;
    let generated = match ensure_default_admin(&db.pool, &test_config()).await.unwrap() {
        AdminBootstrap::GeneratedPassword(p) => p,
        other => panic!("got {other:?}"),
    };

    let app = app(&db.pool);
    let (_, body) = admin_login(&app, SEED_EMAIL, &generated).await;
    let session = body["sessionId"].as_str().unwrap().to_string();

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/admin/change-password",
        Some(&session),
        Some(json!({ "current_password": generated, "new_password": generated })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(must_change(&db.pool).await, 1);

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/admin/change-password",
        Some(&session),
        Some(json!({ "current_password": generated, "new_password": seed_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "the published password must be refused");
    assert_eq!(must_change(&db.pool).await, 1);
}

#[tokio::test]
async fn the_portal_can_clear_a_forced_change() {
    let db = migrated_db().await;
    make_existing_install(&db.pool).await;
    ensure_default_admin(&db.pool, &test_config()).await.unwrap();

    let app = app(&db.pool);
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": SEED_EMAIL, "password": seed_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mustChangePassword"], true);
    let session = body["sessionId"].as_str().unwrap().to_string();

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/user/change-password",
        Some(&session),
        Some(json!({ "currentPassword": seed_password(), "newPassword": random_password() })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(must_change(&db.pool).await, 0);
}

#[tokio::test]
async fn configured_credentials_rescue_an_existing_install() {
    let db = migrated_db().await;
    make_existing_install(&db.pool).await;
    let password = random_password();
    let mut config = test_config();
    config.default_admin_password = Some(password.clone());

    let outcome = ensure_default_admin(&db.pool, &config).await.unwrap();
    assert_eq!(outcome, AdminBootstrap::AppliedConfigured);
    assert_eq!(must_change(&db.pool).await, 0);

    let app = app(&db.pool);
    assert_eq!(admin_login(&app, SEED_EMAIL, &password).await.0, StatusCode::OK);
    assert_eq!(admin_login(&app, SEED_EMAIL, &seed_password()).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_admin_who_already_changed_their_password_is_left_alone() {
    let db = migrated_db().await;
    let password = random_password();
    let hash = jottery_server::utils::password::hash_password_with_params(&password, 1024, 1, 1).unwrap();
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(&hash)
        .bind(SEED_ADMIN_ID)
        .execute(&db.pool)
        .await
        .unwrap();

    let mut config = test_config();
    config.default_admin_email = "someone-else@example.com".to_string();
    config.default_admin_password = Some(random_password());

    let outcome = ensure_default_admin(&db.pool, &config).await.unwrap();
    assert_eq!(outcome, AdminBootstrap::AlreadyConfigured);
    assert_eq!(must_change(&db.pool).await, 0);
    assert_eq!(admin_login(&app(&db.pool), SEED_EMAIL, &password).await.0, StatusCode::OK);
}

#[tokio::test]
async fn a_configured_email_already_in_use_does_not_take_over_that_account() {
    let db = migrated_db().await;
    create_user(&db.pool, "taken@example.com", &random_password()).await;
    make_existing_install(&db.pool).await;

    let password = random_password();
    let mut config = test_config();
    config.default_admin_email = "taken@example.com".to_string();
    config.default_admin_password = Some(password.clone());

    assert_eq!(
        ensure_default_admin(&db.pool, &config).await.unwrap(),
        AdminBootstrap::AppliedConfigured
    );
    // The seeded account keeps its address, and takes the password.
    assert_eq!(admin_login(&app(&db.pool), SEED_EMAIL, &password).await.0, StatusCode::OK);
}
