// Every API route, through the router `main` serves, against every kind of
// credential that shouldn't open it. A route added without its middleware fails
// here, and a route added without an entry in this table fails the last test.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::json;
use std::collections::BTreeSet;

const ID: &str = "00000000-0000-0000-0000-000000000000";

const DEVICE_ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/v1/sync/status"),
    ("POST", "/api/v1/sync/push"),
    ("POST", "/api/v1/sync/pull"),
    ("GET", "/api/v1/sync/attachments/:id"),
    ("DELETE", "/api/v1/sync/notes/:id"),
    ("DELETE", "/api/v1/sync/device"),
    ("GET", "/api/v1/sync/events/token"),
    ("GET", "/api/v1/auth/wrapped-key"),
    ("PUT", "/api/v1/auth/wrapped-key"),
    ("GET", "/api/v1/inbox"),
    ("DELETE", "/api/v1/inbox"),
    ("DELETE", "/api/v1/inbox/:id"),
    ("GET", "/api/v1/inbox/status"),
];

const USER_ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/v1/user/account"),
    ("DELETE", "/api/v1/user/account"),
    ("DELETE", "/api/v1/user/notes"),
    ("POST", "/api/v1/user/change-password"),
    ("GET", "/api/v1/user/wrapped-key"),
    ("PUT", "/api/v1/user/wrapped-key"),
    ("POST", "/api/v1/user/logout"),
    ("GET", "/api/v1/user/devices"),
    ("DELETE", "/api/v1/user/devices/:id"),
    ("POST", "/api/v1/user/inbox-token"),
    ("DELETE", "/api/v1/user/inbox-token"),
    ("GET", "/api/v1/user/inbox-token/status"),
];

const ADMIN_ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/v1/admin/me"),
    ("POST", "/api/v1/admin/logout"),
    ("GET", "/api/v1/admin/users"),
    ("GET", "/api/v1/admin/users/:id"),
    ("DELETE", "/api/v1/admin/users/:id"),
    ("POST", "/api/v1/admin/users/:id/approve"),
    ("POST", "/api/v1/admin/users/:id/deactivate"),
    ("POST", "/api/v1/admin/users/:id/activate"),
    ("POST", "/api/v1/admin/users/:id/toggle-admin"),
    ("GET", "/api/v1/admin/users/:id/devices"),
    ("PATCH", "/api/v1/admin/users/:id/settings"),
    ("POST", "/api/v1/admin/users/:id/reset-password"),
    ("DELETE", "/api/v1/admin/devices/:id"),
    ("PATCH", "/api/v1/admin/devices/:id"),
    ("GET", "/api/v1/admin/stats"),
    ("GET", "/api/v1/admin/audit"),
    ("GET", "/api/v1/admin/notes/metadata"),
    ("POST", "/api/v1/admin/change-password"),
];

const INBOX_SUBMIT_ROUTES: &[(&str, &str)] = &[("POST", "/api/v1/inbox")];

/// Routes open to anyone, each checking its own credential if it takes one.
const OPEN_ROUTES: &[&str] = &[
    "/api/v1/auth/login",
    "/api/v1/user/login",
    "/api/v1/user/status",
    "/api/v1/auth/register-device",
    "/api/v1/auth/clone-device",
    "/api/v1/auth/register-user",
];

/// Takes a short-lived token in its URL, and has its own test below.
const STREAM_ROUTE: &str = "/api/v1/sync/events";

struct Credentials {
    db: TestDb,
    app: axum::Router,
    device_key: String,
    user_session: String,
    inbox_token: String,
}

async fn credentials() -> Credentials {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let user = create_user(&db.pool, &email, &password).await;
    let (_, device_key) = create_device(&db.pool, &user).await;

    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let user_session = login["sessionId"].as_str().unwrap().to_string();

    let (status, token) = call(&app, "POST", "/api/v1/user/inbox-token", Some(&user_session), None).await;
    assert_eq!(status, StatusCode::CREATED);
    let inbox_token = token["token"].as_str().unwrap().to_string();

    Credentials { db, app, device_key, user_session, inbox_token }
}

async fn status_of(app: &axum::Router, method: &str, path: &str, bearer: Option<&str>) -> StatusCode {
    let uri = path.replace(":id", ID);
    call(app, method, &uri, bearer, None).await.0
}

