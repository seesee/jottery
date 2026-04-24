pub mod admin;
pub mod auth;
pub mod extractors;
pub mod inbox;
pub mod passkeys;
pub mod sse;
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
    use std::sync::Arc;

    use crate::{utils::crypto::hash_sha256, AppState, db::SessionRepository};

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
        let hashed_key = hash_sha256(api_key);

        tracing::debug!(
            "API auth attempt: api_key_len={}, hashed_key={}",
            api_key.len(),
            &hashed_key[..16] // First 16 chars for debugging
        );

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

        // MFA gate: a session created when the user has passkeys enrolled
        // starts with mfa_verified = 0. It's only accepted on the passkey
        // authentication endpoints (which use `user_mfa_pending_middleware`
        // instead of this one). For every other protected endpoint, an
        // unverified session is rejected.
        if session.mfa_verified == 0 {
            tracing::warn!(
                "Rejected mfa-pending session {} on non-passkey endpoint",
                session.id,
            );
            return Err(StatusCode::UNAUTHORIZED);
        }

        // Add session to request extensions for use in handlers
        request.extensions_mut().insert(session);

        Ok(next.run(request).await)
    }

    /// Middleware for endpoints that complete a pending MFA step (i.e.
    /// passkey authenticate begin/complete). Accepts sessions in either
    /// state — mfa_verified = 0 OR 1 — because:
    /// - A fresh login creates mfa_verified = 0 and must be able to start
    ///   the passkey assertion immediately.
    /// - A session already verified via passkey might re-run the assertion
    ///   (e.g. for re-authentication on a sensitive action in the future).
    pub async fn user_mfa_pending_middleware(
        State(state): State<Arc<AppState>>,
        headers: HeaderMap,
        mut request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let session_token = extract_session_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

        let session = SessionRepository::validate_and_get(&state.pool, &session_token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let user_active: i64 = sqlx::query_scalar!(
            r#"SELECT is_active FROM users WHERE id = ?"#,
            session.user_id
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

        if user_active != 1 {
            return Err(StatusCode::FORBIDDEN);
        }

        request.extensions_mut().insert(session);
        Ok(next.run(request).await)
    }

    /// Information extracted from inbox token authentication
    /// Contains only user_id (inbox tokens have limited scope)
    #[derive(Debug, Clone)]
    pub struct InboxAuth {
        pub user_id: String,
    }

    /// Inbox token middleware — extracts Bearer token, SHA-256 hashes it,
    /// looks up users.inbox_token_hash, verifies user is active and approved
    pub async fn inbox_auth_middleware(
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

        let token = &auth_header[7..];

        // Hash the token
        let hashed_token = hash_sha256(token);

        // Look up user by inbox token hash
        let result = sqlx::query!(
            "SELECT id, is_active, approved FROM users WHERE inbox_token_hash = ?",
            hashed_token
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match result {
            Some(user) if user.is_active == 1 && user.approved == 1 => {
                request.extensions_mut().insert(InboxAuth {
                    user_id: user.id,
                });
                Ok(next.run(request).await)
            }
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }

    /// Extract session token from headers
    /// Checks both Cookie header and Authorization header (Bearer token)
    pub fn extract_session_token(headers: &HeaderMap) -> Option<String> {
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
