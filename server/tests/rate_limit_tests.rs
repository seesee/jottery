// Throttling on the real router. Addresses are per-test, and each test builds
// its own router, so limits don't carry between tests.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::config::Config;
use serde_json::{json, Value};

const PEER: &str = "203.0.113.7";
const OTHER_PEER: &str = "203.0.113.8";
const PROXY: &str = "127.0.0.1";

fn config_with(auth_per_minute: u32, hops: usize) -> Config {
    let mut config = test_config();
    config.rate_limit.auth_per_minute = auth_per_minute;
    config.rate_limit.trusted_proxy_hops = hops;
    config
}

fn login_body(email: &str, password: &str) -> Value {
    json!({ "email": email, "password": password })
}

async fn login_from(app: &axum::Router, peer: &str, email: &str, headers: &[(&str, &str)]) -> StatusCode {
    call_from(
        app,
        peer,
        "POST",
        "/api/v1/user/login",
        Some(login_body(email, &random_password())),
        headers,
    )
    .await
    .0
}

/// A distinct email each time, so only the per-address limit can trigger.
async fn spray(app: &axum::Router, peer: &str, attempts: usize, headers: &[(&str, &str)]) -> Vec<StatusCode> {
    let mut statuses = Vec::new();
    for _ in 0..attempts {
        statuses.push(login_from(app, peer, &format!("{}@example.com", new_id()), headers).await);
    }
    statuses
}

// --- per email ---

#[tokio::test]
async fn repeated_failures_lock_an_account_even_for_the_right_password() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (email, password) = (format!("{}@example.com", new_id()), random_password());
    create_user(&db.pool, &email, &password).await;

    for _ in 0..5 {
        let (status, _) = call(&app, "POST", "/api/v1/user/login", None, Some(login_body(&email, &random_password()))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    let (status, headers, body) = call_full(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(login_body(&email, &password)),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "rate_limited");
    assert!(headers.contains_key("retry-after"));
}

#[tokio::test]
async fn every_password_route_shares_one_count() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (email, password) = (format!("{}@example.com", new_id()), random_password());
    create_user(&db.pool, &email, &password).await;

    let wrong = |uri: &'static str| {
        let body = json!({
            "email": email,
            "password": random_password(),
            "deviceName": "device",
            "deviceType": "web",
        });
        (uri, body)
    };

    // Spread across all three routes, a guesser still runs out
    for (uri, body) in [
        wrong("/api/v1/auth/login"),
        wrong("/api/v1/user/login"),
        wrong("/api/v1/auth/register-device"),
        wrong("/api/v1/auth/login"),
        wrong("/api/v1/auth/register-device"),
    ] {
        let (status, _) = call(&app, "POST", uri, None, Some(body)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }

    for uri in ["/api/v1/auth/login", "/api/v1/user/login", "/api/v1/auth/register-device"] {
        let (status, _) = call(
            &app,
            "POST",
            uri,
            None,
            Some(json!({
                "email": email,
                "password": password,
                "deviceName": "device",
                "deviceType": "web",
            })),
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{uri}");
    }
}

#[tokio::test]
async fn an_unknown_address_behaves_like_a_known_one() {
    // Otherwise locking would reveal which accounts exist
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());

    for _ in 0..5 {
        let (status, _) = call(&app, "POST", "/api/v1/user/login", None, Some(login_body(&email, &random_password()))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = call(&app, "POST", "/api/v1/user/login", None, Some(login_body(&email, &random_password()))).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn a_successful_login_clears_the_count() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (email, password) = (format!("{}@example.com", new_id()), random_password());
    create_user(&db.pool, &email, &password).await;

    for _ in 0..2 {
        for _ in 0..4 {
            let (status, _) = call(&app, "POST", "/api/v1/user/login", None, Some(login_body(&email, &random_password()))).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        }
        let (status, _) = call(&app, "POST", "/api/v1/user/login", None, Some(login_body(&email, &password))).await;
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn a_locked_account_keeps_syncing() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let user = create_user(&db.pool, &email, &random_password()).await;
    let (_, api_key) = create_device(&db.pool, &user).await;

    for _ in 0..6 {
        call(&app, "POST", "/api/v1/user/login", None, Some(login_body(&email, &random_password()))).await;
    }

    let (status, _) = call(&app, "GET", "/api/v1/sync/status", Some(&api_key), None).await;
    assert_eq!(status, StatusCode::OK);
}

// --- per client address ---

#[tokio::test]
async fn one_address_runs_out_and_another_does_not() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(3, 0));

    let statuses = spray(&app, PEER, 4, &[]).await;
    assert_eq!(statuses[..3], [StatusCode::UNAUTHORIZED; 3]);
    assert_eq!(statuses[3], StatusCode::TOO_MANY_REQUESTS);

    assert_eq!(spray(&app, OTHER_PEER, 1, &[]).await, [StatusCode::UNAUTHORIZED]);
}

#[tokio::test]
async fn a_rate_limited_response_says_when_to_retry() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(1, 0));
    spray(&app, PEER, 1, &[]).await;

    let (status, headers, body) = call_from(
        &app,
        PEER,
        "POST",
        "/api/v1/user/login",
        Some(login_body("a@example.com", &random_password())),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "rate_limited");
    assert!(headers.contains_key("retry-after"));
}

