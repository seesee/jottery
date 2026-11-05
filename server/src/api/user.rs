use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    api::cookies::{clear_session_cookie, grant_session, CookieScope},
    db::{SessionRepository, UserRepository},
    error::{AppError, AppResult},
    models::{CreateSessionParams, Session},
    services::AdminService,
    utils::{
        crypto::{hash_sha256, log_id},
        password::{hash_for_storage, validate_password_strength, verify_sign_in, verify_stored},
    },
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
    pub inbox: InboxAccountInfo,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxAccountInfo {
    pub item_count: i64,
    pub total_size_bytes: i64,
    pub max_items: i64,
    pub max_size_mb: i64,
    pub has_token: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    /// Ask for the session as an HttpOnly cookie. See `api::cookies`.
    #[serde(default)]
    pub cookie_session: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    /// Absent when the session is in a cookie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Present, and true, only when the session is in a cookie.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cookie_session: bool,
    pub expires_at: String,
    pub user: UserInfo,
    /// The session can only change the password until this is cleared.
    pub must_change_password: bool,
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
) -> AppResult<(StatusCode, HeaderMap, Json<LoginResponse>)> {
    state.login_throttle.check(&req.email)?;

    // Get user by email
    let user = UserRepository::find_by_email(&state.pool, &req.email)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::InternalServerError
        })?;

    // Verify password
    let password_valid = verify_sign_in(
        &state.config,
        &req.password,
        user.as_ref().map(|user| user.password_hash.as_str()),
    )
    .await
    .map_err(|e| {
        tracing::error!("Password verification failed: {}", e);
        AppError::InternalServerError
    })?;

    let Some(user) = user.filter(|_| password_valid) else {
        tracing::warn!("Login failed: bad credentials for {}", log_id(&req.email));
        state.login_throttle.record_failure(&req.email);
        return Err(AppError::Unauthorized);
    };
    state.login_throttle.record_success(&req.email);

    // Registration is open, so a session must wait for an admin's approval.
    // These come after the password check so they reveal nothing to strangers.
    if user.approved == 0 {
        tracing::warn!("Login failed: user not approved: {}", user.id);
        return Err(AppError::Forbidden("Account pending admin approval".to_string()));
    }

    if user.is_active == 0 {
        tracing::warn!("Login failed: user deactivated: {}", user.id);
        return Err(AppError::Forbidden("Account deactivated".to_string()));
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

    // Calculate expiry (configurable, default 1 day)
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

    tracing::info!("User logged in successfully: {}", user.id);

    let grant = grant_session(&state.config, CookieScope::User, req.cookie_session, session_token);

    Ok((
        StatusCode::OK,
        grant.headers,
        Json(LoginResponse {
            session_id: grant.session_id, // The actual token, not the session ID
            cookie_session: grant.cookie_session,
            expires_at: expires_at_str,
            user: UserInfo {
                id: user.id,
                email: user.email,
                is_admin: user.is_admin != 0,
            },
            must_change_password: user.must_change_password != 0,
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
        "SELECT email, storage_quota_mb, created_at, inbox_token_hash, inbox_max_items, inbox_max_size_mb FROM users WHERE id = ?",
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
        "SELECT COUNT(*) FROM attachments_meta m
         WHERE m.user_id = ?1 AND EXISTS (
             SELECT 1 FROM note_attachments na
             JOIN notes n ON n.id = na.note_id AND n.user_id = na.user_id
             WHERE na.user_id = ?1 AND na.attachment_id = m.attachment_id AND n.deleted = 0
         )",
        user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to count attachments: {}", e);
        AppError::InternalServerError
    })?;

    // Calculate storage used
    let storage_used = UserRepository::measure_storage(&state.pool, &user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to calculate storage: {}", e);
            AppError::InternalServerError
        })?;

    // Get last sync time
    let last_sync: Option<String> = sqlx::query_scalar(
        "SELECT MAX(last_seen_at) FROM clients WHERE user_id = ?"
    )
    .bind(&user_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    // Get inbox stats
    let inbox_stats = sqlx::query!(
        "SELECT COUNT(*) as count, COALESCE(SUM(size_bytes), 0) as total_size FROM inbox_items WHERE user_id = ?",
        user_id
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query inbox stats: {}", e);
        AppError::InternalServerError
    })?;

    Ok(Json(UserAccountInfo {
        email: user.email,
        note_count: note_count as i64,
        attachment_count: attachment_count as i64,
        storage_used_bytes: storage_used,
        storage_quota_mb: user.storage_quota_mb.unwrap_or(state.config.default_storage_quota_mb) as i64,
        created_at: user.created_at,
        last_sync_at: last_sync,
        inbox: InboxAccountInfo {
            item_count: inbox_stats.count as i64,
            total_size_bytes: inbox_stats.total_size as i64,
            max_items: user.inbox_max_items.unwrap_or(state.config.default_inbox_max_items),
            max_size_mb: user.inbox_max_size_mb.unwrap_or(state.config.default_inbox_max_size_mb),
            has_token: user.inbox_token_hash.is_some(),
        },
    }))
}

/// How long a sign-in counts as proof of the password. Released iOS clients ask for
/// the password, sign in and delete the account at once, without sending it again.
const RECENT_SIGN_IN_MINUTES: i64 = 5;

/// The password, sent with a request that can't be undone.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordConfirmation {
    pub current_password: Option<String>,
}

