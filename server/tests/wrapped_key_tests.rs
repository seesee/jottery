// The server keeps one wrapped master key per account, and overwriting it with the
// wrong one locks every new device out. A client that decided to write based on
// what it read must be able to say so, and lose if the key has moved on.

mod common;

use axum::http::{header, StatusCode};
use common::*;
use serde_json::{json, Value};

const DEVICE_ROUTE: &str = "/api/v1/auth/wrapped-key";
const SESSION_ROUTE: &str = "/api/v1/user/wrapped-key";

struct Fixture {
    db: TestDb,
    app: axum::Router,
    api_key: String,
    email: String,
    password: String,
}

async fn setup() -> Fixture {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let user = create_user(&db.pool, &email, &password).await;
    let (_, api_key) = create_device(&db.pool, &user).await;
    Fixture { db, app, api_key, email, password }
}

fn body(blob: &str) -> Value {
    json!({ "blob": blob, "kdfVersion": 1, "kdfIterations": 1_000_000 })
}

fn with_replace(blob: &str, replace: bool) -> Value {
    let mut value = body(blob);
    value["replace"] = json!(replace);
    value
}

impl Fixture {
    async fn put(&self, request: Value, if_match: Option<&str>) -> StatusCode {
        self.put_to(DEVICE_ROUTE, &self.api_key, request, if_match).await
    }

    async fn put_to(
        &self,
        route: &str,
        bearer: &str,
        request: Value,
        if_match: Option<&str>,
    ) -> StatusCode {
        let headers: Vec<(&str, &str)> = if_match.iter().map(|tag| ("If-Match", *tag)).collect();
        let (status, _, _) =
            call_full(&self.app, "PUT", route, Some(bearer), Some(request), &headers).await;
        status
    }

    async fn get(&self) -> Value {
        let (status, value) = call(&self.app, "GET", DEVICE_ROUTE, Some(&self.api_key), None).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        value
    }

    async fn stored_blob(&self) -> Option<String> {
        sqlx::query_scalar("SELECT wrapped_blob FROM wrapped_keys")
            .fetch_optional(&self.db.pool)
            .await
            .unwrap()
    }

    async fn updated_at(&self) -> String {
        self.get().await["updatedAt"].as_str().expect("updatedAt is a string").to_string()
    }

    /// Store a first key, returning its `updatedAt`.
    async fn seed(&self, blob: &str) -> String {
        assert_eq!(self.put(body(blob), None).await, StatusCode::OK);
        self.updated_at().await
    }
}

#[tokio::test]
async fn reading_the_key_says_when_it_last_changed() {
    let fixture = setup().await;
    fixture.seed("first").await;

    let key = fixture.get().await;

    assert_eq!(key["blob"], json!("first"));
    assert_eq!(key["kdfVersion"], json!(1));
    assert_eq!(key["kdfIterations"], json!(1_000_000));
    let updated_at = key["updatedAt"].as_str().unwrap();
    assert!(chrono::DateTime::parse_from_rfc3339(updated_at).is_ok(), "{updated_at}");
}

#[tokio::test]
async fn released_clients_can_still_replace_the_key() {
    // They send no condition, and a password change has to reach the server: a
    // refusal here would leave a blob only the old password opens.
    let fixture = setup().await;
    fixture.seed("first").await;

    assert_eq!(fixture.put(body("second"), None).await, StatusCode::OK);

    assert_eq!(fixture.stored_blob().await.as_deref(), Some("second"));
}

#[tokio::test]
async fn a_write_based_on_the_current_key_succeeds() {
    let fixture = setup().await;
    let updated_at = fixture.seed("first").await;

    assert_eq!(fixture.put(body("second"), Some(&updated_at)).await, StatusCode::OK);

    assert_eq!(fixture.stored_blob().await.as_deref(), Some("second"));
    assert_ne!(fixture.updated_at().await, updated_at);
}

