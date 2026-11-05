//! Admin service - business logic for server administration
//!
//! This module provides the core business logic for administrative operations
//! that can be used by both CLI tools and web APIs.

use crate::audit::{self, AuditActor, AuditEvent};
use crate::db::{SessionRepository, UserRepository};
use crate::models::User;
use crate::utils::password::{hash_password_with_params, validate_password_strength};
use sqlx::{SqliteConnection, SqlitePool};

/// Device information returned by admin service
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub user_id: String,
    pub device_name: String,
    pub device_type: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub is_active: bool,
}

/// User information with statistics
#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user: User,
    pub note_count: i64,
    pub device_count: i64,
    pub devices: Vec<DeviceInfo>,
}

/// Server statistics
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub total_users: i64,
    pub approved_users: i64,
    pub pending_users: i64,
    pub inactive_users: i64,
    pub total_devices: i64,
    pub active_devices: i64,
    pub total_notes: i64,
    pub deleted_notes: i64,
    pub total_versions: i64,
    pub total_attachments: i64,
    pub total_storage_bytes: i64,
}

/// Filter options for listing users
#[derive(Debug, Clone, Default)]
pub struct UserListFilter {
    pub pending_only: bool,
    pub active_only: bool,
}

/// Admin service errors
#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("This is the last admin, and a server needs one")]
    LastAdmin,

    #[error("{0} matches more than one id: use more of it")]
    AmbiguousId(String),

    #[error("User is already {0}")]
    AlreadyInState(String),

    #[error("{0}")]
    WeakPassword(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Password hashing failed: {0}")]
    PasswordHash(String),
}

pub type Result<T> = std::result::Result<T, AdminError>;

/// Shortest id prefix accepted. Some commands act at once with no confirmation, so a
/// prefix has to be long enough to mean something.
const MIN_ID_PREFIX: usize = 8;

/// A LIKE pattern for ids that start with `prefix`, with its wildcards made literal.
/// `None` if it's too short to use.
fn id_prefix_pattern(prefix: &str) -> Option<String> {
    if prefix.len() < MIN_ID_PREFIX {
        return None;
    }

    let escaped = prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    Some(format!("{escaped}%"))
}

/// Admin service for server management operations
pub struct AdminService;

impl AdminService {
    /// List all users with optional filtering
    pub async fn list_users(pool: &SqlitePool, filter: UserListFilter) -> Result<Vec<User>> {
        let users = UserRepository::get_all(pool).await?;

        let filtered: Vec<_> = users
            .into_iter()
            .filter(|u| {
                if filter.pending_only {
                    u.approved == 0
                } else if filter.active_only {
                    u.is_active == 1 && u.approved == 1
                } else {
                    true
                }
            })
            .collect();

        Ok(filtered)
    }

    /// Get detailed user information including statistics and devices
    pub async fn get_user_info(pool: &SqlitePool, identifier: &str) -> Result<UserInfo> {
        let user = Self::find_user(pool, identifier).await?;
        let note_count = UserRepository::get_note_count(pool, &user.id).await.unwrap_or(0);
        let device_count = UserRepository::get_device_count(pool, &user.id).await.unwrap_or(0);
        let devices = Self::get_devices_for_user(pool, &user.id).await?;

        Ok(UserInfo {
            user,
            note_count,
            device_count,
            devices,
        })
    }

    /// Record an action taken with jottery-admin, which has no account or address.
    /// The methods that find a user by a prefix are its entry points: the dashboard
    /// uses the ones below them, and records its own actions with who took them.
    async fn record_for_cli(pool: &SqlitePool, event: AuditEvent<'_>) {
        audit::record(pool, &AuditActor::default(), event).await;
    }