fn signed_in_recently(session: &Session) -> bool {
    chrono::DateTime::parse_from_rfc3339(&session.created_at).is_ok_and(|created_at| {
        chrono::Utc::now() - created_at.with_timezone(&chrono::Utc)
            < chrono::Duration::minutes(RECENT_SIGN_IN_MINUTES)
    })
}

/// Check that the person behind a session really is the account holder, before
/// something that can't be undone. A session alone may be stolen, or left open.
async fn confirm_account_holder(
    state: &AppState,
    session: &Session,
    confirmation: Option<PasswordConfirmation>,
) -> AppResult<()> {
    let Some(password) = confirmation.and_then(|c| c.current_password) else {
        return if signed_in_recently(session) {
            Ok(())
        } else {
            Err(AppError::PasswordRequired)
        };
    };

    let user = UserRepository::get_by_id(&state.pool, &session.user_id).await?;
    state.login_throttle.check(&user.email)?;

    let password_valid = verify_stored(&password, &user.password_hash)
        .await
        .map_err(|e| {
            tracing::error!("Password verification failed: {}", e);
            AppError::InternalServerError
        })?;

    if !password_valid {
        tracing::warn!("Wrong password for a destructive action: user {}", user.id);
        state.login_throttle.record_failure(&user.email);
        return Err(AppError::Unauthorized);
    }

    state.login_throttle.record_success(&user.email);
    Ok(())
}

