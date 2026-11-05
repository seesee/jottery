pub mod users;
pub mod stats;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::{
    api::cookies::{is_forgeable, session_credential},
    db::SessionRepository,
    error::AppError,
    AppState,
};

/// The only admin routes open to an account that must change its password.
const PASSWORD_CHANGE_ALLOWED_PATHS: [&str; 2] =
    ["/api/v1/admin/change-password", "/api/v1/admin/logout"];

/// Admin session middleware
/// Validates session token and ensures user is admin
pub async fn admin_auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract session token from cookie or Authorization header
    let credential = session_credential(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    if is_forgeable(&credential, request.method(), &headers) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate session and get user info (function hashes token internally)
    let session = SessionRepository::validate_and_get(&state.pool, &credential.token)
        .await
        .map_err(|e| {
            tracing::error!("Session validation failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Verify user is admin (check in database)
    let user = match sqlx::query!(
        r#"SELECT id, is_admin, is_active, must_change_password FROM users WHERE id = ?"#,
        session.user_id
    )
    .fetch_optional(&state.pool)
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::warn!("Session references non-existent user: {}", session.user_id);
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(e) => {
            tracing::error!("Database error during admin auth: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Check if user is admin
    if user.is_admin != 1 {
        tracing::warn!("Non-admin user attempted admin access: {}", session.user_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if user is active
    if user.is_active != 1 {
        tracing::warn!("Inactive user attempted admin access: {}", session.user_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // A session on a password that must be changed can do nothing else
    if user.must_change_password != 0
        && !PASSWORD_CHANGE_ALLOWED_PATHS.contains(&request.uri().path())
    {
        return Ok(AppError::PasswordChangeRequired.into_response());
    }

    // Add user_id to request extensions for use in handlers
    request.extensions_mut().insert(session.user_id.clone());

    // Update last_used_at timestamp (fire and forget)
    let pool = state.pool.clone();
    let session_id = session.id.clone();
    tokio::spawn(async move {
        let _ = SessionRepository::update_last_used(&pool, &session_id).await;
    });

    request.extensions_mut().insert(session);

    Ok(next.run(request).await)
}