#[tokio::test]
async fn a_quoted_tag_is_understood() {
    let fixture = setup().await;
    let updated_at = fixture.seed("first").await;

    let tag = format!("\"{updated_at}\"");
    assert_eq!(fixture.put(body("second"), Some(&tag)).await, StatusCode::OK);
}

#[tokio::test]
async fn a_write_based_on_an_old_key_is_refused() {
    // Device B read the blob, device A then changed the password, and B's repair
    // must not put back a key wrapped with the old password.
    let fixture = setup().await;
    let read_by_b = fixture.seed("first").await;
    assert_eq!(fixture.put(with_replace("from-a", true), None).await, StatusCode::OK);

    assert_eq!(
        fixture.put(body("from-b"), Some(&read_by_b)).await,
        StatusCode::CONFLICT
    );

    assert_eq!(fixture.stored_blob().await.as_deref(), Some("from-a"));
}

#[tokio::test]
async fn only_one_of_two_racing_writes_wins() {
    let fixture = setup().await;
    let read_by_both = fixture.seed("first").await;

    let (first, second) = tokio::join!(
        fixture.put(body("one"), Some(&read_by_both)),
        fixture.put(body("two"), Some(&read_by_both)),
    );

    let mut outcomes = [first, second];
    outcomes.sort();
    assert_eq!(outcomes, [StatusCode::OK, StatusCode::CONFLICT]);
}

#[tokio::test]
async fn a_condition_on_a_key_that_is_not_there_is_refused() {
    let fixture = setup().await;

    let status = fixture
        .put(body("first"), Some("2026-01-01T00:00:00+00:00"))
        .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(fixture.stored_blob().await, None);
}

#[tokio::test]
async fn replace_overwrites_without_a_condition() {
    let fixture = setup().await;
    fixture.seed("first").await;

    assert_eq!(fixture.put(with_replace("second", true), None).await, StatusCode::OK);

    assert_eq!(fixture.stored_blob().await.as_deref(), Some("second"));
}

#[tokio::test]
async fn a_condition_outranks_replace() {
    let fixture = setup().await;
    fixture.seed("first").await;

    let status = fixture
        .put(with_replace("second", true), Some("2026-01-01T00:00:00+00:00"))
        .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(fixture.stored_blob().await.as_deref(), Some("first"));
}

#[tokio::test]
async fn replace_false_only_creates() {
    let fixture = setup().await;

    assert_eq!(fixture.put(with_replace("first", false), None).await, StatusCode::OK);
    assert_eq!(fixture.stored_blob().await.as_deref(), Some("first"));

    assert_eq!(
        fixture.put(with_replace("second", false), None).await,
        StatusCode::CONFLICT
    );
    assert_eq!(fixture.stored_blob().await.as_deref(), Some("first"));
}

#[tokio::test]
async fn the_session_route_follows_the_same_rules() {
    let fixture = setup().await;
    let (status, login) = call(
        &fixture.app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": fixture.email, "password": fixture.password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let session = login["sessionId"].as_str().unwrap();
    let updated_at = fixture.seed("first").await;

    assert_eq!(
        fixture.put_to(SESSION_ROUTE, session, body("old"), Some("2026-01-01T00:00:00+00:00")).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        fixture.put_to(SESSION_ROUTE, session, body("new"), Some(&updated_at)).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_web_client_on_another_origin_may_send_the_condition() {
    let db = migrated_db().await;
    let config = jottery_server::config::Config {
        cors_allowed_origins: Some(vec!["https://notes.example.com".to_string()]),
        ..test_config()
    };
    let app = app_with(&db.pool, config);

    let (_, headers, _) = call_full(
        &app,
        "OPTIONS",
        DEVICE_ROUTE,
        None,
        None,
        &[
            ("Origin", "https://notes.example.com"),
            ("Access-Control-Request-Method", "PUT"),
            ("Access-Control-Request-Headers", "if-match"),
        ],
    )
    .await;

    let allowed = headers[header::ACCESS_CONTROL_ALLOW_HEADERS].to_str().unwrap().to_lowercase();
    assert!(allowed.contains("if-match"), "{allowed}");
}
