use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{models::{CreateSessionParams, Session}, utils::crypto::hash_sha256};

/// Session repository for database operations
pub struct SessionRepository;

impl SessionRepository {
    /// Create a new session
    pub async fn create(
        pool: &SqlitePool,
        params: CreateSessionParams,
    ) -> Result<Session, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Hash the session token for storage
        let token_hash = hash_sha256(&params.token);

        sqlx::query!(
            r#"
            INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_used_at, user_agent, ip_address)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            id,
            params.user_id,
            token_hash,
            now,
            params.expires_at,
            now,
            params.user_agent,
            params.ip_address
        )
        .execute(pool)
        .await?;

        Self::get_by_id(pool, &id).await
    }

    /// Get session by ID
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Session, sqlx::Error> {
        sqlx::query_as!(
            Session,
            r#"SELECT * FROM sessions WHERE id = ?"#,
            id
        )
        .fetch_one(pool)
        .await
    }

    /// Get session by token
    pub async fn get_by_token(pool: &SqlitePool, token: &str) -> Result<Session, sqlx::Error> {
        // Hash the token to look it up
        let token_hash = hash_sha256(token);

        sqlx::query_as!(
            Session,
            r#"SELECT * FROM sessions WHERE token_hash = ?"#,
            token_hash
        )
        .fetch_one(pool)
        .await
    }

    /// Validate session and check expiry
    pub async fn validate_and_get(
        pool: &SqlitePool,
        token: &str,
    ) -> Result<Option<Session>, sqlx::Error> {
        let session = match Self::get_by_token(pool, token).await {
            Ok(s) => s,
            Err(sqlx::Error::RowNotFound) => return Ok(None),
            Err(e) => return Err(e),
        };

        // Check if session is expired
        let now = chrono::Utc::now();
        let expires_at = chrono::DateTime::parse_from_rfc3339(&session.expires_at)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        if expires_at < now {
            // Session expired, delete it
            Self::delete(pool, &session.id).await?;
            return Ok(None);
        }

        // Update last used timestamp
        Self::update_last_used(pool, &session.id).await?;

        Ok(Some(session))
    }

    /// Update last used timestamp
    pub async fn update_last_used(pool: &SqlitePool, session_id: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query!(
            r#"UPDATE sessions SET last_used_at = ? WHERE id = ?"#,
            now,
            session_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete a session (logout)
    pub async fn delete(pool: &SqlitePool, session_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"DELETE FROM sessions WHERE id = ?"#,
            session_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete all sessions for a user
    #[allow(dead_code)]
    pub async fn delete_all_for_user(
        pool: &SqlitePool,
        user_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"DELETE FROM sessions WHERE user_id = ?"#,
            user_id
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Delete expired sessions (cleanup)
    #[allow(dead_code)]
    pub async fn delete_expired(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query!(
            r#"DELETE FROM sessions WHERE expires_at < ?"#,
            now
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get all active sessions for a user
    #[allow(dead_code)]
    pub async fn get_by_user(pool: &SqlitePool, user_id: &str) -> Result<Vec<Session>, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query_as!(
            Session,
            r#"
            SELECT * FROM sessions
            WHERE user_id = ? AND expires_at > ?
            ORDER BY created_at DESC
            "#,
            user_id,
            now
        )
        .fetch_all(pool)
        .await
    }

    /// Count active sessions
    #[allow(dead_code)]
    pub async fn count_active(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM sessions WHERE expires_at > ?"#,
            now
        )
        .fetch_one(pool)
        .await?;

        Ok(result.count.into())
    }
}
