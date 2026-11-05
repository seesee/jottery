// Admin user management endpoints

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Serialize;
use std::sync::Arc;

use crate::{
    api::cookies::{clear_session_cookie, CookieScope},
    audit::{self, AuditActor, AuditEvent},
    db::{SessionRepository, UserRepository},
    error::{AppError, AppResult},
    models::{Session, User, UserInfo},
    services::{AdminError, AdminService},
    utils::password::{hash_for_storage, validate_password_strength, verify_stored},
    AppState,
};

/// Who is signed in. A cookie session can't reach the user routes, so this is how
/// the dashboard learns that, and that its session still works.
/// GET /api/v1/admin/me
pub async fn me(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<Json<UserInfo>> {
    let user = UserRepository::get_by_id(&state.pool, &session.user_id).await?;

    Ok(Json(UserInfo {
        id: user.id,
        email: user.email,
        is_admin: user.is_admin == 1,
    }))
}

/// End the current session.
/// POST /api/v1/admin/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<(StatusCode, HeaderMap)> {
    SessionRepository::delete(&state.pool, &session.id).await?;

    tracing::info!("Admin logged out: session_id={}", session.id);
    Ok((StatusCode::NO_CONTENT, clear_session_cookie(CookieScope::Admin)))
}

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
        r#"SELECT id, email, approved, is_admin, is_active, created_at, approved_at, last_login_at, storage_quota_mb, max_upload_size_mb, inbox_max_items, inbox_max_size_mb, inbox_token_hash FROM users WHERE id = ?"#,
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
            COUNT(*) as count,
            COALESCE(SUM(am.size), 0) as total_bytes
        FROM attachments_meta am
        WHERE am.user_id = ?1 AND EXISTS (
            SELECT 1 FROM note_attachments na
            JOIN notes n ON n.id = na.note_id AND n.user_id = na.user_id
            WHERE na.user_id = ?1 AND na.attachment_id = am.attachment_id AND n.deleted = 0
        )
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

    // Get inbox usage
    let inbox_usage = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as count,
            COALESCE(SUM(size_bytes), 0) as total_bytes
        FROM inbox_items
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
            "maxUploadSizeMb": user.max_upload_size_mb,
            "inboxMaxItems": user.inbox_max_items,
            "inboxMaxSizeMb": user.inbox_max_size_mb,
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
                "inbox": {
                    "count": inbox_usage.count,
                    "totalBytes": inbox_usage.total_bytes,
                    "hasToken": user.inbox_token_hash.is_some()
                },
                "lastSyncAt": last_sync.last_sync
            },
            "devices": device_list
        })),
    ))
}

/// Look up the user a route names. Ids are exact here: only the command line
/// accepts a prefix.
async fn target_user(state: &AppState, user_id: &str) -> AppResult<User> {
    UserRepository::get_by_id(&state.pool, user_id)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => AppError::NotFound("User not found".to_string()),
            e => e.into(),
        })
}