async fn assert_refused(
    app: &axum::Router,
    routes: &[(&str, &str)],
    bearer: Option<&str>,
    expected: StatusCode,
    credential: &str,
) {
    for (method, path) in routes {
        assert_eq!(
            status_of(app, method, path, bearer).await,
            expected,
            "{method} {path} with {credential}"
        );
    }
}

#[tokio::test]
async fn no_credentials_open_nothing() {
    let c = credentials().await;

    for routes in [DEVICE_ROUTES, USER_ROUTES, ADMIN_ROUTES, INBOX_SUBMIT_ROUTES] {
        assert_refused(&c.app, routes, None, StatusCode::UNAUTHORIZED, "nothing").await;
    }
}

#[tokio::test]
async fn a_made_up_credential_opens_nothing() {
    let c = credentials().await;
    let bearer = new_id();

    for routes in [DEVICE_ROUTES, USER_ROUTES, ADMIN_ROUTES, INBOX_SUBMIT_ROUTES] {
        assert_refused(&c.app, routes, Some(&bearer), StatusCode::UNAUTHORIZED, "a made-up token").await;
    }
}

#[tokio::test]
async fn a_device_key_opens_only_device_routes() {
    let c = credentials().await;
    let key = Some(c.device_key.as_str());

    for routes in [USER_ROUTES, ADMIN_ROUTES, INBOX_SUBMIT_ROUTES] {
        assert_refused(&c.app, routes, key, StatusCode::UNAUTHORIZED, "a device key").await;
    }
}

#[tokio::test]
async fn a_user_session_opens_no_device_or_admin_route() {
    let c = credentials().await;
    let session = Some(c.user_session.as_str());

    assert_refused(&c.app, DEVICE_ROUTES, session, StatusCode::UNAUTHORIZED, "a session").await;
    assert_refused(&c.app, INBOX_SUBMIT_ROUTES, session, StatusCode::UNAUTHORIZED, "a session").await;
    assert_refused(&c.app, ADMIN_ROUTES, session, StatusCode::FORBIDDEN, "a non-admin session").await;
}

#[tokio::test]
async fn an_inbox_token_opens_only_submission() {
    // It's handed to scripts and shortcuts, so it must reach nothing else
    let c = credentials().await;
    let token = Some(c.inbox_token.as_str());

    for routes in [DEVICE_ROUTES, USER_ROUTES, ADMIN_ROUTES] {
        assert_refused(&c.app, routes, token, StatusCode::UNAUTHORIZED, "an inbox token").await;
    }
}

#[tokio::test]
async fn the_event_stream_needs_its_token() {
    let c = credentials().await;

    let (status, _) = call(&c.app, "GET", &format!("{STREAM_ROUTE}?token={}", new_id()), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = call(
        &c.app,
        "GET",
        &format!("{STREAM_ROUTE}?token={}", c.device_key),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_deactivated_account_opens_nothing_with_a_valid_session() {
    let c = credentials().await;
    sqlx::query("UPDATE users SET is_active = 0 WHERE email != 'admin@localhost'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    let session = Some(c.user_session.as_str());

    assert_refused(&c.app, USER_ROUTES, session, StatusCode::FORBIDDEN, "a deactivated user").await;
    assert_refused(&c.app, DEVICE_ROUTES, Some(&c.device_key), StatusCode::FORBIDDEN, "a deactivated user's device").await;
}

#[test]
fn every_route_in_the_router_is_in_this_table() {
    let source = include_str!("../src/app.rs");

    let mut in_router: BTreeSet<&str> = BTreeSet::new();
    for piece in source.split('"').skip(1).step_by(2) {
        if piece.starts_with("/api/") {
            in_router.insert(piece);
        }
    }
    assert!(in_router.len() > 20, "found too few routes: {in_router:?}");

    let mut in_table: BTreeSet<&str> = OPEN_ROUTES.iter().copied().collect();
    in_table.insert(STREAM_ROUTE);
    for routes in [DEVICE_ROUTES, USER_ROUTES, ADMIN_ROUTES, INBOX_SUBMIT_ROUTES] {
        in_table.extend(routes.iter().map(|(_, path)| *path));
    }

    let missing: Vec<_> = in_router.difference(&in_table).collect();
    assert!(missing.is_empty(), "add these to the table in this file: {missing:?}");
}
