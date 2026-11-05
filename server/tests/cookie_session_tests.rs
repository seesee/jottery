// HttpOnly session cookies for the dashboards. Script in the notes app shares an
// origin with the dashboards, so the token must be out of its reach, and moving it
// into a cookie must not make cross-site requests possible.

mod common;

use axum::http::{header, HeaderMap, StatusCode};
use common::*;
use jottery_server::config::Config;
use serde_json::{json, Value};

const CSRF_HEADER: (&str, &str) = ("X-Requested-With", "JotteryDashboard");

const ADMIN_LOGIN: &str = "/api/v1/auth/login";
const USER_LOGIN: &str = "/api/v1/user/login";

fn https_config() -> Config {
    Config {
        enable_hsts: true,
        ..test_config()
    }
}

struct Fixture {
    db: TestDb,
    app: axum::Router,
    user_id: String,
    email: String,
    password: String,
}

/// An admin account on a server deployed behind HTTPS.
async fn admin_fixture() -> Fixture {
    fixture(https_config(), true).await
}

async fn fixture(config: Config, is_admin: bool) -> Fixture {
    let db = migrated_db().await;
    let app = app_with(&db.pool, config);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let user_id = create_user(&db.pool, &email, &password).await;
    if is_admin {
        sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
            .bind(&user_id)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    Fixture { db, app, user_id, email, password }
}

impl Fixture {
    async fn login(&self, uri: &str, extra: Value) -> (StatusCode, HeaderMap, Value) {
        let mut body = json!({ "email": self.email, "password": self.password });
        for (key, value) in extra.as_object().unwrap() {
            body[key] = value.clone();
        }
        call_full(&self.app, "POST", uri, None, Some(body), &[]).await
    }

    /// Sign in for a cookie session, returning what a browser would send back.
    async fn cookie_login(&self, uri: &str) -> String {
        let (status, headers, body) = self.login(uri, json!({ "cookieSession": true })).await;
        assert_eq!(status, StatusCode::OK, "login failed: {body}");
        cookie_pair(&headers)
    }

    async fn force_password_change(&self) {
        sqlx::query("UPDATE users SET must_change_password = 1 WHERE id = ?")
            .bind(&self.user_id)
            .execute(&self.db.pool)
            .await
            .unwrap();
    }

    async fn session_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE user_id = ?")
            .bind(&self.user_id)
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
    }
}

fn set_cookie(headers: &HeaderMap) -> String {
    headers
        .get(header::SET_COOKIE)
        .expect("response sets a cookie")
        .to_str()
        .unwrap()
        .to_string()
}

/// The `name=value` part of a Set-Cookie header.
fn cookie_pair(headers: &HeaderMap) -> String {
    set_cookie(headers).split(';').next().unwrap().to_string()
}

fn attributes(set_cookie: &str) -> Vec<String> {
    set_cookie.split(';').skip(1).map(|a| a.trim().to_string()).collect()
}

async fn with_cookie(
    fixture: &Fixture,
    method: &str,
    uri: &str,
    cookie: &str,
    extra: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut headers = vec![("Cookie", cookie)];
    headers.extend_from_slice(extra);
    call_full(&fixture.app, method, uri, None, body, &headers).await
}

#[tokio::test]
async fn admin_login_sets_the_cookie_the_dashboard_was_promised() {
    let fixture = admin_fixture().await;

    let (status, headers, body) = fixture.login(ADMIN_LOGIN, json!({ "cookieSession": true })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cookieSession"], json!(true));
    assert!(body.get("sessionId").is_none(), "the token must stay out of script's reach: {body}");
    assert_eq!(body["user"]["email"], json!(fixture.email));

    let cookie = set_cookie(&headers);
    assert!(cookie.starts_with("session_token="), "{cookie}");
    let attributes = attributes(&cookie);
    for expected in [
        "HttpOnly".to_string(),
        "Secure".to_string(),
        "SameSite=Strict".to_string(),
        format!("Max-Age={}", 7 * 24 * 60 * 60),
        "Path=/api/v1/admin".to_string(),
    ] {
        assert!(attributes.contains(&expected), "missing {expected} in {cookie}");
    }
}

#[tokio::test]
async fn portal_login_scopes_its_cookie_to_the_user_api() {
    let fixture = fixture(https_config(), false).await;

    let (status, headers, body) = fixture.login(USER_LOGIN, json!({ "cookieSession": true })).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cookieSession"], json!(true));
    assert!(body.get("sessionId").is_none());
    let cookie = set_cookie(&headers);
    assert!(attributes(&cookie).contains(&"Path=/api/v1/user".to_string()), "{cookie}");
    assert!(attributes(&cookie).contains(&"HttpOnly".to_string()), "{cookie}");
}

#[tokio::test]
async fn the_cookie_reflects_the_configured_session_expiry() {
    let config = Config { session_expiry_days: 2, ..https_config() };
    let fixture = fixture(config, true).await;

    let (_, headers, _) = fixture.login(ADMIN_LOGIN, json!({ "cookieSession": true })).await;

    let expected = format!("Max-Age={}", 2 * 24 * 60 * 60);
    assert!(attributes(&set_cookie(&headers)).contains(&expected));
}

#[tokio::test]
async fn a_client_that_does_not_ask_keeps_its_token() {
    let fixture = admin_fixture().await;

    for uri in [ADMIN_LOGIN, USER_LOGIN] {
        let (status, headers, body) = fixture.login(uri, json!({})).await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["sessionId"].is_string(), "{body}");
        assert!(body.get("cookieSession").is_none(), "{body}");
        assert!(headers.get(header::SET_COOKIE).is_none());
    }
}

