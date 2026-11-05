//! Limits on requests as a whole, so that no burst of them can take the server down.

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// Requests handled at once. More are refused, not queued: the point is to stay up.
/// An event stream only counts until its response starts.
const MAX_IN_FLIGHT: usize = 256;

/// Pushes and pulls handled at once. Each can hold tens of MB, so these wait their
/// turn rather than fail a device's sync. A body isn't read until its turn comes.
const MAX_BULK_IN_FLIGHT: usize = 8;

/// How long a request gets, including reading its body. This is what ends a body
/// sent a byte at a time.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a push or pull gets, including the wait for its turn. A real sync can
/// be large and on a slow connection.
const BULK_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The routes that carry a vault's worth of data.
fn is_bulk(path: &str) -> bool {
    matches!(path, "/api/v1/sync/push" | "/api/v1/sync/pull")
}

#[derive(Clone)]
pub struct RequestLimits {
    in_flight: Arc<Semaphore>,
    bulk_in_flight: Arc<Semaphore>,
    timeout: Duration,
    bulk_timeout: Duration,
}

impl Default for RequestLimits {
    fn default() -> Self {
        Self::new(MAX_IN_FLIGHT, MAX_BULK_IN_FLIGHT, REQUEST_TIMEOUT, BULK_TIMEOUT)
    }
}

impl RequestLimits {
    pub fn new(
        in_flight: usize,
        bulk_in_flight: usize,
        timeout: Duration,
        bulk_timeout: Duration,
    ) -> Self {
        Self {
            in_flight: Arc::new(Semaphore::new(in_flight)),
            bulk_in_flight: Arc::new(Semaphore::new(bulk_in_flight)),
            timeout,
            bulk_timeout,
        }
    }
}

fn refuse(status: StatusCode, message: &str) -> Response {
    (
        status,
        [(header::RETRY_AFTER, "1")],
        Json(json!({ "error": message })),
    )
        .into_response()
}

/// Middleware that applies the limits. A health check is exempt: a server that's
/// busy isn't dead, and shouldn't get restarted for being busy.
pub async fn enforce(
    State(limits): State<RequestLimits>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/health" {
        return next.run(request).await;
    }

    let Ok(_permit) = limits.in_flight.clone().try_acquire_owned() else {
        tracing::warn!("Refusing a request: {} already in flight", MAX_IN_FLIGHT);
        return refuse(StatusCode::SERVICE_UNAVAILABLE, "The server is busy");
    };

    let finished = if is_bulk(path) {
        tokio::time::timeout(limits.bulk_timeout, async {
            let _turn = limits.bulk_in_flight.acquire().await;
            next.run(request).await
        })
        .await
    } else {
        tokio::time::timeout(limits.timeout, next.run(request)).await
    };

    finished.unwrap_or_else(|_| refuse(StatusCode::REQUEST_TIMEOUT, "The request took too long"))
}
