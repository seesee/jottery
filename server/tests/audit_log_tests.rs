// Who did what, and when. Without this, "who demoted Alice?" has no answer once
// the process logs are gone.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::services::AdminService;
use serde_json::{json, Value};
use sqlx::SqlitePool;

struct Fixture {
    db: TestDb,
    app: axum::Router,
    admin: String,
    token: String,
}

async fn setup() -> Fixture {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let admin = create_user(&db.pool, &email, &password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&admin)
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = body["sessionId"].as_str().unwrap().to_string();
    Fixture { db, app, admin, token }
}

async fn add_user(pool: &SqlitePool) -> String {
    create_user(pool, &format!("{}@example.com", new_id()), &random_password()).await
}

impl Fixture {
    async fn act(&self, method: &str, uri: &str, body: Option<Value>) -> StatusCode {
        let (status, _) = call(&self.app, method, uri, Some(&self.token), body).await;
        status
    }

    async fn log(&self, query: &str) -> Value {
        let uri = format!("/api/v1/admin/audit{query}");
        let (status, body) = call(&self.app, "GET", &uri, Some(&self.token), None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    async fn actions(&self) -> Vec<String> {
        self.log("").await["logs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["action"].as_str().unwrap().to_string())
            .collect()
    }
}

#[tokio::test]
async fn each_admin_action_is_recorded() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;
    let (device, _) = create_device(&f.db.pool, &user).await;
    sqlx::query("UPDATE users SET approved = 0 WHERE id = ?")
        .bind(&user)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let base = format!("/api/v1/admin/users/{user}");

    let steps: Vec<(&str, String, Option<Value>, StatusCode, &str)> = vec![
        ("POST", format!("{base}/approve"), None, StatusCode::NO_CONTENT, "user.approve"),
        ("POST", format!("{base}/deactivate"), None, StatusCode::NO_CONTENT, "user.deactivate"),
        ("POST", format!("{base}/activate"), None, StatusCode::NO_CONTENT, "user.activate"),
        (
            "POST",
            format!("{base}/toggle-admin"),
            Some(json!({ "is_admin": true })),
            StatusCode::NO_CONTENT,
            "user.admin_grant",
        ),
        (
            "POST",
            format!("{base}/toggle-admin"),
            Some(json!({ "is_admin": false })),
            StatusCode::NO_CONTENT,
            "user.admin_revoke",
        ),
        (
            "POST",
            format!("{base}/reset-password"),
            Some(json!({ "new_password": random_password() })),
            StatusCode::NO_CONTENT,
            "user.password_reset",
        ),
        (
            "PATCH",
            format!("{base}/settings"),
            Some(json!({ "storageQuotaMb": 2000 })),
            StatusCode::NO_CONTENT,
            "user.settings_update",
        ),
        (
            "PATCH",
            format!("/api/v1/admin/devices/{device}"),
            Some(json!({ "name": "renamed" })),
            StatusCode::NO_CONTENT,
            "device.rename",
        ),
        (
            "DELETE",
            format!("/api/v1/admin/devices/{device}"),
            None,
            StatusCode::NO_CONTENT,
            "device.revoke",
        ),
        (
            "DELETE",
            format!("/api/v1/admin/devices/{device}"),
            None,
            StatusCode::NO_CONTENT,
            "device.delete",
        ),
        ("DELETE", base.clone(), None, StatusCode::NO_CONTENT, "user.delete"),
    ];

    for (method, uri, body, expected, action) in steps {
        assert_eq!(f.act(method, &uri, body).await, expected, "{method} {uri}");

        let entries = f.log(&format!("?action={action}")).await;
        let entry = &entries["logs"][0];
        assert_eq!(entry["adminUserId"], json!(f.admin), "{action}");
        assert_eq!(entry["targetUserId"], json!(user), "{action}");
        assert!(entry["createdAt"].is_string(), "{action}");
    }
}

#[tokio::test]
async fn a_device_action_names_the_device() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;
    let (device, _) = create_device(&f.db.pool, &user).await;

    f.act("DELETE", &format!("/api/v1/admin/devices/{device}"), None).await;

    assert_eq!(f.log("").await["logs"][0]["targetResourceId"], json!(device));
}

#[tokio::test]
async fn changing_your_own_password_is_recorded_without_the_password() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let admin = create_user(&db.pool, &email, &password).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&admin)
        .execute(&db.pool)
        .await
        .unwrap();
    let (_, login) = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    let token = login["sessionId"].as_str().unwrap();
    let new_password = random_password();

    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/admin/change-password",
        Some(token),
        Some(json!({ "current_password": password, "new_password": new_password })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, log) = call(&app, "GET", "/api/v1/admin/audit", Some(token), None).await;
    assert_eq!(log["logs"][0]["action"], json!("admin.password_change"));
    assert_eq!(log["logs"][0]["targetUserId"], json!(admin));
    assert!(!log.to_string().contains(&new_password));
}

