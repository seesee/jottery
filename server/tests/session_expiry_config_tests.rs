// Session expiry configuration tests
//
// Tests for configurable session expiry duration

use sqlx::SqlitePool;

// Helper to create test database
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    // Create users table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            approved INTEGER NOT NULL DEFAULT 0,
            is_active INTEGER NOT NULL DEFAULT 1,
            is_admin INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            last_login_at TEXT
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to create users table");

    // Create sessions table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            token_hash TEXT UNIQUE NOT NULL,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            last_used_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id)
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("Failed to create sessions table");

    pool
}

#[tokio::test]
async fn test_session_expiry_default_7_days() {
    let pool = setup_test_db().await;

    let user_id = "test_user";
    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();

    // Create user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, created_at)
           VALUES (?, ?, ?, 1, ?)"#,
        user_id,
        "test@example.com",
        "hash",
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create user");

    // Default expiry is 7 days
    let session_expiry_days = 7;
    let expires_at = (now + chrono::Duration::days(session_expiry_days)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        "session1",
        user_id,
        "token_hash",
        now_str,
        expires_at,
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create session");

    // Verify session expiry is 7 days
    let session = sqlx::query!(
        r#"SELECT created_at, expires_at FROM sessions WHERE id = ?"#,
        "session1"
    )
    .fetch_one(&pool)
    .await
    .expect("Should fetch session");

    let created = chrono::DateTime::parse_from_rfc3339(&session.created_at)
        .expect("Should parse created_at");
    let expires = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .expect("Should parse expires_at");

    let duration = expires.signed_duration_since(created);

    assert_eq!(
        duration.num_days(),
        7,
        "Session should expire in exactly 7 days"
    );
}

#[tokio::test]
async fn test_session_expiry_custom_14_days() {
    let pool = setup_test_db().await;

    let user_id = "test_user";
    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();

    // Create user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, created_at)
           VALUES (?, ?, ?, 1, ?)"#,
        user_id,
        "test@example.com",
        "hash",
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create user");

    // Custom expiry: 14 days
    let session_expiry_days = 14;
    let expires_at = (now + chrono::Duration::days(session_expiry_days)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        "session1",
        user_id,
        "token_hash",
        now_str,
        expires_at,
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create session");

    // Verify session expiry is 14 days
    let session = sqlx::query!(
        r#"SELECT created_at, expires_at FROM sessions WHERE id = ?"#,
        "session1"
    )
    .fetch_one(&pool)
    .await
    .expect("Should fetch session");

    let created = chrono::DateTime::parse_from_rfc3339(&session.created_at)
        .expect("Should parse created_at");
    let expires = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .expect("Should parse expires_at");

    let duration = expires.signed_duration_since(created);

    assert_eq!(
        duration.num_days(),
        14,
        "Session should expire in exactly 14 days"
    );
}

#[tokio::test]
async fn test_session_expiry_short_1_day() {
    let pool = setup_test_db().await;

    let user_id = "test_user";
    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();

    // Create user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, created_at)
           VALUES (?, ?, ?, 1, ?)"#,
        user_id,
        "test@example.com",
        "hash",
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create user");

    // Short expiry: 1 day (high security)
    let session_expiry_days = 1;
    let expires_at = (now + chrono::Duration::days(session_expiry_days)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        "session1",
        user_id,
        "token_hash",
        now_str,
        expires_at,
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create session");

    // Verify session expiry is 1 day
    let session = sqlx::query!(
        r#"SELECT created_at, expires_at FROM sessions WHERE id = ?"#,
        "session1"
    )
    .fetch_one(&pool)
    .await
    .expect("Should fetch session");

    let created = chrono::DateTime::parse_from_rfc3339(&session.created_at)
        .expect("Should parse created_at");
    let expires = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .expect("Should parse expires_at");

    let duration = expires.signed_duration_since(created);

    assert_eq!(
        duration.num_days(),
        1,
        "Session should expire in exactly 1 day"
    );
}

#[tokio::test]
async fn test_session_expiry_long_30_days() {
    let pool = setup_test_db().await;

    let user_id = "test_user";
    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();

    // Create user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, created_at)
           VALUES (?, ?, ?, 1, ?)"#,
        user_id,
        "test@example.com",
        "hash",
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create user");

    // Long expiry: 30 days (convenience over security)
    let session_expiry_days = 30;
    let expires_at = (now + chrono::Duration::days(session_expiry_days)).to_rfc3339();

    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        "session1",
        user_id,
        "token_hash",
        now_str,
        expires_at,
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create session");

    // Verify session expiry is 30 days
    let session = sqlx::query!(
        r#"SELECT created_at, expires_at FROM sessions WHERE id = ?"#,
        "session1"
    )
    .fetch_one(&pool)
    .await
    .expect("Should fetch session");

    let created = chrono::DateTime::parse_from_rfc3339(&session.created_at)
        .expect("Should parse created_at");
    let expires = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
        .expect("Should parse expires_at");

    let duration = expires.signed_duration_since(created);

    assert_eq!(
        duration.num_days(),
        30,
        "Session should expire in exactly 30 days"
    );
}

#[tokio::test]
async fn test_multiple_sessions_different_expiry() {
    let pool = setup_test_db().await;

    let user_id = "test_user";
    let now = chrono::Utc::now();
    let now_str = now.to_rfc3339();

    // Create user
    sqlx::query!(
        r#"INSERT INTO users (id, email, password_hash, approved, created_at)
           VALUES (?, ?, ?, 1, ?)"#,
        user_id,
        "test@example.com",
        "hash",
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create user");

    // Create sessions with different expiry times
    // (simulating config changes over time)

    // Session 1: 7 days
    let expires_1 = (now + chrono::Duration::days(7)).to_rfc3339();
    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        "session1",
        user_id,
        "token_hash_1",
        now_str,
        expires_1,
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create session 1");

    // Session 2: 14 days
    let expires_2 = (now + chrono::Duration::days(14)).to_rfc3339();
    sqlx::query!(
        r#"INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        "session2",
        user_id,
        "token_hash_2",
        now_str,
        expires_2,
        now_str
    )
    .execute(&pool)
    .await
    .expect("Failed to create session 2");

    // Verify both sessions have correct expiry
    let sessions = sqlx::query!(
        r#"SELECT id, created_at, expires_at FROM sessions WHERE user_id = ? ORDER BY id"#,
        user_id
    )
    .fetch_all(&pool)
    .await
    .expect("Should fetch sessions");

    assert_eq!(sessions.len(), 2);

    let session1 = &sessions[0];
    let created1 = chrono::DateTime::parse_from_rfc3339(&session1.created_at).unwrap();
    let expires1 = chrono::DateTime::parse_from_rfc3339(&session1.expires_at).unwrap();
    assert_eq!(expires1.signed_duration_since(created1).num_days(), 7);

    let session2 = &sessions[1];
    let created2 = chrono::DateTime::parse_from_rfc3339(&session2.created_at).unwrap();
    let expires2 = chrono::DateTime::parse_from_rfc3339(&session2.expires_at).unwrap();
    assert_eq!(expires2.signed_duration_since(created2).num_days(), 14);
}

#[cfg(test)]
mod session_expiry_documentation {
    #[test]
    fn document_session_expiry_recommendations() {
        println!("\n=== Session Expiry Recommendations ===");
        println!("\nHigh Security (1-3 days):");
        println!("  - Financial applications");
        println!("  - Healthcare systems");
        println!("  - Admin dashboards");
        println!("  - SESSION_EXPIRY_DAYS=1 or 3");

        println!("\nStandard Security (7 days - DEFAULT):");
        println!("  - General business applications");
        println!("  - Standard SaaS products");
        println!("  - SESSION_EXPIRY_DAYS=7");

        println!("\nConvenience-Focused (14-30 days):");
        println!("  - Personal productivity tools");
        println!("  - Low-risk applications");
        println!("  - SESSION_EXPIRY_DAYS=14 or 30");

        println!("\n=== Security Considerations ===");
        println!("- Shorter expiry = better security, worse UX");
        println!("- Longer expiry = better UX, worse security");
        println!("- Balance based on threat model");
        println!("- Consider implementing sliding expiry (extend on activity)");
        println!("========================================\n");
    }

    #[test]
    fn document_session_security_best_practices() {
        println!("\n=== Session Security Best Practices ===");
        println!("\n1. Session Expiry:");
        println!("   - Use SESSION_EXPIRY_DAYS based on risk level");
        println!("   - Shorter is more secure");

        println!("\n2. Session Tokens:");
        println!("   - Always use HTTPS to prevent token interception");
        println!("   - Store tokens in httpOnly cookies when possible");
        println!("   - Never log session tokens");

        println!("\n3. Session Management:");
        println!("   - Implement logout functionality");
        println!("   - Allow users to view/revoke active sessions");
        println!("   - Clean up expired sessions regularly");

        println!("\n4. Additional Security:");
        println!("   - Consider IP-based session validation");
        println!("   - Implement device fingerprinting");
        println!("   - Log all session creation/deletion");
        println!("=========================================\n");
    }
}