/// What a dashboard asks for may already be true, which isn't an error to the
/// person clicking the button. Returns whether anything changed.
fn tolerate_no_change(result: Result<(), AdminError>) -> AppResult<bool> {
    match result {
        Ok(()) => Ok(true),
        Err(AdminError::AlreadyInState(_)) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Approve pending user
/// POST /api/v1/admin/users/:id/approve
pub async fn approve_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(current_user_id): axum::Extension<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
) -> AppResult<StatusCode> {
    let user = target_user(&state, &user_id).await?;
    if tolerate_no_change(AdminService::approve(&state.pool, &user, Some(&current_user_id)).await)? {
        audit::record(&state.pool, &actor, AuditEvent::on_user(audit::USER_APPROVE, &user_id)).await;
    }

    tracing::info!("User approved: user_id={}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Deactivate user
/// POST /api/v1/admin/users/:id/deactivate
pub async fn deactivate_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
) -> AppResult<StatusCode> {
    let user = target_user(&state, &user_id).await?;
    if tolerate_no_change(AdminService::deactivate(&state.pool, &user).await)? {
        audit::record(&state.pool, &actor, AuditEvent::on_user(audit::USER_DEACTIVATE, &user_id)).await;
    }

    tracing::info!("User deactivated: user_id={}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Reactivate user
/// POST /api/v1/admin/users/:id/activate
pub async fn activate_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
) -> AppResult<StatusCode> {
    let user = target_user(&state, &user_id).await?;
    if tolerate_no_change(AdminService::activate(&state.pool, &user).await)? {
        audit::record(&state.pool, &actor, AuditEvent::on_user(audit::USER_ACTIVATE, &user_id)).await;
    }

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
    axum::Extension(actor): axum::Extension<AuditActor>,
    Json(req): Json<ToggleAdminRequest>,
) -> AppResult<StatusCode> {
    // Prevent demoting yourself
    if user_id == current_user_id && !req.is_admin {
        return Err(crate::error::AppError::BadRequest(
            "Cannot remove your own admin privileges".to_string(),
        ));
    }

    let user = target_user(&state, &user_id).await?;

    let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;

    if !req.is_admin {
        AdminService::ensure_not_last_admin(&mut tx, &user).await?;
    }

    // Update admin status
    let is_admin_value = if req.is_admin { 1 } else { 0 };
    sqlx::query!(
        r#"UPDATE users SET is_admin = ? WHERE id = ?"#,
        is_admin_value,
        user_id
    )
    .execute(&mut *tx)
    .await?;

    // A session keeps no record of the role it was issued for, so a token captured
    // before a promotion would otherwise become an admin token
    let role_changed = (user.is_admin == 1) != req.is_admin;
    if role_changed {
        SessionRepository::delete_all_for_user(&mut tx, &user_id).await?;
    }

    tx.commit().await?;

    if role_changed {
        let action = if req.is_admin { audit::USER_ADMIN_GRANT } else { audit::USER_ADMIN_REVOKE };
        audit::record(&state.pool, &actor, AuditEvent::on_user(action, &user_id)).await;
    }

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
    axum::Extension(actor): axum::Extension<AuditActor>,
) -> AppResult<StatusCode> {
    // Prevent deleting yourself
    if user_id == current_user_id {
        return Err(crate::error::AppError::BadRequest(
            "Cannot delete your own account".to_string(),
        ));
    }

    let user = target_user(&state, &user_id).await?;
    AdminService::delete(&state.pool, &user).await?;
    audit::record(&state.pool, &actor, AuditEvent::on_user(audit::USER_DELETE, &user_id)).await;

    tracing::info!("User deleted: user_id={} by admin={}", user_id, current_user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// List user's devices
/// GET /api/v1/admin/users/:id/devices
pub async fn list_user_devices(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
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
            "devices": device_list
        })),
    ))
}

