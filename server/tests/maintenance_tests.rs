// What housekeeping removes, and what it must leave.

mod common;

use common::*;
use jottery_server::maintenance;
use sqlx::SqlitePool;

async fn add_session(pool: &SqlitePool, user_id: &str, expires_at: &str) -> String {
    let id = new_id();
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(new_id())
    .bind("2026-01-01T00:00:00+00:00")
    .bind(expires_at)
    .bind("2026-01-01T00:00:00+00:00")
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn session_exists(pool: &SqlitePool, id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sessions WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
        > 0
}

#[tokio::test]
async fn expired_sessions_are_removed_and_live_ones_kept() {
    let db = migrated_db().await;
    let user = create_user(&db.pool, &format!("{}@example.com", new_id()), &random_password()).await;
    let expired = add_session(&db.pool, &user, "2020-01-01T00:00:00+00:00").await;
    let live = add_session(&db.pool, &user, "2999-01-01T00:00:00+00:00").await;

    maintenance::run_once(&db.pool).await.unwrap();

    assert!(!session_exists(&db.pool, &expired).await);
    assert!(session_exists(&db.pool, &live).await);
}
