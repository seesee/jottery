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

/// Get user details
/// GET /api/v1/admin/users/:id
pub async fn get_user(
    State(_state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    // TODO: Implement user detail retrieval
    Ok((StatusCode::OK, Json(serde_json::json!({}))))
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

/// Delete user
/// DELETE /api/v1/admin/users/:id
pub async fn delete_user(
    State(_state): State<Arc<AppState>>,
) -> AppResult<StatusCode> {
    // TODO: Implement user deletion
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
