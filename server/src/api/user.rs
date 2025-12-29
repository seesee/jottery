use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    db::SessionRepository,
    error::{AppError, AppResult},
    models::{CreateSessionParams, Session},
    AppState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAccountInfo {
    pub email: String,
    pub note_count: i64,
    pub attachment_count: i64,
    pub storage_used_bytes: i64,
    pub storage_quota_mb: i64,
    pub created_at: String,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub session_id: String,
    pub expires_at: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub is_admin: bool,
}

/// User login (for account management, not admin dashboard)
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> AppResult<(StatusCode, Json<LoginResponse>)> {
    tracing::info!("User login request: email={}", req.email);

    // Get user by email
    let user = sqlx::query!(
        "SELECT id, email, password_hash, is_admin FROM users WHERE email = ?",
        req.email
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError
    })?
    .ok_or_else(|| {
        tracing::warn!("Login failed: user not found: {}", req.email);
        AppError::Unauthorized
    })?;

    // Verify password
    let password_valid = crate::utils::password::verify_password(&req.password, &user.password_hash)
        .map_err(|e| {
            tracing::error!("Password verification failed: {}", e);
            AppError::InternalServerError
        })?;

    if !password_valid {
        tracing::warn!("Login failed: invalid password for {}", req.email);
        return Err(AppError::Unauthorized);
    }

    // Update last login
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query!("UPDATE users SET last_login_at = ? WHERE id = ?", now, user.id)
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update last_login_at: {}", e);
            AppError::InternalServerError
        })?;

    // Generate session token
    let session_token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();

    // Calculate expiry (7 days from now)
    let expires_at = chrono::Utc::now() + chrono::Duration::days(7);
    let expires_at_str = expires_at.to_rfc3339();

    // Create session
    let _session = SessionRepository::create(
        &state.pool,
        CreateSessionParams {
            user_id: user.id.clone(),
            token: session_token.clone(),
            expires_at: expires_at_str.clone(),
            user_agent: None,
            ip_address: None,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create session: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!("User logged in successfully: {}", req.email);

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            session_id: session_token.clone(),  // Return the actual token, not the session ID
            expires_at: expires_at_str,
            user: UserInfo {
                id: user.id,
                email: user.email,
                is_admin: user.is_admin != 0,
            },
        }),
    ))
}

/// Get user's own account information
pub async fn get_account_info(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<Json<UserAccountInfo>> {
    let user_id = session.user_id;

    // Get user info
    let user = sqlx::query!(
        "SELECT email, storage_quota_mb, created_at FROM users WHERE id = ?",
        user_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalError("Database query failed".to_string())
    })?
    .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    // Count notes
    let note_count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM notes WHERE user_id = ? AND deleted = 0",
        user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to count notes: {}", e);
        AppError::InternalServerError
    })?;

    // Count attachments
    let attachment_count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM attachments_meta WHERE note_id IN (SELECT id FROM notes WHERE user_id = ? AND deleted = 0)",
        user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to count attachments: {}", e);
        AppError::InternalServerError
    })?;

    // Calculate storage used
    let storage_used = sqlx::query_scalar!(
        "SELECT COALESCE(SUM(size), 0) FROM attachments_meta
         WHERE note_id IN (SELECT id FROM notes WHERE user_id = ? AND deleted = 0)",
        user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to calculate storage: {}", e);
        AppError::InternalServerError
    })?;

    // Get last sync time
    let last_sync = sqlx::query_scalar!(
        "SELECT MAX(last_seen_at) FROM clients WHERE user_id = ?",
        user_id
    )
    .fetch_one(&state.pool)
    .await
    .ok()
    .flatten();

    Ok(Json(UserAccountInfo {
        email: user.email,
        note_count: note_count as i64,
        attachment_count: attachment_count.map(|c| c as i64).unwrap_or(0),
        storage_used_bytes: storage_used.map(|s| s as i64).unwrap_or(0),
        storage_quota_mb: user.storage_quota_mb.unwrap_or(1000) as i64,
        created_at: user.created_at,
        last_sync_at: last_sync,
    }))
}

/// Delete all user's notes from server
pub async fn delete_all_notes(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<StatusCode> {
    let user_id = session.user_id;

    tracing::warn!("User {} requested to delete all their notes", user_id);

    // Delete in transaction
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| {
            tracing::error!("Failed to start transaction: {}", e);
            AppError::InternalServerError
        })?;

    // Delete note versions
    sqlx::query!(
        "DELETE FROM note_versions WHERE note_id IN (SELECT id FROM notes WHERE user_id = ?)",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete note versions: {}", e);
        AppError::InternalServerError
    })?;

    // Delete attachment data
    sqlx::query!(
        "DELETE FROM attachments_data WHERE id IN
         (SELECT id FROM attachments_meta WHERE note_id IN
          (SELECT id FROM notes WHERE user_id = ?))",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete attachment data: {}", e);
        AppError::InternalServerError
    })?;

    // Delete attachment metadata
    sqlx::query!(
        "DELETE FROM attachments_meta WHERE note_id IN (SELECT id FROM notes WHERE user_id = ?)",
        user_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to delete attachment metadata: {}", e);
        AppError::InternalServerError
    })?;

    // Delete notes
    let result = sqlx::query!("DELETE FROM notes WHERE user_id = ?", user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete notes: {}", e);
            AppError::InternalServerError
        })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!(
        "Deleted {} notes for user {}",
        result.rows_affected(),
        user_id
    );

    Ok(StatusCode::NO_CONTENT)
}