#[tokio::test]
async fn a_forged_header_cannot_dodge_the_limit_by_default() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(2, 0));

    let mut statuses = Vec::new();
    for spoofed in ["1.1.1.1", "2.2.2.2", "3.3.3.3"] {
        statuses.extend(spray(&app, PEER, 1, &[("x-forwarded-for", spoofed)]).await);
    }
    assert_eq!(
        statuses,
        [StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED, StatusCode::TOO_MANY_REQUESTS]
    );
}

#[tokio::test]
async fn behind_a_trusted_proxy_each_client_gets_its_own_limit() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(2, 1));

    // Everything arrives from the proxy, as it does through the bundled Caddy
    let first = spray(&app, PROXY, 3, &[("x-forwarded-for", PEER)]).await;
    assert_eq!(first[2], StatusCode::TOO_MANY_REQUESTS);

    let other = spray(&app, PROXY, 1, &[("x-forwarded-for", OTHER_PEER)]).await;
    assert_eq!(other, [StatusCode::UNAUTHORIZED], "clients must not share the proxy's limit");
}

#[tokio::test]
async fn behind_a_trusted_proxy_a_forged_entry_is_ignored() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(2, 1));

    // The client controls everything left of the entry the proxy appended
    let mut statuses = Vec::new();
    for forged in ["1.1.1.1", "2.2.2.2", "3.3.3.3"] {
        let chain = format!("{forged}, {PEER}");
        statuses.extend(spray(&app, PROXY, 1, &[("x-forwarded-for", &chain)]).await);
    }
    assert_eq!(statuses[2], StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn registration_has_its_own_tighter_limit() {
    let db = migrated_db().await;
    let mut config = test_config();
    config.rate_limit.register_per_hour = 2;
    let app = app_with(&db.pool, config);

    let mut statuses = Vec::new();
    for _ in 0..3 {
        let body = login_body(&format!("{}@example.com", new_id()), &random_password());
        let (status, _, _) = call_from(&app, PEER, "POST", "/api/v1/auth/register-user", Some(body), &[]).await;
        statuses.push(status);
    }
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::CREATED, StatusCode::TOO_MANY_REQUESTS]);
}

#[tokio::test]
async fn the_status_check_is_limited_too() {
    let db = migrated_db().await;
    let mut config = test_config();
    config.rate_limit.status_per_minute = 2;
    let app = app_with(&db.pool, config);

    let mut statuses = Vec::new();
    for _ in 0..3 {
        let uri = format!("/api/v1/user/status?email={}@example.com", new_id());
        statuses.push(call_from(&app, PEER, "GET", &uri, None, &[]).await.0);
    }
    assert_eq!(statuses, [StatusCode::OK, StatusCode::OK, StatusCode::TOO_MANY_REQUESTS]);
}

#[tokio::test]
async fn device_cloning_and_registration_are_covered() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(2, 0));

    for uri in ["/api/v1/auth/clone-device", "/api/v1/auth/register-device"] {
        let body = json!({
            "apiKey": new_id(),
            "email": format!("{}@example.com", new_id()),
            "password": random_password(),
            "deviceName": "device",
            "deviceType": "web",
        });
        call_from(&app, PEER, "POST", uri, Some(body), &[]).await;
    }

    let (status, _, _) = call_from(
        &app,
        PEER,
        "POST",
        "/api/v1/auth/clone-device",
        Some(json!({ "apiKey": new_id(), "deviceName": "device", "deviceType": "web" })),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn sync_is_not_rate_limited() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(1, 0));
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, api_key) = create_device(&db.pool, &user).await;

    for _ in 0..20 {
        let (status, _, _) = call_from(
            &app,
            PEER,
            "GET",
            "/api/v1/sync/status",
            None,
            &[("authorization", &format!("Bearer {api_key}"))],
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
}

#[tokio::test]
async fn rate_limiting_can_be_turned_off() {
    let db = migrated_db().await;
    let mut config = config_with(1, 0);
    config.rate_limit.enabled = false;
    let app = app_with(&db.pool, config);

    assert_eq!(spray(&app, PEER, 5, &[]).await, [StatusCode::UNAUTHORIZED; 5]);
}

#[tokio::test]
async fn the_audit_trail_records_the_client_not_what_it_claimed() {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config_with(10, 1));
    let (email, password) = (format!("{}@example.com", new_id()), random_password());
    let user = create_user(&db.pool, &email, &password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&user)
        .execute(&db.pool)
        .await
        .unwrap();

    let chain = format!("6.6.6.6, {PEER}");
    let (status, _, _) = call_from(
        &app,
        PROXY,
        "POST",
        "/api/v1/auth/login",
        Some(login_body(&email, &password)),
        &[("x-forwarded-for", &chain)],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let recorded: Option<String> = sqlx::query_scalar("SELECT ip_address FROM sessions WHERE user_id = ?")
        .bind(&user)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(recorded.as_deref(), Some(PEER));
}