#[tokio::test]
async fn plain_http_installs_ignore_the_request() {
    // A Secure cookie would never come back over plain HTTP, leaving a dashboard
    // that can't stay signed in. ENABLE_HSTS is how an install says it's on HTTPS.
    let fixture = fixture(test_config(), true).await;

    for uri in [ADMIN_LOGIN, USER_LOGIN] {
        let (status, headers, body) = fixture.login(uri, json!({ "cookieSession": true })).await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["sessionId"].is_string(), "{body}");
        assert!(body.get("cookieSession").is_none(), "{body}");
        assert!(headers.get(header::SET_COOKIE).is_none());
    }
}

#[tokio::test]
async fn a_failed_login_sets_no_cookie() {
    let fixture = admin_fixture().await;

    let (status, headers, _) = call_full(
        &fixture.app,
        "POST",
        ADMIN_LOGIN,
        None,
        Some(json!({ "email": fixture.email, "password": random_password(), "cookieSession": true })),
        &[],
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(headers.get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn the_cookie_authenticates_reads_without_the_header() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;

    let (status, _, body) =
        with_cookie(&fixture, "GET", "/api/v1/admin/users", &cookie, &[], None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn a_cross_site_write_is_refused() {
    // SameSite=Strict should stop the cookie arriving at all. This is what stands
    // behind it if a browser or a same-site host lets one through.
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;
    let target = create_user(&fixture.db.pool, &format!("{}@example.com", new_id()), &random_password()).await;

    for (method, uri) in [
        ("POST", format!("/api/v1/admin/users/{target}/deactivate")),
        ("DELETE", format!("/api/v1/admin/users/{target}")),
        ("PATCH", format!("/api/v1/admin/users/{target}/settings")),
        ("POST", "/api/v1/admin/logout".to_string()),
    ] {
        let (status, _, _) =
            with_cookie(&fixture, method, &uri, &cookie, &[], Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
    }

    let active: i64 = sqlx::query_scalar("SELECT is_active FROM users WHERE id = ?")
        .bind(&target)
        .fetch_one(&fixture.db.pool)
        .await
        .unwrap();
    assert_eq!(active, 1);
    assert_eq!(fixture.session_count().await, 1, "logout must not have run");
}

#[tokio::test]
async fn the_header_must_carry_the_agreed_value() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;
    let target = create_user(&fixture.db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let uri = format!("/api/v1/admin/users/{target}/deactivate");

    for value in ["XMLHttpRequest", "", "jotterydashboard"] {
        let (status, _, _) =
            with_cookie(&fixture, "POST", &uri, &cookie, &[("X-Requested-With", value)], None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "value {value:?}");
    }

    let (status, _, body) = with_cookie(&fixture, "POST", &uri, &cookie, &[CSRF_HEADER], None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
}

#[tokio::test]
async fn the_portal_applies_the_same_rule() {
    let fixture = fixture(https_config(), false).await;
    let cookie = fixture.cookie_login(USER_LOGIN).await;

    let (status, _, _) =
        with_cookie(&fixture, "POST", "/api/v1/user/inbox-token", &cookie, &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _, _) =
        with_cookie(&fixture, "DELETE", "/api/v1/user/notes", &cookie, &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _, body) =
        with_cookie(&fixture, "GET", "/api/v1/user/account", &cookie, &[], None).await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, _, _) =
        with_cookie(&fixture, "POST", "/api/v1/user/inbox-token", &cookie, &[CSRF_HEADER], None).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn a_bearer_token_needs_no_header() {
    // A browser never adds Authorization by itself, so a token can't be forged
    // cross-site, and released clients don't send the header.
    let fixture = fixture(https_config(), false).await;
    let (_, _, body) = fixture.login(USER_LOGIN, json!({})).await;
    let token = body["sessionId"].as_str().unwrap();

    let (status, _) = call(&fixture.app, "POST", "/api/v1/user/inbox-token", Some(token), None).await;

    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn an_unusable_authorization_header_does_not_excuse_a_cookie() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;

    let (status, _, _) = with_cookie(
        &fixture,
        "POST",
        "/api/v1/admin/logout",
        &cookie,
        &[("Authorization", "Basic abc")],
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_form_cannot_sign_a_browser_in() {
    let fixture = admin_fixture().await;
    let body = format!("email={}&password={}&cookieSession=true", fixture.email, fixture.password);
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(ADMIN_LOGIN)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(axum::body::Body::from(body))
        .unwrap();

    let response = tower::ServiceExt::oneshot(fixture.app.clone(), request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn cross_origin_scripts_cannot_send_the_cookie() {
    // The header has to be allowed for preflight, so it's the absence of
    // Allow-Credentials that keeps a credentialed cross-origin request out.
    for allowed in ["https://notes.example.com", "*"] {
        let config = Config {
            cors_allowed_origins: Some(vec![allowed.to_string()]),
            ..https_config()
        };
        let fixture = fixture(config, true).await;

        let (_, headers, _) = call_full(
            &fixture.app,
            "OPTIONS",
            "/api/v1/admin/users",
            None,
            None,
            &[
                ("Origin", "https://notes.example.com"),
                ("Access-Control-Request-Method", "POST"),
                ("Access-Control-Request-Headers", "x-requested-with"),
            ],
        )
        .await;

        let allowed_headers = headers
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .expect("preflight lists headers")
            .to_str()
            .unwrap()
            .to_lowercase();
        assert!(allowed_headers.contains("x-requested-with"), "{allowed_headers}");
        assert!(
            headers.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none(),
            "credentialed cross-origin requests must stay impossible"
        );
    }
}

#[tokio::test]
async fn admin_me_says_who_is_signed_in() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;

    let (status, _, body) =
        with_cookie(&fixture, "GET", "/api/v1/admin/me", &cookie, &[], None).await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["email"], json!(fixture.email));
    assert_eq!(body["id"], json!(fixture.user_id));
    assert_eq!(body["isAdmin"], json!(true));
}

#[tokio::test]
async fn admin_me_is_for_admins() {
    let fixture = fixture(https_config(), false).await;
    let (_, _, body) = fixture.login(USER_LOGIN, json!({})).await;
    let token = body["sessionId"].as_str().unwrap();

    let (status, _) = call(&fixture.app, "GET", "/api/v1/admin/me", Some(token), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(&fixture.app, "GET", "/api/v1/admin/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_me_waits_for_a_forced_password_change() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;
    fixture.force_password_change().await;

    let (status, _, body) =
        with_cookie(&fixture, "GET", "/api/v1/admin/me", &cookie, &[], None).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], json!("password_change_required"));
}

#[tokio::test]
async fn admin_logout_ends_the_session_and_clears_the_cookie() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;

    let (status, headers, _) =
        with_cookie(&fixture, "POST", "/api/v1/admin/logout", &cookie, &[CSRF_HEADER], None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let cleared = set_cookie(&headers);
    assert!(cleared.starts_with("session_token=;"), "{cleared}");
    let attributes = attributes(&cleared);
    assert!(attributes.contains(&"Max-Age=0".to_string()), "{cleared}");
    assert!(attributes.contains(&"Path=/api/v1/admin".to_string()), "{cleared}");
    assert_eq!(fixture.session_count().await, 0);

    let (status, _, _) =
        with_cookie(&fixture, "GET", "/api/v1/admin/users", &cookie, &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_logout_works_for_a_token_too() {
    let fixture = admin_fixture().await;
    let (_, _, body) = fixture.login(ADMIN_LOGIN, json!({})).await;
    let token = body["sessionId"].as_str().unwrap();

    let (status, _) = call(&fixture.app, "POST", "/api/v1/admin/logout", Some(token), None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&fixture.app, "GET", "/api/v1/admin/users", Some(token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn signing_out_stays_possible_during_a_forced_password_change() {
    let fixture = admin_fixture().await;
    let cookie = fixture.cookie_login(ADMIN_LOGIN).await;
    fixture.force_password_change().await;

    let (status, _, _) =
        with_cookie(&fixture, "POST", "/api/v1/admin/logout", &cookie, &[CSRF_HEADER], None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(fixture.session_count().await, 0);
}

#[tokio::test]
async fn portal_logout_clears_its_cookie() {
    let fixture = fixture(https_config(), false).await;
    let cookie = fixture.cookie_login(USER_LOGIN).await;

    let (status, headers, _) =
        with_cookie(&fixture, "POST", "/api/v1/user/logout", &cookie, &[CSRF_HEADER], None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let cleared = set_cookie(&headers);
    assert!(attributes(&cleared).contains(&"Max-Age=0".to_string()), "{cleared}");
    assert!(attributes(&cleared).contains(&"Path=/api/v1/user".to_string()), "{cleared}");
    assert_eq!(fixture.session_count().await, 0);
}