/// Delete all user's notes from server
pub async fn delete_all_notes(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    confirmation: Option<Json<PasswordConfirmation>>,
) -> AppResult<StatusCode> {
    confirm_account_holder(&state, &session, confirmation.map(|Json(c)| c)).await?;

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

    // Use the helper to delete all notes data
    let notes_deleted = UserRepository::delete_all_notes_data(&mut tx, &user_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete notes data: {}", e);
            AppError::InternalServerError
        })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!(
        "Deleted {} notes for user {}",
        notes_deleted,
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
    /// Re-wrapped master key blob (envelope encryption)
    pub wrapped_blob: Option<String>,
    pub kdf_version: Option<i32>,
    pub kdf_iterations: Option<i32>,
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
    let password_valid = verify_stored(&req.current_password, &user.password_hash)
        .await
        .map_err(|e| {
            tracing::error!("Password verification failed: {}", e);
            AppError::InternalServerError
        })?;

    if !password_valid {
        tracing::warn!("Password change failed: invalid current password for user {}", user_id);
        return Err(AppError::Unauthorized);
    }

    let kdf_version = req.kdf_version.unwrap_or(1);
    let kdf_iterations = req.kdf_iterations.unwrap_or(1_000_000);
    if let Some(blob) = &req.wrapped_blob {
        crate::api::auth::validate_wrapped_key(blob, kdf_version, kdf_iterations)?;
    }

    // Validate new password strength based on configured complexity
    if let Err(msg) = validate_password_strength(&req.new_password, &state.config.password_complexity) {
        tracing::warn!("Password change validation failed for user {}: {}", user_id, msg);
        return Err(AppError::BadRequest(msg));
    }

    // "Changing" to the same password would satisfy a forced change
    if verify_stored(&req.new_password, &user.password_hash).await.unwrap_or(false) {
        return Err(AppError::BadRequest(
            "New password must be different from the current one".to_string(),
        ));
    }

    // Hash new password with configured Argon2 parameters
    let new_password_hash = hash_for_storage(&state.config, &req.new_password)
        .await
        .map_err(|e| {
            tracing::error!("Password hashing failed: {}", e);
            AppError::InternalServerError
        })?;

    // All or nothing: a new password with the old blob would strand other devices,
    // and sessions from before the change are what it's meant to end.
    let mut tx = state.pool.begin().await?;

    UserRepository::set_password(&mut tx, user_id, &new_password_hash).await?;
    SessionRepository::delete_others_for_user(&mut tx, user_id, &session.id).await?;

    if let Some(blob) = &req.wrapped_blob {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query!(
            "INSERT INTO wrapped_keys (user_id, wrapped_blob, kdf_version, kdf_iterations, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id) DO UPDATE SET
               wrapped_blob = excluded.wrapped_blob,
               kdf_version = excluded.kdf_version,
               kdf_iterations = excluded.kdf_iterations,
               updated_at = excluded.updated_at",
            user_id,
            blob,
            kdf_version,
            kdf_iterations,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;

        tracing::info!("Wrapped key updated during password change for user: {}", user_id);
    }

    tx.commit().await?;

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
    confirmation: Option<Json<PasswordConfirmation>>,
) -> AppResult<StatusCode> {
    confirm_account_holder(&state, &session, confirmation.map(|Json(c)| c)).await?;

    let user_id = &session.user_id;

    let user = UserRepository::get_by_id(&state.pool, user_id).await?;

    match query.mode.as_str() {
        "deactivate" => {
            // Soft delete: allows re-registration with admin approval
            AdminService::deactivate(&state.pool, &user).await?;
            tracing::info!("User deactivated their account: {}", user_id);
            Ok(StatusCode::NO_CONTENT)
        }
        "delete" => {
            // Hard delete: remove all user data
            AdminService::delete(&state.pool, &user).await?;
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

/// Check user approval status (no auth required)
/// GET /api/v1/user/status?email=...
#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    pub email: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserStatusResponse {
    pub exists: bool,
    pub is_approved: bool,
    pub is_active: bool,
}

pub async fn check_status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatusQuery>,
) -> AppResult<Json<UserStatusResponse>> {
    // Look up user by email
    let user = UserRepository::find_by_email(&state.pool, &query.email)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::InternalServerError
        })?;

    match user {
        Some(u) => Ok(Json(UserStatusResponse {
            exists: true,
            is_approved: u.approved != 0,
            is_active: u.is_active != 0,
        })),
        None => Ok(Json(UserStatusResponse {
            exists: false,
            is_approved: false,
            is_active: false,
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct StatusCheckRequest {
    pub email: String,
    pub password: String,
}

/// Check where an account stands, for someone who holds its password
/// POST /api/v1/user/status
///
/// The GET above answers for any address, which tells anyone who has an account.
/// It stays for released clients. This one gives a stranger the same 401 whether
/// the address has an account or not, and counts as a password guess.
pub async fn check_own_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StatusCheckRequest>,
) -> AppResult<Json<UserStatusResponse>> {
    state.login_throttle.check(&req.email)?;

    let user = UserRepository::find_by_email(&state.pool, &req.email).await?;

    let password_valid = verify_sign_in(
        &state.config,
        &req.password,
        user.as_ref().map(|user| user.password_hash.as_str()),
    )
    .await
    .map_err(|e| {
        tracing::error!("Password verification failed: {}", e);
        AppError::InternalServerError
    })?;

    let Some(user) = user.filter(|_| password_valid) else {
        state.login_throttle.record_failure(&req.email);
        return Err(AppError::Unauthorized);
    };
    state.login_throttle.record_success(&req.email);

    Ok(Json(UserStatusResponse {
        exists: true,
        is_approved: user.approved != 0,
        is_active: user.is_active != 0,
    }))
}

/// Logout - invalidate current session
/// POST /api/v1/user/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<(StatusCode, HeaderMap)> {
    SessionRepository::delete(&state.pool, &session.id).await?;

    tracing::info!("User logged out: session_id={}", session.id);
    Ok((StatusCode::NO_CONTENT, clear_session_cookie(CookieScope::User)))
}

// ============================================================================
// Device Management
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub created_at: String,
    pub last_seen_at: Option<String>,
    pub is_active: bool,
}

/// List user's own devices
/// GET /api/v1/user/devices
pub async fn list_devices(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<Json<Vec<DeviceInfo>>> {
    let user_id = &session.user_id;

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
    .await
    .map_err(|e| {
        tracing::error!("Failed to list devices: {}", e);
        AppError::InternalServerError
    })?;

    let device_list: Vec<DeviceInfo> = devices
        .into_iter()
        .map(|d| DeviceInfo {
            id: d.id,
            name: d.device_name,
            device_type: d.device_type,
            created_at: d.created_at,
            last_seen_at: d.last_seen_at,
            is_active: d.is_active.unwrap_or(0) == 1,
        })
        .collect();

    Ok(Json(device_list))
}

/// Revoke (deactivate) user's own device
/// DELETE /api/v1/user/devices/:id
pub async fn revoke_device(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
    Path(device_id): Path<String>,
) -> AppResult<StatusCode> {
    let user_id = &session.user_id;

    // Verify the device belongs to this user
    let device = sqlx::query!(
        "SELECT user_id, is_active FROM clients WHERE id = ?",
        device_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError
    })?
    .ok_or_else(|| AppError::NotFound("Device not found".to_string()))?;

    // Verify the device belongs to this user
    if device.user_id != *user_id {
        return Err(AppError::Forbidden("Cannot revoke another user's device".to_string()));
    }

    let is_active = device.is_active.unwrap_or(0) == 1;

    if is_active {
        // Deactivate the device (soft revoke)
        sqlx::query!(
            "UPDATE clients SET is_active = 0 WHERE id = ?",
            device_id
        )
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to revoke device: {}", e);
            AppError::InternalServerError
        })?;

        tracing::info!("User {} revoked device {}", user_id, device_id);
    } else {
        // Already revoked — hard-delete the device
        sqlx::query!(
            "DELETE FROM clients WHERE id = ?",
            device_id
        )
        .execute(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete device: {}", e);
            AppError::InternalServerError
        })?;

        tracing::info!("User {} deleted revoked device {}", user_id, device_id);
    }

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================================
// Inbox Token Management
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateInboxTokenResponse {
    pub token: String,
}

