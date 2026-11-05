// Reading the configuration, through the code `main` uses.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::config::Config;
use serde_json::json;

fn config(settings: &[(&str, &str)]) -> Config {
    Config::from_lookup(|name| {
        settings
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.to_string())
    })
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let config = config(&[]);

    assert_eq!(config.database_url, "sqlite:jottery.db");
    assert_eq!(config.port, 3030);
    assert_eq!(config.max_payload_size, 104_857_600);
    assert_eq!(config.session_expiry_days, 1);
    assert_eq!(config.default_admin_email, "admin@localhost");
    assert_eq!(config.default_admin_password, None);
    assert_eq!(config.password_complexity, "standard");
    assert_eq!(config.default_storage_quota_mb, 1000);
    assert!(!config.enable_hsts);
    assert!(!config.enforce_storage_quota);
    assert!(config.rate_limit.enabled);
    assert_eq!(config.rate_limit.trusted_proxy_hops, 0);
    assert_eq!(config.cors_allowed_origins, None);
}

#[test]
fn argon2_defaults_match_the_owasp_minimum() {
    let config = config(&[]);

    assert_eq!(config.argon2_m_cost, 19456);
    assert_eq!(config.argon2_t_cost, 2);
    assert_eq!(config.argon2_p_cost, 1);
}

#[test]
fn settings_override_the_defaults() {
    let config = config(&[
        ("PORT", "8080"),
        ("SESSION_EXPIRY_DAYS", "14"),
        ("ARGON2_M_COST", "65536"),
        ("DEFAULT_STORAGE_QUOTA_MB", "50"),
        ("TRUSTED_PROXY_HOPS", "2"),
        ("RATE_LIMIT_ENABLED", "false"),
        ("ENFORCE_STORAGE_QUOTA", "true"),
    ]);

    assert_eq!(config.port, 8080);
    assert_eq!(config.session_expiry_days, 14);
    assert_eq!(config.argon2_m_cost, 65536);
    assert_eq!(config.default_storage_quota_mb, 50);
    assert_eq!(config.rate_limit.trusted_proxy_hops, 2);
    assert!(!config.rate_limit.enabled);
    assert!(config.enforce_storage_quota);
}

#[test]
fn a_value_that_does_not_parse_falls_back_to_the_default() {
    let config = config(&[
        ("PORT", "not-a-port"),
        ("SESSION_EXPIRY_DAYS", "soon"),
        ("TRUSTED_PROXY_HOPS", "-1"),
        ("ENFORCE_STORAGE_QUOTA", "yes please"),
    ]);

    assert_eq!(config.port, 3030);
    assert_eq!(config.session_expiry_days, 1);
    assert_eq!(config.rate_limit.trusted_proxy_hops, 0);
    assert!(!config.enforce_storage_quota);
}

#[test]
fn hsts_is_only_on_when_clearly_asked_for() {
    assert!(config(&[("ENABLE_HSTS", "true")]).enable_hsts);
    assert!(config(&[("ENABLE_HSTS", "TRUE")]).enable_hsts);
    assert!(!config(&[("ENABLE_HSTS", "1")]).enable_hsts);
    assert!(!config(&[("ENABLE_HSTS", "false")]).enable_hsts);
}

#[test]
fn password_complexity_ignores_case() {
    assert_eq!(config(&[("PASSWORD_COMPLEXITY", "STRONG")]).password_complexity, "strong");
}

#[test]
fn an_empty_admin_password_counts_as_unset() {
    assert_eq!(config(&[("DEFAULT_ADMIN_PASSWORD", "")]).default_admin_password, None);
}

#[test]
fn cors_origins_are_split_trimmed_and_tidied() {
    let config = config(&[(
        "CORS_ALLOWED_ORIGINS",
        " https://a.example.com ,,https://b.example.com, ",
    )]);

    assert_eq!(
        config.cors_allowed_origins,
        Some(vec![
            "https://a.example.com".to_string(),
            "https://b.example.com".to_string()
        ])
    );
}

/// The expiry a real login reports, in whole days from now.
async fn login_expiry_days(session_expiry_days: i64) -> i64 {
    let db = migrated_db().await;
    let config = Config { session_expiry_days, ..test_config() };
    let app = app_with(&db.pool, config);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    create_user(&db.pool, &email, &password).await;

    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let expires_at = chrono::DateTime::parse_from_rfc3339(body["expiresAt"].as_str().unwrap()).unwrap();
    // Rounded up, because a little of the first day has passed already
    (expires_at.with_timezone(&chrono::Utc) - chrono::Utc::now()).num_hours() / 24 + 1
}

#[tokio::test]
async fn a_session_lasts_as_long_as_configured() {
    for days in [1, 7, 14, 30] {
        assert_eq!(login_expiry_days(days).await, days);
    }
}
