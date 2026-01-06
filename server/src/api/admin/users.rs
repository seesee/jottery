// Admin user management endpoints

use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::{error::AppResult, AppState};

#[derive(Debug, Serialize)]
pub struct UserListItem {
    pub id: String,
    pub email: String,
    pub approved: bool,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "deviceCount")]
    pub device_count: i64,
    #[serde(rename = "noteCount")]
    pub note_count: i64,
}

/// List all users (paginated)
/// GET /api/v1/admin/users
pub async fn list_users(
    State(state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let users = sqlx::query!(
        r#"
        SELECT
            u.id,
            u.email,
            u.approved,
            u.is_admin,
            u.is_active,
            u.created_at,
            COUNT(DISTINCT c.id) as device_count,
            COUNT(DISTINCT n.id) as note_count
        FROM users u
        LEFT JOIN clients c ON u.id = c.user_id AND c.is_active = 1
        LEFT JOIN notes n ON u.id = n.user_id AND n.deleted = 0
        GROUP BY u.id
        ORDER BY u.created_at DESC
        "#
    )
    .fetch_all(&state.pool)
    .await?;

    let user_list: Vec<UserListItem> = users
        .into_iter()
        .map(|u| UserListItem {
            id: u.id,
            email: u.email,
            approved: u.approved == 1,
            is_admin: u.is_admin == 1,
            is_active: u.is_active == 1,
            created_at: u.created_at,
            device_count: u.device_count as i64,
            note_count: u.note_count as i64,
        })
        .collect();

    let total = user_list.len();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "users": user_list,
            "total": total
        })),
    ))
}

/// Get user details with statistics
/// GET /api/v1/admin/users/:id
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // Get user basic info
    let user = sqlx::query!(
        r#"SELECT id, email, approved, is_admin, is_active, created_at, approved_at, last_login_at, storage_quota_mb FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("User not found".to_string()))?;

    // Get device count and last seen
    let device_stats = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(CASE WHEN is_active = 1 THEN 1 END) as active,
            MAX(last_seen_at) as "last_seen: Option<String>"
        FROM clients
        WHERE user_id = ?
        "#,
        user_id
    )
    .fetch_one(&state.pool)
    .await?;

    // Get note count
    let note_count = sqlx::query!(
        r#"SELECT COUNT(*) as count FROM notes WHERE user_id = ? AND deleted = 0"#,
        user_id
    )
    .fetch_one(&state.pool)
    .await?;

    // Get attachment count and total size
    let attachment_stats = sqlx::query!(
        r#"
        SELECT
            COUNT(DISTINCT am.id) as count,
            COALESCE(SUM(am.size), 0) as total_bytes
        FROM notes n
        JOIN attachments_meta am ON n.id = am.note_id
        WHERE n.user_id = ? AND n.deleted = 0
        "#,
        user_id
    )
    .fetch_one(&state.pool)
    .await?;

    // Get last sync time (when server last received an update)
    let last_sync = sqlx::query!(
        r#"
        SELECT MAX(server_modified_at) as "last_sync: Option<String>"
        FROM notes
        WHERE user_id = ?
        "#,
        user_id
    )
    .fetch_one(&state.pool)
    .await?;

    // Get list of devices
    let devices = sqlx::query!(
        r#"
        SELECT id, device_name, device_type, created_at, last_seen_at, is_active
        FROM clients
        WHERE user_id = ?
        ORDER BY created_at DESC
        "#,
        user_id
    )
    .fetch_all(&state.pool)
    .await?;

    let device_list: Vec<serde_json::Value> = devices
        .into_iter()
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "name": d.device_name,
                "type": d.device_type,
                "createdAt": d.created_at,
                "lastSeenAt": d.last_seen_at,
                "isActive": d.is_active.unwrap_or(0) == 1
            })
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "id": user.id,
            "email": user.email,
            "approved": user.approved == 1,
            "isAdmin": user.is_admin == 1,
            "isActive": user.is_active == 1,
            "createdAt": user.created_at,
            "approvedAt": user.approved_at,
            "lastLoginAt": user.last_login_at,
            "storageQuotaMb": user.storage_quota_mb,
            "stats": {
                "devices": {
                    "total": device_stats.total,
                    "active": device_stats.active,
                    "lastSeen": device_stats.last_seen
                },
                "notes": {
                    "count": note_count.count
                },
                "attachments": {
                    "count": attachment_stats.count,
                    "totalBytes": attachment_stats.total_bytes
                },
                "lastSyncAt": last_sync.last_sync
            },
            "devices": device_list
        })),
    ))
}

