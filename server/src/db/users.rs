use sqlx::{Sqlite, SqliteConnection, SqlitePool, Transaction};
use uuid::Uuid;

use crate::{config::Config, models::User};

/// User repository for database operations
pub struct UserRepository;

impl UserRepository {
    /// Create a new user, with the limits new accounts get on this server. Returns
    /// `None` if the address is taken, in any case. The check is part of the insert,
    /// so that two registrations at once can't both pass it.
    pub async fn create(
        pool: &SqlitePool,
        email: &str,
        password_hash: &str,
        config: &Config,
    ) -> Result<Option<User>, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        let inserted = sqlx::query!(
            r#"
            INSERT INTO users (
                id, email, password_hash, created_at, approved, is_admin, is_active,
                storage_quota_mb, max_upload_size_mb, inbox_max_items, inbox_max_size_mb
            )
            SELECT ?1, ?2, ?3, ?4, 0, 0, 1, ?5, ?6, ?7, ?8
            WHERE NOT EXISTS (SELECT 1 FROM users WHERE email = ?2 COLLATE NOCASE)
            "#,
            id,
            email,
            password_hash,
            now,
            config.default_storage_quota_mb,
            config.default_max_upload_size_mb,
            config.default_inbox_max_items,
            config.default_inbox_max_size_mb
        )
        .execute(pool)
        .await?;

        if inserted.rows_affected() == 0 {
            return Ok(None);
        }

        Self::get_by_id(pool, &id).await.map(Some)
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
        Self::find_by_email(pool, email)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Find a user by email, if there is one, whatever case it was typed in. Accounts
    /// that differ only by case can exist from before that was refused, so the exact
    /// address wins: anything else would lock one of them out.
    pub async fn find_by_email(pool: &SqlitePool, email: &str) -> Result<Option<User>, sqlx::Error> {
        let email = email.trim();

        sqlx::query_as!(
            User,
            r#"SELECT * FROM users WHERE email = ?1 COLLATE NOCASE
               ORDER BY email = ?1 DESC, created_at
               LIMIT 1"#,
            email
        )
        .fetch_optional(pool)
        .await
    }

    /// Check if email exists, in any case
    pub async fn email_exists(pool: &SqlitePool, email: &str) -> Result<bool, sqlx::Error> {
        let email = email.trim();
        let result = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM users WHERE email = ? COLLATE NOCASE"#,
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

    /// Approve a user. `approved_by` is the admin who did, if it was one: the
    /// command line has no account behind it.
    pub async fn approve(
        pool: &SqlitePool,
        user_id: &str,
        approved_by: Option<&str>,
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

    /// What a user holds on the server, in bytes: blobs, and the content and tags of
    /// notes and versions. Reads every note, so it isn't for the request path.
    pub async fn measure_storage(pool: &SqlitePool, user_id: &str) -> Result<i64, sqlx::Error> {
        let bytes: i64 = sqlx::query_scalar(
            "SELECT
               COALESCE((SELECT SUM(LENGTH(content) + LENGTH(tags)) FROM notes WHERE user_id = ?1), 0)
             + COALESCE((SELECT SUM(LENGTH(content) + LENGTH(tags)) FROM note_versions WHERE user_id = ?1), 0)
             + COALESCE((SELECT SUM(LENGTH(d.data)) FROM attachments_meta m
                         JOIN attachments_data d ON d.id = m.id WHERE m.user_id = ?1), 0)",
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(bytes)
    }

    /// Set a user's password, which also satisfies a forced change.
    pub async fn set_password(
        conn: &mut SqliteConnection,
        user_id: &str,
        password_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE users SET password_hash = ?, must_change_password = 0 WHERE id = ?"#,
            password_hash,
            user_id
        )
        .execute(conn)
        .await?;

        Ok(())
    }

    /// Deactivate a user
    pub async fn deactivate(conn: &mut SqliteConnection, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"UPDATE users SET is_active = 0 WHERE id = ?"#,
            user_id
        )
        .execute(conn)
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

    /// Delete all notes and related data for a user within a transaction
    ///
    /// Every statement filters on the owner. Note ids are only unique per user,
    /// so matching on a note id alone would reach into other accounts.
    ///
    /// Deleting attachment metadata cascades to the blobs and to
    /// `note_attachments`.
    ///
    /// Returns the number of notes deleted.
    pub async fn delete_all_notes_data(
        tx: &mut Transaction<'_, Sqlite>,
        user_id: &str,
    ) -> Result<u64, sqlx::Error> {
        sqlx::query!("DELETE FROM note_versions WHERE user_id = ?", user_id)
            .execute(&mut **tx)
            .await?;

        sqlx::query!("DELETE FROM attachments_meta WHERE user_id = ?", user_id)
            .execute(&mut **tx)
            .await?;

        let result = sqlx::query!("DELETE FROM notes WHERE user_id = ?", user_id)
            .execute(&mut **tx)
            .await?;

        Ok(result.rows_affected())
    }

    /// Delete all user account data within a transaction
    ///
    /// Deletes in the correct order:
    /// 1. All notes data (via delete_all_notes_data)
    /// 2. Sessions
    /// 3. Clients (devices)
    /// 4. User record
    pub async fn delete_account_data(
        tx: &mut Transaction<'_, Sqlite>,
        user_id: &str,
    ) -> Result<(), sqlx::Error> {
        // Delete all notes and related data
        Self::delete_all_notes_data(tx, user_id).await?;

        // Delete sessions
        sqlx::query!("DELETE FROM sessions WHERE user_id = ?", user_id)
            .execute(&mut **tx)
            .await?;

        // Delete clients (devices)
        sqlx::query!("DELETE FROM clients WHERE user_id = ?", user_id)
            .execute(&mut **tx)
            .await?;

        // Delete the user record
        sqlx::query!("DELETE FROM users WHERE id = ?", user_id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }
}
