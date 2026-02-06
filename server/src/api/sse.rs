//! Server-Sent Events (SSE) for real-time sync notifications
//!
//! When any device pushes changes, the server notifies all other connected
//! devices belonging to that user, triggering an immediate sync pull.
//!
//! Security: SSE tokens are short-lived (5 minutes) and obtained by
//! exchanging a valid API key. This prevents API keys from appearing
//! in URLs, browser history, or server logs.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::{utils::crypto::hash_sha256, AppState};

/// SSE token validity duration (5 minutes)
const SSE_TOKEN_VALIDITY_SECS: u64 = 300;

/// Notification sent when a sync push occurs
#[derive(Clone, Debug)]
pub struct SyncNotification {
    pub user_id: String,
    pub source_client_id: String, // Don't notify the source device
}

/// Broadcast channel type for sync notifications
pub type SyncBroadcast = broadcast::Sender<SyncNotification>;

/// SSE token info stored in memory
#[derive(Clone, Debug)]
pub struct SseTokenInfo {
    pub user_id: String,
    pub client_id: String,
    pub expires_at: u64, // Unix timestamp
}

/// SSE token store - maps token hash to token info
pub type SseTokenStore = Arc<RwLock<HashMap<String, SseTokenInfo>>>;

/// Create a new SSE token store
pub fn create_token_store() -> SseTokenStore {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Response containing the SSE token
#[derive(Debug, Serialize)]
pub struct SseTokenResponse {
    pub token: String,
    pub expires_in: u64,
}

/// Exchange API key for a short-lived SSE token
///
/// The API key is passed via Authorization header (Bearer token).
/// Returns a short-lived token that can be used in the SSE URL query parameter.
pub async fn get_sse_token(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<SseTokenResponse>, StatusCode> {
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
        "SSE token request: api_key_len={}, hashed_key={}",
        api_key.len(),
        &hashed_key[..16]
    );

    // Look up client in database
    let client = sqlx::query!(
        "SELECT id, user_id, is_active FROM clients WHERE api_key = ?",
        hashed_key
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error during SSE token request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let client = match client {
        Some(c) if c.is_active == Some(1) => c,
        Some(c) => {
            tracing::warn!(
                "SSE token request with inactive API key: client_id={}, is_active={:?}",
                c.id,
                c.is_active
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
        None => {
            tracing::warn!(
                "SSE token request with unknown API key: hashed_prefix={}",
                &hashed_key[..16]
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Generate a random token
    let token = uuid::Uuid::new_v4().to_string();

    // Hash the token for storage (so tokens in URLs can't be correlated to stored data)
    let token_hash = hash_sha256(&token);

    // Calculate expiration
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expires_at = now + SSE_TOKEN_VALIDITY_SECS;

    // Store token info
    {
        let mut store = state.sse_tokens.write().await;

        // Clean up expired tokens while we have the write lock
        store.retain(|_, info| info.expires_at > now);

        // Store the new token
        store.insert(
            token_hash,
            SseTokenInfo {
                user_id: client.user_id.clone(),
                client_id: client.id.clone(),
                expires_at,
            },
        );

        tracing::debug!(
            "SSE token issued for client {} (user {}), expires in {}s, store size: {}",
            client.id,
            client.user_id,
            SSE_TOKEN_VALIDITY_SECS,
            store.len()
        );
    }

    Ok(Json(SseTokenResponse {
        token,
        expires_in: SSE_TOKEN_VALIDITY_SECS,
    }))
}

/// Query parameters for SSE endpoint
#[derive(Debug, Deserialize)]
pub struct SseAuthQuery {
    /// Short-lived SSE token (obtained from /api/v1/sync/events/token)
    pub token: String,
}

/// SSE endpoint for sync notifications
///
/// Clients connect here to receive real-time notifications when other
/// devices sync changes. The connection remains open with periodic
/// heartbeats to keep proxies from timing out.
///
/// Authentication is via a short-lived token obtained from the
/// /api/v1/sync/events/token endpoint. This prevents API keys from
/// appearing in URLs, browser history, or server logs.
pub async fn sync_events(
    State(state): State<Arc<AppState>>,
    Query(auth): Query<SseAuthQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    // Hash the provided token
    let token_hash = hash_sha256(&auth.token);

    // Look up token in store
    let token_info = {
        let store = state.sse_tokens.read().await;
        store.get(&token_hash).cloned()
    };

    let token_info = match token_info {
        Some(info) => info,
        None => {
            tracing::warn!(
                "SSE connection attempt with unknown token: hash_prefix={}",
                &token_hash[..16]
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Check if token is expired
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if token_info.expires_at <= now {
        tracing::warn!(
            "SSE connection attempt with expired token: client_id={}, expired_at={}",
            token_info.client_id,
            token_info.expires_at
        );

        // Remove expired token
        {
            let mut store = state.sse_tokens.write().await;
            store.remove(&token_hash);
        }

        return Err(StatusCode::UNAUTHORIZED);
    }

    let user_id = token_info.user_id;
    let client_id = token_info.client_id;

    tracing::info!(
        "SSE connection established for user {} (client {})",
        user_id,
        client_id
    );

    // Subscribe to broadcast channel
    let rx = state.sync_broadcast.subscribe();

    // Filter notifications: only for this user, not from this client
    let stream = BroadcastStream::new(rx)
        .filter_map(move |result| {
            match result {
                Ok(notification) => {
                    // Only notify if:
                    // 1. Same user (user_id matches)
                    // 2. Different client (not the source of the push)
                    if notification.user_id == user_id
                        && notification.source_client_id != client_id
                    {
                        tracing::debug!(
                            "Sending sync notification to client {} (from {})",
                            client_id,
                            notification.source_client_id
                        );
                        Some(Ok(Event::default().event("sync").data("pull")))
                    } else {
                        None
                    }
                }
                Err(e) => {
                    // Lagged behind - missed some messages
                    // This is fine, client will sync anyway on next pull
                    tracing::warn!("SSE broadcast lagged: {}", e);
                    None
                }
            }
        });

    // Return SSE stream with keepalive heartbeat
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("heartbeat"),
    ))
}