    /// Refuse if taking `user` out of the admin role, however that happens, would
    /// leave the server without an admin. Nobody could then approve users, and an
    /// image without jottery-admin can't be repaired from inside. Call it inside
    /// the write transaction, so that two admins can't each leave the other last.
    pub async fn ensure_not_last_admin(conn: &mut SqliteConnection, user: &User) -> Result<()> {
        if user.is_admin != 1 || user.is_active != 1 {
            return Ok(());
        }

        let active_admins =
            sqlx::query_scalar!("SELECT COUNT(*) FROM users WHERE is_admin = 1 AND is_active = 1")
                .fetch_one(conn)
                .await?;

        if active_admins <= 1 {
            return Err(AdminError::LastAdmin);
        }
        Ok(())
    }

    /// Approve a pending user. `approved_by` is the acting admin's id, if any.
    pub async fn approve(pool: &SqlitePool, user: &User, approved_by: Option<&str>) -> Result<()> {
        if user.approved == 1 {
            return Err(AdminError::AlreadyInState("approved".to_string()));
        }

        UserRepository::approve(pool, &user.id, approved_by).await?;
        Ok(())
    }

    /// Deactivate an account. Its devices stop working at once, and a session from
    /// before must not come back if the account is reactivated.
    pub async fn deactivate(pool: &SqlitePool, user: &User) -> Result<()> {
        if user.is_active == 0 {
            return Err(AdminError::AlreadyInState("inactive".to_string()));
        }

        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::ensure_not_last_admin(&mut tx, user).await?;
        UserRepository::deactivate(&mut tx, &user.id).await?;
        SessionRepository::delete_all_for_user(&mut tx, &user.id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Reactivate an account.
    pub async fn activate(pool: &SqlitePool, user: &User) -> Result<()> {
        if user.is_active == 1 {
            return Err(AdminError::AlreadyInState("active".to_string()));
        }

        UserRepository::activate(pool, &user.id).await?;
        Ok(())
    }

    /// Delete an account and everything in it.
    pub async fn delete(pool: &SqlitePool, user: &User) -> Result<()> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::ensure_not_last_admin(&mut tx, user).await?;
        UserRepository::delete_account_data(&mut tx, &user.id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Approve a pending user, found by id, id prefix or email
    pub async fn approve_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;
        Self::approve(pool, &user, None).await?;
        Self::record_for_cli(pool, AuditEvent::on_user(audit::USER_APPROVE, &user.id)).await;
        Self::find_user(pool, &user.id).await
    }

    /// Deactivate a user account, found by id, id prefix or email
    pub async fn deactivate_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;
        Self::deactivate(pool, &user).await?;
        Self::record_for_cli(pool, AuditEvent::on_user(audit::USER_DEACTIVATE, &user.id)).await;
        Self::find_user(pool, &user.id).await
    }

    /// Activate a user account, found by id, id prefix or email
    pub async fn activate_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;
        Self::activate(pool, &user).await?;
        Self::record_for_cli(pool, AuditEvent::on_user(audit::USER_ACTIVATE, &user.id)).await;
        Self::find_user(pool, &user.id).await
    }