/// Approve pending user
/// POST /api/v1/admin/users/:id/approve
pub async fn approve_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> AppResult<StatusCode> {
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query!(
        r#"UPDATE users SET approved = 1, approved_at = ? WHERE id = ?"#,
        now,
        user_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("User approved: user_id={}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Deactivate user
/// POST /api/v1/admin/users/:id/deactivate
pub async fn deactivate_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> AppResult<StatusCode> {
    sqlx::query!(
        r#"UPDATE users SET is_active = 0 WHERE id = ?"#,
        user_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("User deactivated: user_id={}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Reactivate user
/// POST /api/v1/admin/users/:id/activate
pub async fn activate_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> AppResult<StatusCode> {
    sqlx::query!(
        r#"UPDATE users SET is_active = 1 WHERE id = ?"#,
        user_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("User activated: user_id={}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Toggle admin status
/// POST /api/v1/admin/users/:id/toggle-admin
#[derive(Debug, serde::Deserialize)]
pub struct ToggleAdminRequest {
    pub is_admin: bool,
}

pub async fn toggle_admin(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(current_user_id): axum::Extension<String>,
    Json(req): Json<ToggleAdminRequest>,
) -> AppResult<StatusCode> {
    // Prevent demoting yourself
    if user_id == current_user_id && !req.is_admin {
        return Err(crate::error::AppError::BadRequest(
            "Cannot remove your own admin privileges".to_string(),
        ));
    }

    // If demoting an admin, check if there are other admins
    if !req.is_admin {
        let target_user = sqlx::query!(
            r#"SELECT is_admin FROM users WHERE id = ?"#,
            user_id
        )
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound("User not found".to_string()))?;

        // Only check if currently an admin
        if target_user.is_admin == 1 {
            let admin_count = sqlx::query!(
                r#"SELECT COUNT(*) as count FROM users WHERE is_admin = 1 AND is_active = 1"#
            )
            .fetch_one(&state.pool)
            .await?;

            if admin_count.count <= 1 {
                return Err(crate::error::AppError::BadRequest(
                    "Cannot demote the last admin user".to_string(),
                ));
            }
        }
    }

    // Update admin status
    let is_admin_value = if req.is_admin { 1 } else { 0 };
    sqlx::query!(
        r#"UPDATE users SET is_admin = ? WHERE id = ?"#,
        is_admin_value,
        user_id
    )
    .execute(&state.pool)
    .await?;

    let action = if req.is_admin { "promoted to admin" } else { "demoted from admin" };
    tracing::info!("User {}: user_id={} by admin={}", action, user_id, current_user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Delete user
/// DELETE /api/v1/admin/users/:id
pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(current_user_id): axum::Extension<String>,
) -> AppResult<StatusCode> {
    // Prevent deleting yourself
    if user_id == current_user_id {
        return Err(crate::error::AppError::BadRequest(
            "Cannot delete your own account".to_string(),
        ));
    }

    // Check if target user is an admin
    let target_user = sqlx::query!(
        r#"SELECT is_admin FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("User not found".to_string()))?;

    // If deleting an admin, check if there are other admins
    if target_user.is_admin == 1 {
        let admin_count = sqlx::query!(
            r#"SELECT COUNT(*) as count FROM users WHERE is_admin = 1 AND is_active = 1"#
        )
        .fetch_one(&state.pool)
        .await?;

        if admin_count.count <= 1 {
            return Err(crate::error::AppError::BadRequest(
                "Cannot delete the last admin user".to_string(),
            ));
        }
    }

    // Delete user (CASCADE will handle related records)
    sqlx::query!(
        r#"DELETE FROM users WHERE id = ?"#,
        user_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("User deleted: user_id={} by admin={}", user_id, current_user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// List user's devices
/// GET /api/v1/admin/users/:id/devices
pub async fn list_user_devices(
    State(_state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // TODO: Implement device listing
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "devices": []
        })),
    ))
}

/// Revoke device API key
/// DELETE /api/v1/admin/devices/:id
pub async fn revoke_device(
    State(_state): State<Arc<AppState>>,
) -> AppResult<StatusCode> {
    // TODO: Implement device revocation
    Ok(StatusCode::NO_CONTENT)
}

/// Change admin password
/// POST /api/v1/admin/change-password
#[derive(Debug, serde::Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    State(state): State<Arc<AppState>>,
    axum::Extension(user_id): axum::Extension<String>,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<StatusCode> {
    use crate::utils::password::{hash_password_with_params, verify_password};
    use crate::db::UserRepository;

    // Get current user
    let user = UserRepository::get_by_id(&state.pool, &user_id)
        .await
        .map_err(|_| crate::error::AppError::Unauthorized)?;

    // Verify current password
    let password_valid = verify_password(&req.current_password, &user.password_hash)
        .map_err(|e| {
            tracing::error!("Password verification failed: {}", e);
            crate::error::AppError::InternalServerError
        })?;

    if !password_valid {
        tracing::warn!("Password change failed: invalid current password for user {}", user_id);
        return Err(crate::error::AppError::Unauthorized);
    }

    // Validate new password strength
    if req.new_password.len() < 12 {
        return Err(crate::error::AppError::BadRequest(
            "New password must be at least 12 characters".to_string(),
        ));
    }

    // Hash new password with configured Argon2 parameters
    let new_password_hash = hash_password_with_params(
        &req.new_password,
        state.config.argon2_m_cost,
        state.config.argon2_t_cost,
        state.config.argon2_p_cost,
    )
    .map_err(|e| {
        tracing::error!("Password hashing failed: {}", e);
        crate::error::AppError::InternalServerError
    })?;

    // Update password
    sqlx::query!(
        r#"UPDATE users SET password_hash = ? WHERE id = ?"#,
        new_password_hash,
        user_id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("Password changed successfully for user: {}", user_id);
    Ok(StatusCode::NO_CONTENT)
}
