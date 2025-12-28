pub mod users;
pub mod stats;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::{db::SessionRepository, AppState};

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

    // Hash the session token for lookup (we store SHA-256 hashes)
    let mut hasher = Sha256::new();
    hasher.update(session_token.as_bytes());
    let token_hash = format!("{:x}", hasher.finalize());

    // Validate session and get user info
    let session = SessionRepository::validate_and_get(&state.pool, &token_hash)
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

/// Extract session token from headers
/// Checks both Cookie header and Authorization header (Bearer token)
fn extract_session_token(headers: &HeaderMap) -> Option<String> {
    // Try Authorization header first (Bearer token)
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }

    // Try Cookie header (session_token=xxx)
    if let Some(cookie_header) = headers.get("Cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            // Parse cookies (simple implementation)
            for cookie in cookie_str.split(';') {
                let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
                if parts.len() == 2 && parts[0] == "session_token" {
                    return Some(parts[1].to_string());
                }
            }
        }
    }

    None
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
