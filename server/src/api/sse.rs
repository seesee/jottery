//! Server-Sent Events (SSE) for real-time sync notifications
//!
//! When any device pushes changes, the server notifies all other connected
//! devices belonging to that user, triggering an immediate sync pull.

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::AppState;

/// Notification sent when a sync push occurs
#[derive(Clone, Debug)]
pub struct SyncNotification {
    pub user_id: String,
    pub source_client_id: String, // Don't notify the source device
}

/// Broadcast channel type for sync notifications
pub type SyncBroadcast = broadcast::Sender<SyncNotification>;

/// Query parameters for SSE endpoint (API key auth via query param)
/// EventSource doesn't support custom headers, so we use query params
#[derive(Debug, Deserialize)]
pub struct SseAuthQuery {
    /// API key for authentication
    pub api_key: String,
}

/// SSE endpoint for sync notifications
///
/// Clients connect here to receive real-time notifications when other
/// devices sync changes. The connection remains open with periodic
/// heartbeats to keep proxies from timing out.
///
/// Authentication is via query parameter since EventSource doesn't
/// support custom headers.
pub async fn sync_events(
    State(state): State<Arc<AppState>>,
    Query(auth): Query<SseAuthQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, axum::http::StatusCode> {
    // Authenticate via API key (same logic as middleware but for query param)
    let mut hasher = Sha256::new();
    hasher.update(auth.api_key.as_bytes());
    let hashed_key = format!("{:x}", hasher.finalize());

    tracing::debug!(
        "SSE auth attempt: api_key_len={}, hashed_key={}",
        auth.api_key.len(),
        &hashed_key[..16] // First 16 chars for debugging
    );

    // Look up client in database
    let client = sqlx::query!(
        "SELECT id, user_id, is_active FROM clients WHERE api_key = ?",
        hashed_key
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error during SSE auth: {}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let client = match client {
        Some(c) if c.is_active == Some(1) => c,
        Some(c) => {
            tracing::warn!(
                "SSE connection attempt with inactive API key: client_id={}, is_active={:?}",
                c.id,
                c.is_active
            );
            return Err(axum::http::StatusCode::UNAUTHORIZED);
        }
        None => {
            tracing::warn!(
                "SSE connection attempt with unknown API key: hashed_prefix={}",
                &hashed_key[..16]
            );
            return Err(axum::http::StatusCode::UNAUTHORIZED);
        }
    };

    let user_id = client.user_id;
    let client_id = client.id;

    tracing::info!(
        "SSE connection established for user {} (client {})",
        user_id,
        client_id
    );

    // Subscribe to broadcast channel
    let rx = state.sync_broadcast.subscribe();

    // Filter notifications: only for this user, not from this client
    let user_id_clone = user_id.clone();
    let client_id_clone = client_id.clone();

    let stream = BroadcastStream::new(rx)
        .filter_map(move |result| {
            match result {
                Ok(notification) => {
                    // Only notify if:
                    // 1. Same user (user_id matches)
                    // 2. Different client (not the source of the push)
                    if notification.user_id == user_id_clone
                        && notification.source_client_id != client_id_clone
                    {
                        tracing::debug!(
                            "Sending sync notification to client {} (from {})",
                            client_id_clone,
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