    /// Delete a user and all their data, found by id, id prefix or email
    pub async fn delete_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;
        Self::delete(pool, &user).await?;
        Self::record_for_cli(pool, AuditEvent::on_user(audit::USER_DELETE, &user.id)).await;
        Ok(user)
    }

    /// Reset a user's password
    pub async fn reset_password(
        pool: &SqlitePool,
        identifier: &str,
        new_password: &str,
        argon2_params: (u32, u32, u32), // (m_cost, t_cost, p_cost)
        password_complexity: &str,
    ) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;

        validate_password_strength(new_password, password_complexity)
            .map_err(AdminError::WeakPassword)?;

        let (m_cost, t_cost, p_cost) = argon2_params;
        let password_hash = hash_password_with_params(new_password, m_cost, t_cost, p_cost)
            .map_err(|e| AdminError::PasswordHash(e.to_string()))?;

        let mut tx = pool.begin().await?;
        UserRepository::set_password(&mut tx, &user.id, &password_hash).await?;
        SessionRepository::delete_all_for_user(&mut tx, &user.id).await?;
        tx.commit().await?;

        Self::record_for_cli(pool, AuditEvent::on_user(audit::USER_PASSWORD_RESET, &user.id)).await;

        Ok(user)
    }

    /// List all devices, optionally filtered by user
    pub async fn list_devices(pool: &SqlitePool, user_identifier: Option<&str>) -> Result<Vec<DeviceInfo>> {
        if let Some(identifier) = user_identifier {
            let user = Self::find_user(pool, identifier).await?;
            Self::get_devices_for_user(pool, &user.id).await
        } else {
            Self::get_all_devices(pool).await
        }
    }

    /// Delete a device by ID
    pub async fn delete_device(pool: &SqlitePool, device_id: &str) -> Result<DeviceInfo> {
        let device = Self::find_device(pool, device_id).await?;

        sqlx::query!("DELETE FROM clients WHERE id = ?", device.id)
            .execute(pool)
            .await?;

        let event = AuditEvent {
            target_user_id: Some(&device.user_id),
            ..AuditEvent::on_device(audit::DEVICE_DELETE, &device.id)
        };
        Self::record_for_cli(pool, event).await;

        Ok(device)
    }

    /// Revoke (deactivate) a device
    pub async fn revoke_device(pool: &SqlitePool, device_id: &str) -> Result<DeviceInfo> {
        let device = Self::find_device(pool, device_id).await?;

        sqlx::query!("UPDATE clients SET is_active = 0 WHERE id = ?", device.id)
            .execute(pool)
            .await?;

        let event = AuditEvent {
            target_user_id: Some(&device.user_id),
            ..AuditEvent::on_device(audit::DEVICE_REVOKE, &device.id)
        };
        Self::record_for_cli(pool, event).await;

        // Return updated device
        Self::find_device(pool, &device.id).await
    }

    /// Get server statistics
    pub async fn get_stats(pool: &SqlitePool) -> Result<ServerStats> {
        let (total_users, approved_users, pending_users) =
            UserRepository::get_count_by_status(pool).await?;

        let inactive_users: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM users WHERE is_active = 0")
                .fetch_one(pool)
                .await?
                .into();

        let total_devices: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM clients")
            .fetch_one(pool)
            .await?
            .into();

        let active_devices: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM clients WHERE is_active = 1")
                .fetch_one(pool)
                .await?
                .into();

        let total_notes: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM notes WHERE deleted = 0")
                .fetch_one(pool)
                .await?
                .into();

        let deleted_notes: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM notes WHERE deleted = 1")
                .fetch_one(pool)
                .await?
                .into();

        let total_attachments: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM attachments_meta")
                .fetch_one(pool)
                .await?
                .into();

        let total_storage_bytes: i64 =
            sqlx::query_scalar!("SELECT COALESCE(SUM(size), 0) FROM attachments_meta")
                .fetch_one(pool)
                .await?;

        let total_versions: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM note_versions")
            .fetch_one(pool)
            .await?
            .into();

        Ok(ServerStats {
            total_users,
            approved_users,
            pending_users,
            inactive_users,
            total_devices,
            active_devices,
            total_notes,
            deleted_notes,
            total_versions,
            total_attachments,
            total_storage_bytes,
        })
    }

    /// Get user's email by device ID
    pub async fn get_user_email_for_device(pool: &SqlitePool, user_id: &str) -> Result<String> {
        let email = sqlx::query_scalar!("SELECT email FROM users WHERE id = ?", user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "unknown".to_string());
        Ok(email)
    }

    // --- Helper functions ---

    /// Find user by ID (partial match) or email
    async fn find_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        // Try exact email match first
        if identifier.contains('@') {
            if let Ok(user) = UserRepository::get_by_email(pool, identifier).await {
                return Ok(user);
            }
        }

        // An exact id wins, even if it's also the start of another one
        if let Some(user) = sqlx::query_as!(User, "SELECT * FROM users WHERE id = ?", identifier)
            .fetch_optional(pool)
            .await?
        {
            return Ok(user);
        }

        let not_found = || AdminError::UserNotFound(identifier.to_string());
        let pattern = id_prefix_pattern(identifier).ok_or_else(not_found)?;

        // Two rows are enough to know it's ambiguous
        let mut matches = sqlx::query_as!(
            User,
            r#"SELECT * FROM users WHERE id LIKE ? ESCAPE '\' LIMIT 2"#,
            pattern
        )
        .fetch_all(pool)
        .await?;

        match matches.len() {
            0 => Err(not_found()),
            1 => Ok(matches.remove(0)),
            _ => Err(AdminError::AmbiguousId(identifier.to_string())),
        }
    }

    /// Find device by ID (exact or prefix match)
    async fn find_device(pool: &SqlitePool, device_id: &str) -> Result<DeviceInfo> {
        #[derive(sqlx::FromRow)]
        struct DeviceRow {
            id: String,
            user_id: String,
            device_name: String,
            device_type: String,
            created_at: String,
            last_seen_at: Option<String>,
            is_active: Option<i64>,
        }

        let exact = sqlx::query_as!(
            DeviceRow,
            "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients WHERE id = ?",
            device_id
        )
        .fetch_optional(pool)
        .await?;

        let row = match exact {
            Some(row) => row,
            None => {
                let not_found = || AdminError::DeviceNotFound(device_id.to_string());
                let pattern = id_prefix_pattern(device_id).ok_or_else(not_found)?;

                let mut matches = sqlx::query_as!(
                    DeviceRow,
                    r#"SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active
                       FROM clients WHERE id LIKE ? ESCAPE '\' LIMIT 2"#,
                    pattern
                )
                .fetch_all(pool)
                .await?;

                match matches.len() {
                    0 => return Err(not_found()),
                    1 => matches.remove(0),
                    _ => return Err(AdminError::AmbiguousId(device_id.to_string())),
                }
            }
        };

        Ok(DeviceInfo {
            id: row.id,
            user_id: row.user_id,
            device_name: row.device_name,
            device_type: row.device_type,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
            is_active: row.is_active.unwrap_or(0) == 1,
        })
    }

    /// Get all devices for a user
    async fn get_devices_for_user(pool: &SqlitePool, user_id: &str) -> Result<Vec<DeviceInfo>> {
        #[derive(sqlx::FromRow)]
        struct DeviceRow {
            id: String,
            user_id: String,
            device_name: String,
            device_type: String,
            created_at: String,
            last_seen_at: Option<String>,
            is_active: Option<i64>,
        }

        let rows = sqlx::query_as!(
            DeviceRow,
            "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients WHERE user_id = ? ORDER BY created_at DESC",
            user_id
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DeviceInfo {
                id: r.id,
                user_id: r.user_id,
                device_name: r.device_name,
                device_type: r.device_type,
                created_at: r.created_at,
                last_seen_at: r.last_seen_at,
                is_active: r.is_active.unwrap_or(0) == 1,
            })
            .collect())
    }

    /// Get all devices
    async fn get_all_devices(pool: &SqlitePool) -> Result<Vec<DeviceInfo>> {
        #[derive(sqlx::FromRow)]
        struct DeviceRow {
            id: String,
            user_id: String,
            device_name: String,
            device_type: String,
            created_at: String,
            last_seen_at: Option<String>,
            is_active: Option<i64>,
        }

        let rows = sqlx::query_as!(
            DeviceRow,
            "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients ORDER BY created_at DESC"
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DeviceInfo {
                id: r.id,
                user_id: r.user_id,
                device_name: r.device_name,
                device_type: r.device_type,
                created_at: r.created_at,
                last_seen_at: r.last_seen_at,
                is_active: r.is_active.unwrap_or(0) == 1,
            })
            .collect())
    }
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