#[tokio::test]
async fn a_reset_says_whether_devices_went_with_it() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;

    f.act(
        "POST",
        &format!("/api/v1/admin/users/{user}/reset-password"),
        Some(json!({ "new_password": random_password(), "revokeDevices": true })),
    )
    .await;

    assert_eq!(f.log("").await["logs"][0]["metadata"], json!({ "revokedDevices": true }));
}

#[tokio::test]
async fn the_record_outlives_the_account() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;
    f.act("POST", &format!("/api/v1/admin/users/{user}/deactivate"), None).await;

    f.act("DELETE", &format!("/api/v1/admin/users/{user}"), None).await;

    let entries = f.log(&format!("?targetId={user}")).await;
    assert_eq!(entries["total"], json!(2));
}

#[tokio::test]
async fn it_says_where_the_request_came_from() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;
    let bearer = format!("Bearer {}", f.token);

    let (status, _, _) = call_from(
        &f.app,
        "203.0.113.7",
        "POST",
        &format!("/api/v1/admin/users/{user}/deactivate"),
        None,
        &[("Authorization", &bearer), ("User-Agent", "test-agent/1.0")],
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let entry = &f.log("").await["logs"][0];
    assert_eq!(entry["ipAddress"], json!("203.0.113.7"));
    assert_eq!(entry["userAgent"], json!("test-agent/1.0"));
}

#[tokio::test]
async fn nothing_is_recorded_when_nothing_happened() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;

    // Already approved, and there's no such user
    f.act("POST", &format!("/api/v1/admin/users/{user}/approve"), None).await;
    f.act("POST", &format!("/api/v1/admin/users/{}/approve", new_id()), None).await;
    f.act(
        "POST",
        &format!("/api/v1/admin/users/{}/toggle-admin", f.admin),
        Some(json!({ "is_admin": true })),
    )
    .await;

    assert!(f.actions().await.is_empty());
}

#[tokio::test]
async fn filters_narrow_the_listing() {
    let f = setup().await;
    let (first, second) = (add_user(&f.db.pool).await, add_user(&f.db.pool).await);
    f.act("POST", &format!("/api/v1/admin/users/{first}/deactivate"), None).await;
    f.act("POST", &format!("/api/v1/admin/users/{first}/activate"), None).await;
    f.act("POST", &format!("/api/v1/admin/users/{second}/deactivate"), None).await;

    assert_eq!(f.log("").await["total"], json!(3));
    assert_eq!(f.log(&format!("?targetId={first}")).await["total"], json!(2));
    assert_eq!(f.log("?action=user.deactivate").await["total"], json!(2));
    assert_eq!(f.log(&format!("?adminId={}", f.admin)).await["total"], json!(3));
    assert_eq!(f.log(&format!("?adminId={}", new_id())).await["total"], json!(0));

    assert_eq!(f.log("?from=2000-01-01T00:00:00Z").await["total"], json!(3));
    assert_eq!(f.log("?to=2000-01-01T00:00:00Z").await["total"], json!(0));
    // An offset is taken into account, whatever the zone
    assert_eq!(f.log("?from=2999-01-01T01:00:00%2B01:00").await["total"], json!(0));

    let page = f.log("?limit=1&offset=1").await;
    assert_eq!(page["logs"].as_array().unwrap().len(), 1);
    assert_eq!(page["total"], json!(3));
}

#[tokio::test]
async fn the_newest_entry_comes_first() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;
    f.act("POST", &format!("/api/v1/admin/users/{user}/deactivate"), None).await;
    f.act("POST", &format!("/api/v1/admin/users/{user}/activate"), None).await;

    assert_eq!(f.actions().await, vec!["user.activate", "user.deactivate"]);
}

#[tokio::test]
async fn an_unreadable_date_is_refused() {
    let f = setup().await;

    let (status, _) = call(
        &f.app,
        "GET",
        "/api/v1/admin/audit?from=yesterday",
        Some(&f.token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_failure_to_record_does_not_undo_the_action() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;
    sqlx::query("DROP TABLE admin_audit_log").execute(&f.db.pool).await.unwrap();

    let status = f.act("POST", &format!("/api/v1/admin/users/{user}/deactivate"), None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let active: i64 = sqlx::query_scalar("SELECT is_active FROM users WHERE id = ?")
        .bind(&user)
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(active, 0);
}

#[tokio::test]
async fn the_log_is_for_admins() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    create_user(&db.pool, &email, &password).await;
    let (_, login) = call(
        &app,
        "POST",
        "/api/v1/user/login",
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;

    let (status, _) = call(
        &app,
        "GET",
        "/api/v1/admin/audit",
        login["sessionId"].as_str(),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_command_line_appears_with_no_admin_named() {
    let f = setup().await;
    let user = add_user(&f.db.pool).await;

    AdminService::deactivate_user(&f.db.pool, &user).await.unwrap();

    let entry = &f.log("").await["logs"][0];
    assert_eq!(entry["action"], json!("user.deactivate"));
    assert_eq!(entry["adminUserId"], Value::Null);
    assert_eq!(entry["targetUserId"], json!(user));
}
