pub mod admin;
pub mod auth;
pub mod sync;
pub mod user;

// Middleware for API key authentication
pub mod middleware {
    use axum::{
        extract::{Request, State},
        http::{HeaderMap, StatusCode},
        middleware::Next,
        response::Response,
    };
    use sha2::{Sha256, Digest};
    use std::sync::Arc;

    use crate::{AppState, db::SessionRepository, models::Session};

    /// Client information extracted from API key authentication
    /// Contains both client_id (for audit trail) and user_id (for access control)
    #[derive(Debug, Clone)]
    pub struct ClientInfo {
        pub client_id: String,
        pub user_id: String,
    }

    pub async fn auth_middleware(
        State(state): State<Arc<AppState>>,
        headers: HeaderMap,
        mut request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        // Extract Authorization header
        let auth_header = headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Check Bearer token format
        if !auth_header.starts_with("Bearer ") {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let api_key = &auth_header[7..]; // Remove "Bearer " prefix

        // Hash the API key
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        let hashed_key = format!("{:x}", hasher.finalize());

        // Look up client in database (including user_id for access control)
        let result = sqlx::query!(
            "SELECT id, user_id, is_active FROM clients WHERE api_key = ?",
            hashed_key
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match result {
            Some(client) if client.is_active == Some(1) => {
                let client_id = client.id;
                let user_id = client.user_id;

                // Add both client_id and user_id to request extensions
                // client_id for audit trail, user_id for access control
                request.extensions_mut().insert(ClientInfo {
                    client_id: client_id.clone(),
                    user_id,
                });

                // Update last_seen_at
                let now = chrono::Utc::now().to_rfc3339();
                let _ = sqlx::query!(
                    "UPDATE clients SET last_seen_at = ? WHERE id = ?",
                    now,
                    client_id
                )
                .execute(&state.pool)
                .await;

                Ok(next.run(request).await)
            }
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }

    /// User session middleware (for account management, not admin)
    /// Validates session token but doesn't require admin privileges
    pub async fn user_auth_middleware(
        State(state): State<Arc<AppState>>,
        headers: HeaderMap,
        mut request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        // Extract session token from cookie or Authorization header
        let session_token = extract_session_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

        // Validate session and get user info
        let session = SessionRepository::validate_and_get(&state.pool, &session_token)
            .await
            .map_err(|e| {
                tracing::error!("Session validation failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Verify user is active (but don't check is_admin)
        let user = match sqlx::query!(
            r#"SELECT id, is_active FROM users WHERE id = ?"#,
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
                tracing::error!("Database error during user auth: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        // Check if user is active
        if user.is_active != 1 {
            tracing::warn!("Inactive user attempted access: {}", session.user_id);
            return Err(StatusCode::FORBIDDEN);
        }

        // Add session to request extensions for use in handlers
        request.extensions_mut().insert(session);

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
}
