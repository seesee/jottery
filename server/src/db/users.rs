use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::User;

/// User repository for database operations
pub struct UserRepository;

impl UserRepository {
    /// Create a new user
    pub async fn create(
        pool: &SqlitePool,
        email: &str,
        password_hash: &str,
    ) -> Result<User, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query!(
            r#"
            INSERT INTO users (id, email, password_hash, created_at, approved, is_admin, is_active, storage_quota_mb)
            VALUES (?, ?, ?, ?, 0, 0, 1, 1000)
            "#,
            id,
            email,
            password_hash,
            now
        )
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id).await
    }

    /// Get user by ID
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT * FROM users WHERE id = ?"#,
            id
        )
        .fetch_one(pool)
        .await
    }

    /// Get user by email
    pub async fn get_by_email(pool: &SqlitePool, email: &str) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT * FROM users WHERE email = ?"#,
            email
        )
        .fetch_one(pool)
        .await
    }

    /// Check if email exists
    pub async fn email_exists(pool: &SqlitePool, email: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM users WHERE email = ?"#,
            email
        )
        .fetch_one(pool)
        .await?;

        Ok(result.count > 0)
    }

    /// Get all users (for admin)
    #[allow(dead_code)]
    pub async fn get_all(pool: &SqlitePool) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT * FROM users ORDER BY created_at DESC"#
        )
        .fetch_all(pool)
        .await
    }

    /// Get pending users (not yet approved)
    #[allow(dead_code)]
    pub async fn get_pending(pool: &SqlitePool) -> Result<Vec<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT * FROM users WHERE approved = 0 ORDER BY created_at ASC"#
        )
        .fetch_all(pool)
        .await
    }

    /// Approve a user
    #[allow(dead_code)]
    pub async fn approve(
        pool: &SqlitePool,
        user_id: &str,
        approved_by: &str,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query!(
            r#"
            UPDATE users
            SET approved = 1, approved_at = ?, approved_by = ?
            WHERE id = ?
            "#,
            now,
            approved_by,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Deactivate a user
    #[allow(dead_code)]
    pub async fn deactivate(pool: &SqlitePool, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE users SET is_active = 0 WHERE id = ?"#,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Reactivate a user
    #[allow(dead_code)]
    pub async fn activate(pool: &SqlitePool, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE users SET is_active = 1 WHERE id = ?"#,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Update last login timestamp
    pub async fn update_last_login(pool: &SqlitePool, user_id: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query!(
            r#"UPDATE users SET last_login_at = ? WHERE id = ?"#,
            now,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete a user (hard delete)
    #[allow(dead_code)]
    pub async fn delete(pool: &SqlitePool, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"DELETE FROM users WHERE id = ?"#,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Get user count by approval status
    #[allow(dead_code)]
    pub async fn get_count_by_status(
        pool: &SqlitePool,
    ) -> Result<(i64, i64, i64), sqlx::Error> {
        let total = sqlx::query!(r#"SELECT COUNT(*) as count FROM users"#)
            .fetch_one(pool)
            .await?
            .count
            .into();

        let approved = sqlx::query!(r#"SELECT COUNT(*) as count FROM users WHERE approved = 1"#)
            .fetch_one(pool)
            .await?
            .count
            .into();

        let pending = sqlx::query!(r#"SELECT COUNT(*) as count FROM users WHERE approved = 0"#)
            .fetch_one(pool)
            .await?
            .count
            .into();

        Ok((total, approved, pending))
    }

    /// Get note count for a user
    #[allow(dead_code)]
    pub async fn get_note_count(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM notes WHERE user_id = ? AND deleted = 0"#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        Ok(result.count.into())
    }

    /// Get device count for a user
    #[allow(dead_code)]
    pub async fn get_device_count(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM clients WHERE user_id = ? AND is_active = 1"#,
            user_id
        )
        .fetch_one(pool)
        .await?;

        Ok(result.count.into())
    }
}
