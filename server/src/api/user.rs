use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    db::{SessionRepository, UserRepository},
    error::{AppError, AppResult},
    models::{CreateSessionParams, Session},
    utils::password::{hash_password_with_params, verify_password},
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

    // Calculate expiry (configurable, default 7 days)
    let expires_at = chrono::Utc::now() + chrono::Duration::days(state.config.session_expiry_days);
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
    let last_sync: Option<String> = sqlx::query_scalar(
        "SELECT MAX(last_seen_at) FROM clients WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    Ok(Json(UserAccountInfo {
        email: user.email,
        note_count: note_count as i64,
        attachment_count: attachment_count as i64,
        storage_used_bytes: storage_used as i64,
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

/// Change password request
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Change user's own password
/// POST /api/v1/user/change-password
pub async fn change_password(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Json(req): Json<ChangePasswordRequest>,
) -> AppResult<StatusCode> {
    let user_id = &session.user_id;

    // Get current user
    let user = UserRepository::get_by_id(&state.pool, user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get user: {}", e);
            AppError::Unauthorized
        })?;

    // Verify current password
    let password_valid = verify_password(&req.current_password, &user.password_hash)
        .map_err(|e| {
            tracing::error!("Password verification failed: {}", e);
            AppError::InternalServerError
        })?;

    if !password_valid {
        tracing::warn!("Password change failed: invalid current password for user {}", user_id);
        return Err(AppError::Unauthorized);
    }

    // Validate new password strength
    if req.new_password.len() < 12 {
        return Err(AppError::BadRequest(
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
        AppError::InternalServerError
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

/// Delete account query parameters
#[derive(Debug, Deserialize)]
pub struct DeleteAccountQuery {
    pub mode: String, // "deactivate" or "delete"
}

/// Delete or deactivate user's own account
/// DELETE /api/v1/user/account?mode=deactivate|delete
pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Query(query): Query<DeleteAccountQuery>,
) -> AppResult<StatusCode> {
    let user_id = &session.user_id;

    match query.mode.as_str() {
        "deactivate" => {
            // Soft delete: set is_active = 0, allowing re-registration with admin approval
            sqlx::query!(
                r#"UPDATE users SET is_active = 0 WHERE id = ?"#,
                user_id
            )
            .execute(&state.pool)
            .await?;

            // Invalidate all sessions for this user
            sqlx::query!(
                r#"DELETE FROM sessions WHERE user_id = ?"#,
                user_id
            )
            .execute(&state.pool)
            .await?;

            tracing::info!("User deactivated their account: {}", user_id);
            Ok(StatusCode::NO_CONTENT)
        }
        "delete" => {
            // Hard delete: remove all user data
            // Due to CASCADE constraints, this will delete:
            // - sessions
            // - clients (devices)
            // - notes (which cascades to note_versions, attachments_meta, attachments_data)

            let mut tx = state.pool.begin().await.map_err(|e| {
                tracing::error!("Failed to start transaction: {}", e);
                AppError::InternalServerError
            })?;

            // Delete note versions first (may not have CASCADE set up)
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
            sqlx::query!("DELETE FROM notes WHERE user_id = ?", user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to delete notes: {}", e);
                    AppError::InternalServerError
                })?;

            // Delete sessions
            sqlx::query!("DELETE FROM sessions WHERE user_id = ?", user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to delete sessions: {}", e);
                    AppError::InternalServerError
                })?;

            // Delete clients (devices)
            sqlx::query!("DELETE FROM clients WHERE user_id = ?", user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to delete clients: {}", e);
                    AppError::InternalServerError
                })?;

            // Finally delete the user
            sqlx::query!("DELETE FROM users WHERE id = ?", user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to delete user: {}", e);
                    AppError::InternalServerError
                })?;

            tx.commit().await.map_err(|e| {
                tracing::error!("Failed to commit transaction: {}", e);
                AppError::InternalServerError
            })?;

            tracing::warn!("User permanently deleted their account: {}", user_id);
            Ok(StatusCode::NO_CONTENT)
        }
        _ => {
            Err(AppError::BadRequest(
                "Invalid mode. Use 'deactivate' or 'delete'".to_string(),
            ))
        }
    }
}

/// Logout - invalidate current session
/// POST /api/v1/user/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<StatusCode> {
    // Delete the current session
    sqlx::query!(
        r#"DELETE FROM sessions WHERE id = ?"#,
        session.id
    )
    .execute(&state.pool)
    .await?;

    tracing::info!("User logged out: session_id={}", session.id);
    Ok(StatusCode::NO_CONTENT)
}
