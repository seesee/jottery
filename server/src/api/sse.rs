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
use sqlx::SqlitePool;
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

    // The same rules as every other device request
    let client = crate::api::middleware::authenticate_device(&state.pool, api_key).await?;

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
                client_id: client.client_id.clone(),
                expires_at,
            },
        );

        tracing::debug!(
            "SSE token issued for client {} (user {}), expires in {}s, store size: {}",
            client.client_id,
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

/// Whether a device can still be told about changes. This reads the database rather
/// than listening for revocations, so it also sees what jottery-admin does, which
/// runs in another process.
async fn device_is_usable(pool: &SqlitePool, client_id: &str) -> Result<bool, sqlx::Error> {
    let device = sqlx::query!(
        r#"SELECT c.is_active, u.is_active AS user_is_active, u.approved AS user_approved
           FROM clients c
           JOIN users u ON u.id = c.user_id
           WHERE c.id = ?"#,
        client_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(device.is_some_and(|device| {
        device.is_active == Some(1) && device.user_is_active == 1 && device.user_approved == 1
    }))
}

/// Resolves once a device may no longer have a stream. A failed check isn't a
/// verdict: the stream carries on, and the next check decides.
async fn until_revoked(pool: SqlitePool, client_id: String, every: Duration) {
    loop {
        tokio::time::sleep(every).await;
        match device_is_usable(&pool, &client_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::info!("Ending the event stream of revoked device {}", client_id);
                return;
            }
            Err(e) => tracing::warn!("Couldn't recheck device {}: {}", client_id, e),
        }
    }
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

    // A token stays usable for reconnecting, so what it was issued for has to be
    // checked each time, not only when it was issued
    let usable = device_is_usable(&state.pool, &client_id).await.map_err(|e| {
        tracing::error!("Database error during SSE connection: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if !usable {
        state.sse_tokens.write().await.remove(&token_hash);
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!(
        "SSE connection established for user {} (client {})",
        user_id,
        client_id
    );

    // Subscribe to broadcast channel
    let rx = state.sync_broadcast.subscribe();

    // A stream never ends by itself. The server waits for it on shutdown, and a
    // device that's been revoked mustn't go on hearing when the user syncs.
    let stopping = {
        let shutdown = state.shutdown.clone();
        let revoked = until_revoked(
            state.pool.clone(),
            client_id.clone(),
            Duration::from_secs(state.config.sse_recheck_secs),
        );
        async move {
            tokio::select! {
                _ = shutdown.triggered() => {}
                _ = revoked => {}
            }
        }
    };

    // Filter notifications: only for this user, not from this client
    let stream = futures::StreamExt::take_until(BroadcastStream::new(rx), stopping)
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