/// Generate a new inbox token (replaces any existing token)
/// POST /api/v1/user/inbox-token
pub async fn generate_inbox_token(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<(StatusCode, Json<GenerateInboxTokenResponse>)> {
    let user_id = &session.user_id;

    // Generate 32 random bytes as hex (64 chars)
    let token: String = (0..32)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();

    // SHA-256 hash for storage
    let token_hash = hash_sha256(&token);

    // Store hash (replaces any existing token)
    sqlx::query!(
        "UPDATE users SET inbox_token_hash = ? WHERE id = ?",
        token_hash,
        user_id
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store inbox token: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!("Inbox token generated for user {}", user_id);

    Ok((
        StatusCode::CREATED,
        Json(GenerateInboxTokenResponse { token }),
    ))
}

/// Revoke the inbox token
/// DELETE /api/v1/user/inbox-token
pub async fn revoke_inbox_token(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<StatusCode> {
    let user_id = &session.user_id;

    sqlx::query!(
        "UPDATE users SET inbox_token_hash = NULL WHERE id = ?",
        user_id
    )
    .execute(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to revoke inbox token: {}", e);
        AppError::InternalServerError
    })?;

    tracing::info!("Inbox token revoked for user {}", user_id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxTokenStatusResponse {
    pub has_token: bool,
}

/// Check if an inbox token exists
/// GET /api/v1/user/inbox-token/status
pub async fn get_inbox_token_status(
    State(state): State<Arc<AppState>>,
    axum::Extension(session): axum::Extension<Session>,
) -> AppResult<Json<InboxTokenStatusResponse>> {
    let user_id = &session.user_id;

    let result = sqlx::query!(
        "SELECT inbox_token_hash FROM users WHERE id = ?",
        user_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check inbox token: {}", e);
        AppError::InternalServerError
    })?;

    let has_token = result
        .and_then(|r| r.inbox_token_hash)
        .is_some();

    Ok(Json(InboxTokenStatusResponse { has_token }))
}