/// Revoke device API key (deactivate)
/// DELETE /api/v1/admin/devices/:id
pub async fn revoke_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
) -> AppResult<StatusCode> {
    // Check device exists
    let device = sqlx::query!(
        "SELECT id, user_id, is_active FROM clients WHERE id = ?",
        device_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("Device not found".to_string()))?;

    let is_active = device.is_active.unwrap_or(0) == 1;

    if is_active {
        // Deactivate the device (soft revoke - keeps record for audit)
        sqlx::query!(
            "UPDATE clients SET is_active = 0 WHERE id = ?",
            device.id
        )
        .execute(&state.pool)
        .await?;

        tracing::info!("Admin revoked device: {}", device_id);
        let event = AuditEvent {
            target_user_id: Some(&device.user_id),
            ..AuditEvent::on_device(audit::DEVICE_REVOKE, &device_id)
        };
        audit::record(&state.pool, &actor, event).await;
    } else {
        // Already revoked — hard-delete the device
        sqlx::query!(
            "DELETE FROM clients WHERE id = ?",
            device.id
        )
        .execute(&state.pool)
        .await?;

        tracing::info!("Admin deleted revoked device: {}", device_id);
        let event = AuditEvent {
            target_user_id: Some(&device.user_id),
            ..AuditEvent::on_device(audit::DEVICE_DELETE, &device_id)
        };
        audit::record(&state.pool, &actor, event).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Rename device request
#[derive(Debug, serde::Deserialize)]
pub struct RenameDeviceRequest {
    pub name: String,
}

/// Rename a device
/// PATCH /api/v1/admin/devices/:id
pub async fn rename_device(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
    Json(req): Json<RenameDeviceRequest>,
) -> AppResult<StatusCode> {
    // Check device exists
    let device = sqlx::query!(
        "SELECT id, user_id FROM clients WHERE id = ?",
        device_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("Device not found".to_string()))?;

    // Validate name
    if req.name.trim().is_empty() {
        return Err(crate::error::AppError::BadRequest("Device name cannot be empty".to_string()));
    }

    // Update the device name
    sqlx::query!(
        "UPDATE clients SET device_name = ? WHERE id = ?",
        req.name,
        device.id
    )
    .execute(&state.pool)
    .await?;

    let event = AuditEvent {
        target_user_id: Some(&device.user_id),
        ..AuditEvent::on_device(audit::DEVICE_RENAME, &device_id)
    };
    audit::record(&state.pool, &actor, event).await;

    tracing::info!("Admin renamed device {}: {:?}", device_id, req.name);
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
    axum::Extension(session): axum::Extension<Session>,
    axum::Extension(actor): axum::Extension<AuditActor>,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<StatusCode> {
    let user_id = &session.user_id;

    // Get current user
    let user = UserRepository::get_by_id(&state.pool, user_id)
        .await
        .map_err(|_| crate::error::AppError::Unauthorized)?;

    // Verify current password
    let password_valid = verify_stored(&req.current_password, &user.password_hash)
        .await
        .map_err(|e| {
            tracing::error!("Password verification failed: {}", e);
            crate::error::AppError::InternalServerError
        })?;

    if !password_valid {
        tracing::warn!("Password change failed: invalid current password for user {}", user_id);
        return Err(crate::error::AppError::Unauthorized);
    }

    // Validate new password strength based on configured complexity
    validate_password_strength(&req.new_password, &state.config.password_complexity)
        .map_err(crate::error::AppError::BadRequest)?;

    // "Changing" to the same password would satisfy a forced change
    if verify_stored(&req.new_password, &user.password_hash).await.unwrap_or(false) {
        return Err(crate::error::AppError::BadRequest(
            "New password must be different from the current one".to_string(),
        ));
    }

    // Hash new password with configured Argon2 parameters
    let new_password_hash = hash_for_storage(&state.config, &req.new_password)
        .await
        .map_err(|e| {
            tracing::error!("Password hashing failed: {}", e);
            crate::error::AppError::InternalServerError
        })?;

    let mut tx = state.pool.begin().await?;
    UserRepository::set_password(&mut tx, user_id, &new_password_hash).await?;
    SessionRepository::delete_others_for_user(&mut tx, user_id, &session.id).await?;
    tx.commit().await?;

    audit::record(&state.pool, &actor, AuditEvent::on_user(audit::ADMIN_PASSWORD_CHANGE, user_id)).await;

    tracing::info!("Password changed successfully for user: {}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Reset a user's password (admin action)
/// POST /api/v1/admin/users/:id/reset-password
#[derive(Debug, serde::Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
    /// Also revoke the user's device keys, for an account that may be compromised.
    /// A key isn't tied to the password, so a reset alone leaves them working.
    #[serde(rename = "revokeDevices", default)]
    pub revoke_devices: bool,
}

pub async fn reset_user_password(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
    Json(req): Json<ResetPasswordRequest>,
) -> AppResult<StatusCode> {
    // Check user exists
    let _user = sqlx::query!(
        "SELECT id FROM users WHERE id = ?",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("User not found".to_string()))?;

    // Validate new password strength based on configured complexity
    validate_password_strength(&req.new_password, &state.config.password_complexity)
        .map_err(crate::error::AppError::BadRequest)?;

    // Hash new password with configured Argon2 parameters
    let new_password_hash = hash_for_storage(&state.config, &req.new_password)
        .await
        .map_err(|e| {
            tracing::error!("Password hashing failed: {}", e);
            crate::error::AppError::InternalServerError
        })?;

    let mut tx = state.pool.begin().await?;
    UserRepository::set_password(&mut tx, &user_id, &new_password_hash).await?;
    SessionRepository::delete_all_for_user(&mut tx, &user_id).await?;
    if req.revoke_devices {
        sqlx::query!("UPDATE clients SET is_active = 0 WHERE user_id = ?", user_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    let event = AuditEvent::on_user(audit::USER_PASSWORD_RESET, &user_id)
        .with_metadata(serde_json::json!({ "revokedDevices": req.revoke_devices }));
    audit::record(&state.pool, &actor, event).await;

    tracing::info!("Admin reset password for user: {}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Update user settings (storage quota, max upload size)
/// PATCH /api/v1/admin/users/:id/settings
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserSettingsRequest {
    pub storage_quota_mb: Option<i64>,
    pub max_upload_size_mb: Option<i64>,
    pub inbox_max_items: Option<i64>,
    pub inbox_max_size_mb: Option<i64>,
}

pub async fn update_user_settings(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    axum::Extension(actor): axum::Extension<AuditActor>,
    Json(req): Json<UpdateUserSettingsRequest>,
) -> AppResult<StatusCode> {
    // Check user exists
    let _user = sqlx::query!(
        "SELECT id FROM users WHERE id = ?",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("User not found".to_string()))?;

    // Validate values
    if let Some(max_upload) = req.max_upload_size_mb {
        if max_upload < 1 || max_upload > 1000 {
            return Err(crate::error::AppError::BadRequest(
                "Max upload size must be between 1 and 1000 MB".to_string(),
            ));
        }
    }

    if let Some(quota) = req.storage_quota_mb {
        if quota < 1 || quota > 100000 {
            return Err(crate::error::AppError::BadRequest(
                "Storage quota must be between 1 and 100000 MB".to_string(),
            ));
        }
    }

    // Validate inbox quota values
    if let Some(max_items) = req.inbox_max_items {
        if max_items < 1 || max_items > 10000 {
            return Err(crate::error::AppError::BadRequest(
                "Inbox max items must be between 1 and 10000".to_string(),
            ));
        }
    }

    if let Some(max_size) = req.inbox_max_size_mb {
        if max_size < 1 || max_size > 1000 {
            return Err(crate::error::AppError::BadRequest(
                "Inbox max size must be between 1 and 1000 MB".to_string(),
            ));
        }
    }

    // Update settings
    if let Some(max_upload) = req.max_upload_size_mb {
        sqlx::query!(
            "UPDATE users SET max_upload_size_mb = ? WHERE id = ?",
            max_upload,
            user_id
        )
        .execute(&state.pool)
        .await?;
        tracing::info!("Updated max_upload_size_mb to {} for user {}", max_upload, user_id);
    }

    if let Some(quota) = req.storage_quota_mb {
        sqlx::query!(
            "UPDATE users SET storage_quota_mb = ? WHERE id = ?",
            quota,
            user_id
        )
        .execute(&state.pool)
        .await?;
        tracing::info!("Updated storage_quota_mb to {} for user {}", quota, user_id);
    }

    if let Some(max_items) = req.inbox_max_items {
        sqlx::query!(
            "UPDATE users SET inbox_max_items = ? WHERE id = ?",
            max_items,
            user_id
        )
        .execute(&state.pool)
        .await?;
        tracing::info!("Updated inbox_max_items to {} for user {}", max_items, user_id);
    }

    if let Some(max_size) = req.inbox_max_size_mb {
        sqlx::query!(
            "UPDATE users SET inbox_max_size_mb = ? WHERE id = ?",
            max_size,
            user_id
        )
        .execute(&state.pool)
        .await?;
        tracing::info!("Updated inbox_max_size_mb to {} for user {}", max_size, user_id);
    }

    let event = AuditEvent::on_user(audit::USER_SETTINGS_UPDATE, &user_id).with_metadata(serde_json::json!({
        "storageQuotaMb": req.storage_quota_mb,
        "maxUploadSizeMb": req.max_upload_size_mb,
        "inboxMaxItems": req.inbox_max_items,
        "inboxMaxSizeMb": req.inbox_max_size_mb,
    }));
    audit::record(&state.pool, &actor, event).await;

    Ok(StatusCode::NO_CONTENT)
}
