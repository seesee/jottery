// Cross-origin access, through the router `main` serves.

mod common;

use axum::http::{header, HeaderMap, StatusCode};
use common::*;
use jottery_server::config::Config;

const ROUTE: &str = "/api/v1/sync/status";

async fn preflight(allowed: Option<&[&str]>, origin: &str) -> HeaderMap {
    let db = migrated_db().await;
    let config = Config {
        cors_allowed_origins: allowed.map(|origins| origins.iter().map(|o| o.to_string()).collect()),
        ..test_config()
    };
    let app = app_with(&db.pool, config);

    let (_, headers, _) = call_full(
        &app,
        "OPTIONS",
        ROUTE,
        None,
        None,
        &[
            ("Origin", origin),
            ("Access-Control-Request-Method", "GET"),
            ("Access-Control-Request-Headers", "authorization, content-type"),
        ],
    )
    .await;
    headers
}

fn allowed_origin(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .map(|value| value.to_str().unwrap().to_string())
}

#[tokio::test]
async fn by_default_no_other_origin_is_allowed() {
    let headers = preflight(None, "https://evil.example.com").await;

    assert_eq!(allowed_origin(&headers), None);
}

#[tokio::test]
async fn a_listed_origin_is_allowed() {
    let headers = preflight(Some(&["https://notes.example.com"]), "https://notes.example.com").await;

    assert_eq!(allowed_origin(&headers).as_deref(), Some("https://notes.example.com"));
}

#[tokio::test]
async fn each_listed_origin_is_allowed() {
    let origins = ["https://a.example.com", "https://b.example.com:8443", "http://localhost:5173"];

    for origin in origins {
        let headers = preflight(Some(&origins), origin).await;
        assert_eq!(allowed_origin(&headers).as_deref(), Some(origin));
    }
}

#[tokio::test]
async fn origins_that_only_look_alike_are_refused() {
    let allowed: &[&str] = &["https://notes.example.com"];

    for origin in [
        "https://evil.example.com",
        "http://notes.example.com",
        "https://notes.example.com.evil.com",
        "https://sub.notes.example.com",
        "https://notes.example.com:8443",
        "null",
    ] {
        let headers = preflight(Some(allowed), origin).await;
        assert_eq!(allowed_origin(&headers), None, "{origin}");
    }
}

#[tokio::test]
async fn an_explicit_wildcard_allows_anyone() {
    let headers = preflight(Some(&["*"]), "https://anywhere.example.com").await;

    assert_eq!(allowed_origin(&headers).as_deref(), Some("*"));
}

#[tokio::test]
async fn an_origin_that_cannot_be_a_header_is_skipped() {
    let allowed: &[&str] = &["not a\nheader", "https://notes.example.com"];

    let headers = preflight(Some(allowed), "https://notes.example.com").await;

    assert_eq!(allowed_origin(&headers).as_deref(), Some("https://notes.example.com"));
}

#[tokio::test]
async fn the_headers_clients_send_are_allowed() {
    let headers = preflight(Some(&["https://notes.example.com"]), "https://notes.example.com").await;

    let allowed = headers[header::ACCESS_CONTROL_ALLOW_HEADERS].to_str().unwrap().to_lowercase();
    for name in ["authorization", "content-type", "x-api-key"] {
        assert!(allowed.contains(name), "{name} in {allowed}");
    }
}

#[tokio::test]
async fn credentials_are_never_allowed() {
    for allowed in [&["https://notes.example.com"][..], &["*"][..]] {
        let headers = preflight(Some(allowed), "https://notes.example.com").await;
        assert!(headers.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());
    }
}

#[tokio::test]
async fn a_real_request_carries_the_header_too() {
    let db = migrated_db().await;
    let config = Config {
        cors_allowed_origins: Some(vec!["https://notes.example.com".to_string()]),
        ..test_config()
    };
    let app = app_with(&db.pool, config);
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let (_, key) = create_device(&db.pool, &user).await;

    let (status, headers, _) = call_full(
        &app,
        "GET",
        ROUTE,
        Some(&key),
        None,
        &[("Origin", "https://notes.example.com")],
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(allowed_origin(&headers).as_deref(), Some("https://notes.example.com"));
}
