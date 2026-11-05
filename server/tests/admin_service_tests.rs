// One set of rules for the dashboard and jottery-admin. The two used to drift: the
// command line couldn't approve anyone at all, and only some paths guarded the last
// admin, leaving a server with nobody able to approve users.

mod common;

use axum::http::StatusCode;
use common::*;
use jottery_server::services::{AdminError, AdminService};
use serde_json::json;
use sqlx::SqlitePool;

async fn add_user(pool: &SqlitePool) -> (String, String, String) {
    let email = format!("{}@example.com", new_id());
    let password = random_password();
    let id = create_user(pool, &email, &password).await;
    (id, email, password)
}

async fn add_admin(pool: &SqlitePool) -> (String, String, String) {
    let user = add_user(pool).await;
    sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
        .bind(&user.0)
        .execute(pool)
        .await
        .unwrap();
    user
}

/// The migrations seed an admin. Take it out, so that the counts below are real.
async fn remove_seeded_admin(pool: &SqlitePool) {
    sqlx::query("DELETE FROM users WHERE email = 'admin@localhost'")
        .execute(pool)
        .await
        .unwrap();
}

async fn add_pending(pool: &SqlitePool) -> String {
    let (id, _, _) = add_user(pool).await;
    sqlx::query("UPDATE users SET approved = 0 WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn login(app: &axum::Router, uri: &str, email: &str, password: &str) -> String {
    let (status, body) = call(
        app,
        "POST",
        uri,
        None,
        Some(json!({ "email": email, "password": password })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["sessionId"].as_str().unwrap().to_string()
}

async fn user_exists(pool: &SqlitePool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
        > 0
}

async fn is_active(pool: &SqlitePool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT is_active FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
        == 1
}

#[tokio::test]
async fn the_command_line_can_approve_a_user() {
    let db = migrated_db().await;
    let id = add_pending(&db.pool).await;

    let user = AdminService::approve_user(&db.pool, &id).await.unwrap();

    assert_eq!(user.approved, 1);
    assert!(user.approved_at.is_some());
    assert_eq!(user.approved_by, None);
}

#[tokio::test]
async fn approving_twice_is_reported() {
    let db = migrated_db().await;
    let id = add_pending(&db.pool).await;
    AdminService::approve_user(&db.pool, &id).await.unwrap();

    let again = AdminService::approve_user(&db.pool, &id).await;

    assert!(matches!(again, Err(AdminError::AlreadyInState(_))));
}

#[tokio::test]
async fn the_dashboard_records_who_approved() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (admin, email, password) = add_admin(&db.pool).await;
    let token = login(&app, "/api/v1/auth/login", &email, &password).await;
    let id = add_pending(&db.pool).await;
    let uri = format!("/api/v1/admin/users/{id}/approve");

    let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let approved_by: Option<String> = sqlx::query_scalar("SELECT approved_by FROM users WHERE id = ?")
        .bind(&id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(approved_by, Some(admin));

    // A second click isn't an error to the person making it
    let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn the_dashboard_reports_an_unknown_user() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (_, email, password) = add_admin(&db.pool).await;
    let token = login(&app, "/api/v1/auth/login", &email, &password).await;

    for action in ["approve", "deactivate", "activate"] {
        let uri = format!("/api/v1/admin/users/{}/{action}", new_id());
        let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{action}");
    }
}

#[tokio::test]
async fn the_dashboard_matches_ids_exactly() {
    // The command line accepts a prefix. A web route must never act on a neighbour.
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (_, email, password) = add_admin(&db.pool).await;
    let token = login(&app, "/api/v1/auth/login", &email, &password).await;
    let id = add_pending(&db.pool).await;

    let uri = format!("/api/v1/admin/users/{}/approve", &id[..8]);
    let (status, _) = call(&app, "POST", &uri, Some(&token), None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_last_admin_cannot_be_deactivated_on_any_route() {
    let db = migrated_db().await;
    remove_seeded_admin(&db.pool).await;
    let app = app(&db.pool);
    let (admin, email, password) = add_admin(&db.pool).await;
    let token = login(&app, "/api/v1/auth/login", &email, &password).await;

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/admin/users/{admin}/deactivate"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let refused = AdminService::deactivate_user(&db.pool, &admin).await;
    assert!(matches!(refused, Err(AdminError::LastAdmin)));

    let (status, _) = call(
        &app,
        "DELETE",
        "/api/v1/user/account?mode=deactivate",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    assert!(is_active(&db.pool, &admin).await);
}

#[tokio::test]
async fn the_last_admin_cannot_be_deleted_on_any_route() {
    let db = migrated_db().await;
    remove_seeded_admin(&db.pool).await;
    let app = app(&db.pool);
    let (admin, email, password) = add_admin(&db.pool).await;
    let token = login(&app, "/api/v1/auth/login", &email, &password).await;

    let (status, _) = call(
        &app,
        "DELETE",
        "/api/v1/user/account?mode=delete",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let refused = AdminService::delete_user(&db.pool, &admin).await;
    assert!(matches!(refused, Err(AdminError::LastAdmin)));

    assert!(user_exists(&db.pool, &admin).await);
}

#[tokio::test]
async fn an_inactive_admin_does_not_count_as_a_spare() {
    let db = migrated_db().await;
    remove_seeded_admin(&db.pool).await;
    let (admin, _, _) = add_admin(&db.pool).await;
    let (spare, _, _) = add_admin(&db.pool).await;
    sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
        .bind(&spare)
        .execute(&db.pool)
        .await
        .unwrap();

    let refused = AdminService::deactivate_user(&db.pool, &admin).await;

    assert!(matches!(refused, Err(AdminError::LastAdmin)));
}

#[tokio::test]
async fn an_admin_can_go_while_another_remains() {
    let db = migrated_db().await;
    remove_seeded_admin(&db.pool).await;
    let (first, _, _) = add_admin(&db.pool).await;
    let (second, _, _) = add_admin(&db.pool).await;

    AdminService::deactivate_user(&db.pool, &first).await.unwrap();

    assert!(!is_active(&db.pool, &first).await);
    assert!(matches!(
        AdminService::deactivate_user(&db.pool, &second).await,
        Err(AdminError::LastAdmin)
    ));
}

#[tokio::test]
async fn deactivating_a_user_ends_their_sessions() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, email, password) = add_user(&db.pool).await;
    let token = login(&app, "/api/v1/user/login", &email, &password).await;

    AdminService::deactivate_user(&db.pool, &id).await.unwrap();
    AdminService::activate_user(&db.pool, &id).await.unwrap();

    // Reactivating must not revive a session from before
    let (status, _) = call(&app, "GET", "/api/v1/user/account", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn deleting_a_user_from_the_command_line_takes_all_their_data() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (id, _, _) = add_user(&db.pool).await;
    let (_, key) = create_device(&db.pool, &id).await;
    let attachment = new_id();
    push(
        &app,
        &key,
        json!({
            "notes": [note_with_attachments(&new_id(), "text", "2026-02-01T00:00:00Z", &[&attachment])],
            "attachments": [blob(&attachment, b"data")],
        }),
    )
    .await;

    AdminService::delete_user(&db.pool, &id).await.unwrap();

    assert!(!user_exists(&db.pool, &id).await);
    for table in ["notes", "clients", "attachments_meta", "attachments_data"] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "{table} kept a row");
    }
}

async fn toggle(app: &axum::Router, token: &str, user_id: &str, is_admin: bool) -> StatusCode {
    let (status, _) = call(
        app,
        "POST",
        &format!("/api/v1/admin/users/{user_id}/toggle-admin"),
        Some(token),
        Some(json!({ "is_admin": is_admin })),
    )
    .await;
    status
}

async fn session_works(app: &axum::Router, token: &str) -> bool {
    let (status, _) = call(app, "GET", "/api/v1/user/account", Some(token), None).await;
    status == StatusCode::OK
}

#[tokio::test]
async fn a_change_of_role_ends_that_users_sessions() {
    // A token captured before a promotion mustn't become an admin token
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (_, admin_email, admin_password) = add_admin(&db.pool).await;
    let admin_token = login(&app, "/api/v1/auth/login", &admin_email, &admin_password).await;
    let (id, email, password) = add_user(&db.pool).await;

    let before_promotion = login(&app, "/api/v1/user/login", &email, &password).await;
    assert_eq!(toggle(&app, &admin_token, &id, true).await, StatusCode::NO_CONTENT);
    assert!(!session_works(&app, &before_promotion).await);

    let before_demotion = login(&app, "/api/v1/user/login", &email, &password).await;
    assert_eq!(toggle(&app, &admin_token, &id, false).await, StatusCode::NO_CONTENT);
    assert!(!session_works(&app, &before_demotion).await);

    assert!(session_works(&app, &admin_token).await, "the acting admin stays signed in");
}

#[tokio::test]
async fn asking_for_the_role_a_user_already_has_changes_nothing() {
    let db = migrated_db().await;
    let app = app(&db.pool);
    let (admin, email, password) = add_admin(&db.pool).await;
    let token = login(&app, "/api/v1/auth/login", &email, &password).await;

    assert_eq!(toggle(&app, &token, &admin, true).await, StatusCode::NO_CONTENT);

    assert!(session_works(&app, &token).await);
}

async fn user_with_id(pool: &SqlitePool, id: &str) {
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, approved, is_active, created_at)
         VALUES (?, ?, 'unused', 0, 1, '2026-01-01T00:00:00+00:00')",
    )
    .bind(id)
    .bind(format!("{id}@example.com"))
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn a_prefix_that_fits_two_users_is_refused() {
    let db = migrated_db().await;
    user_with_id(&db.pool, "abcdef12-0000").await;
    user_with_id(&db.pool, "abcdef12-1111").await;

    let result = AdminService::approve_user(&db.pool, "abcdef12").await;

    assert!(matches!(result, Err(AdminError::AmbiguousId(_))));
}

#[tokio::test]
async fn a_prefix_that_fits_one_user_finds_them() {
    let db = migrated_db().await;
    user_with_id(&db.pool, "abcdef12-0000").await;
    user_with_id(&db.pool, "abcdef99-1111").await;

    let user = AdminService::approve_user(&db.pool, "abcdef12").await.unwrap();

    assert_eq!(user.id, "abcdef12-0000");
}

#[tokio::test]
async fn an_exact_id_wins_over_a_longer_one_it_is_a_prefix_of() {
    let db = migrated_db().await;
    user_with_id(&db.pool, "abcdef12").await;
    user_with_id(&db.pool, "abcdef12-1111").await;

    let user = AdminService::approve_user(&db.pool, "abcdef12").await.unwrap();

    assert_eq!(user.id, "abcdef12");
}

#[tokio::test]
async fn a_short_prefix_is_refused() {
    let db = migrated_db().await;
    user_with_id(&db.pool, "abcdef12-0000").await;

    for identifier in ["", "a", "abcdef1"] {
        let result = AdminService::approve_user(&db.pool, identifier).await;
        assert!(matches!(result, Err(AdminError::UserNotFound(_))), "{identifier:?}");
    }
}

#[tokio::test]
async fn wildcards_in_an_identifier_match_nothing() {
    let db = migrated_db().await;
    user_with_id(&db.pool, "abcdef12-0000").await;

    for identifier in ["%%%%%%%%", "________", "abcdef1%", "abcdef1_"] {
        let result = AdminService::approve_user(&db.pool, identifier).await;
        assert!(matches!(result, Err(AdminError::UserNotFound(_))), "{identifier:?}");
    }
}

#[tokio::test]
async fn a_device_prefix_follows_the_same_rules() {
    let db = migrated_db().await;
    let (user, _, _) = add_user(&db.pool).await;
    let (device, _) = create_device(&db.pool, &user).await;
    for id in ["fedcba98-0000", "fedcba98-1111"] {
        sqlx::query(
            "INSERT INTO clients (id, user_id, api_key, device_name, device_type, created_at, last_seen_at, is_active)
             VALUES (?, ?, ?, 'test', 'web', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00', 1)",
        )
        .bind(id)
        .bind(&user)
        .bind(new_id())
        .execute(&db.pool)
        .await
        .unwrap();
    }

    let ambiguous = AdminService::revoke_device(&db.pool, "fedcba98").await;
    assert!(matches!(ambiguous, Err(AdminError::AmbiguousId(_))));

    let wildcard = AdminService::revoke_device(&db.pool, "%%%%%%%%").await;
    assert!(matches!(wildcard, Err(AdminError::DeviceNotFound(_))));

    let short = AdminService::revoke_device(&db.pool, "").await;
    assert!(matches!(short, Err(AdminError::DeviceNotFound(_))));

    let found = AdminService::revoke_device(&db.pool, &device[..13]).await.unwrap();
    assert_eq!(found.id, device);
}
