//! Admin service - business logic for server administration
//!
//! This module provides the core business logic for administrative operations
//! that can be used by both CLI tools and web APIs.

use crate::db::UserRepository;
use crate::models::User;
use crate::utils::password::hash_password_with_params;
use sqlx::SqlitePool;

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

    #[error("Cannot modify admin user")]
    CannotModifyAdmin,

    #[error("User is already {0}")]
    AlreadyInState(String),

    #[error("Password must be at least 12 characters")]
    PasswordTooShort,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Password hashing failed: {0}")]
    PasswordHash(String),
}

pub type Result<T> = std::result::Result<T, AdminError>;

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

    /// Approve a pending user
    pub async fn approve_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;

        if user.approved == 1 {
            return Err(AdminError::AlreadyInState("approved".to_string()));
        }

        UserRepository::approve(pool, &user.id, "admin").await?;

        // Return updated user
        Self::find_user(pool, &user.id).await
    }

    /// Deactivate a user account
    pub async fn deactivate_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;

        if user.is_active == 0 {
            return Err(AdminError::AlreadyInState("inactive".to_string()));
        }

        if user.is_admin == 1 {
            return Err(AdminError::CannotModifyAdmin);
        }

        UserRepository::deactivate(pool, &user.id).await?;

        // Return updated user
        Self::find_user(pool, &user.id).await
    }

    /// Activate a user account
    pub async fn activate_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;

        if user.is_active == 1 {
            return Err(AdminError::AlreadyInState("active".to_string()));
        }

        UserRepository::activate(pool, &user.id).await?;

        // Return updated user
        Self::find_user(pool, &user.id).await
    }

    /// Delete a user and all their data
    pub async fn delete_user(pool: &SqlitePool, identifier: &str) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;

        if user.is_admin == 1 {
            return Err(AdminError::CannotModifyAdmin);
        }

        // Delete user's notes
        sqlx::query!("DELETE FROM notes WHERE user_id = ?", user.id)
            .execute(pool)
            .await?;

        // Delete user's devices
        sqlx::query!("DELETE FROM clients WHERE user_id = ?", user.id)
            .execute(pool)
            .await?;

        // Delete user
        UserRepository::delete(pool, &user.id).await?;

        Ok(user)
    }

    /// Reset a user's password
    pub async fn reset_password(
        pool: &SqlitePool,
        identifier: &str,
        new_password: &str,
        argon2_params: (u32, u32, u32), // (m_cost, t_cost, p_cost)
    ) -> Result<User> {
        let user = Self::find_user(pool, identifier).await?;

        if new_password.len() < 12 {
            return Err(AdminError::PasswordTooShort);
        }

        let (m_cost, t_cost, p_cost) = argon2_params;
        let password_hash = hash_password_with_params(new_password, m_cost, t_cost, p_cost)
            .map_err(|e| AdminError::PasswordHash(e.to_string()))?;

        sqlx::query!(
            "UPDATE users SET password_hash = ? WHERE id = ?",
            password_hash,
            user.id
        )
        .execute(pool)
        .await?;

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

        Ok(device)
    }

    /// Revoke (deactivate) a device
    pub async fn revoke_device(pool: &SqlitePool, device_id: &str) -> Result<DeviceInfo> {
        let device = Self::find_device(pool, device_id).await?;

        sqlx::query!("UPDATE clients SET is_active = 0 WHERE id = ?", device.id)
            .execute(pool)
            .await?;

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

        // Try ID match (exact or prefix)
        let id_prefix = format!("{}%", identifier);
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = ? OR id LIKE ?",
            identifier,
            id_prefix
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AdminError::UserNotFound(identifier.to_string()))?;

        Ok(user)
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

        let id_prefix = format!("{}%", device_id);
        let row = sqlx::query_as!(
            DeviceRow,
            "SELECT id, user_id, device_name, device_type, created_at, last_seen_at, is_active FROM clients WHERE id = ? OR id LIKE ?",
            device_id,
            id_prefix
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AdminError::DeviceNotFound(device_id.to_string()))?;

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
