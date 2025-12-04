pub mod auth;
pub mod sync;

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

    use crate::AppState;

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

        // Look up client in database
        let result = sqlx::query!(
            "SELECT id, is_active FROM clients WHERE api_key = ?",
            hashed_key
        )
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match result {
            Some(client) if client.is_active == 1 => {
                let client_id = client.id.clone().unwrap_or_default();

                // Add client_id to request extensions
                request.extensions_mut().insert(client_id.clone());

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
}
