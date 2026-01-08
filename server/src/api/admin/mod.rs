pub mod users;
pub mod stats;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::{api::middleware::extract_session_token, db::SessionRepository, AppState};

/// Admin session middleware
/// Validates session token and ensures user is admin
pub async fn admin_auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract session token from cookie or Authorization header
    let session_token = extract_session_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate session and get user info (function hashes token internally)
    let session = SessionRepository::validate_and_get(&state.pool, &session_token)
        .await
        .map_err(|e| {
            tracing::error!("Session validation failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Verify user is admin (check in database)
    let user = match sqlx::query!(
        r#"SELECT id, is_admin, is_active FROM users WHERE id = ?"#,
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

    // Add user_id to request extensions for use in handlers
    request.extensions_mut().insert(session.user_id.clone());

    // Update last_used_at timestamp (fire and forget)
    let pool = state.pool.clone();
    let session_id = session.id.clone();
    tokio::spawn(async move {
        let _ = SessionRepository::update_last_used(&pool, &session_id).await;
    });

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_session_token_from_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer test-token-123"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("test-token-123".to_string()));
    }

    #[test]
    fn test_extract_session_token_from_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Cookie",
            HeaderValue::from_static("session_token=test-token-456; other=value"),
        );

        let token = extract_session_token(&headers);
        assert_eq!(token, Some("test-token-456".to_string()));
    }

    #[test]
    fn test_extract_session_token_missing() {
        let headers = HeaderMap::new();
        let token = extract_session_token(&headers);
        assert_eq!(token, None);
    }
}
