pub mod admin;
pub mod auth;
pub mod cookies;
pub mod extractors;
pub mod inbox;
pub mod sse;
pub mod sync;
pub mod user;

// Middleware for API key authentication
pub mod middleware {
    use axum::{
        extract::{Request, State},
        http::{HeaderMap, StatusCode},
        middleware::Next,
        response::{IntoResponse, Response},
    };
    use std::sync::Arc;

    use crate::{
        api::cookies::{is_forgeable, session_credential},
        db::{activity_is_stale, SessionRepository},
        error::AppError,
        utils::crypto::hash_sha256,
        AppState,
    };

    /// The only account routes open to a user who must change their password.
    const PASSWORD_CHANGE_ALLOWED_PATHS: [&str; 2] =
        ["/api/v1/user/change-password", "/api/v1/user/logout"];

    /// Client information extracted from API key authentication
    /// Contains both client_id (for audit trail) and user_id (for access control)
    #[derive(Debug, Clone)]
    pub struct ClientInfo {
        pub client_id: String,
        pub user_id: String,
        pub last_seen_at: Option<String>,
    }

    /// Find the device an API key belongs to, if it may still be used.
    ///
    /// A device is only as good as its account, so a deactivated or unapproved
    /// user is refused with 403 rather than 401: the key is valid, and clients
    /// shouldn't conclude the device has been removed.
    pub async fn authenticate_device(
        pool: &sqlx::SqlitePool,
        api_key: &str,
    ) -> Result<ClientInfo, StatusCode> {
        let hashed_key = hash_sha256(api_key);

        let client = sqlx::query!(
            r#"SELECT c.id, c.user_id, c.is_active, c.last_seen_at, u.is_active AS user_is_active, u.approved AS user_approved
               FROM clients c
               JOIN users u ON u.id = c.user_id
               WHERE c.api_key = ?"#,
            hashed_key
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error during device authentication: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;

        if client.is_active != Some(1) {
            return Err(StatusCode::UNAUTHORIZED);
        }

        if client.user_is_active != 1 || client.user_approved != 1 {
            tracing::warn!(
                "Device {} refused: user {} is deactivated or unapproved",
                client.id,
                client.user_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        Ok(ClientInfo {
            client_id: client.id,
            user_id: client.user_id,
            last_seen_at: client.last_seen_at,
        })
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

        let client = authenticate_device(&state.pool, api_key).await?;

        if activity_is_stale(client.last_seen_at.as_deref()) {
            let now = chrono::Utc::now().to_rfc3339();
            let _ = sqlx::query!(
                "UPDATE clients SET last_seen_at = ? WHERE id = ?",
                now,
                client.client_id
            )
            .execute(&state.pool)
            .await;
        }

        // Add both client_id and user_id to request extensions
        // client_id for audit trail, user_id for access control
        request.extensions_mut().insert(client);

        Ok(next.run(request).await)
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
        let credential = session_credential(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

        if is_forgeable(&credential, request.method(), &headers) {
            return Err(StatusCode::FORBIDDEN);
        }

        // Validate session and get user info
        let session = SessionRepository::validate_and_get(&state.pool, &credential.token)
            .await
            .map_err(|e| {
                tracing::error!("Session validation failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        // Verify user is active (but don't check is_admin)
        let user = match sqlx::query!(
            r#"SELECT id, is_active, approved, must_change_password FROM users WHERE id = ?"#,
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

        // Sessions issued before an account was approved mustn't outlive that
        // rule: login refuses unapproved accounts.
        if user.approved != 1 {
            tracing::warn!("Unapproved user attempted access: {}", session.user_id);
            return Err(StatusCode::FORBIDDEN);
        }

        // A session on a password that must be changed can do nothing else
        if user.must_change_password != 0
            && !PASSWORD_CHANGE_ALLOWED_PATHS.contains(&request.uri().path())
        {
            return Ok(AppError::PasswordChangeRequired.into_response());
        }

        // Add session to request extensions for use in handlers
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
}
