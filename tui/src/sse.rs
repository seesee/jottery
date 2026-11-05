//! Server-Sent Events (SSE) client for real-time sync notifications
//!
//! Runs in a background thread and sends notifications through a channel
//! when other devices sync changes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tracing::{debug, error, warn};

/// Message sent from SSE thread to main thread
#[derive(Debug, Clone)]
pub enum SseMessage {
    /// Server notified that another device synced - trigger a pull
    SyncNotification,
    /// SSE connection established
    Connected,
    /// SSE connection lost (will auto-reconnect)
    Disconnected,
    /// SSE connection failed after max retries
    Failed(String),
}

/// Handle to control the SSE background thread
pub struct SseHandle {
    /// Channel to receive messages from SSE thread
    pub receiver: Receiver<SseMessage>,
    /// Flag to signal the thread to stop
    stop_flag: Arc<AtomicBool>,
    /// Thread handle for cleanup
    thread_handle: Option<JoinHandle<()>>,
}

impl SseHandle {
    /// A handle with these messages waiting, and no thread behind it.
    #[cfg(test)]
    pub fn holding(messages: Vec<SseMessage>) -> Self {
        let (sender, receiver) = mpsc::channel();
        for message in messages {
            sender.send(message).unwrap();
        }
        Self { receiver, stop_flag: Arc::default(), thread_handle: None }
    }

    /// Check for pending messages without blocking
    pub fn try_recv(&self) -> Option<SseMessage> {
        self.receiver.try_recv().ok()
    }

    /// Stop the SSE thread.
    ///
    /// This doesn't wait for it. The thread only sees the flag after its next line from
    /// the server, up to 30 seconds away on a quiet stream, and this runs on the UI
    /// thread, including on the way out. Nothing depends on the thread having finished.
    pub fn stop(&mut self) {
        debug!("Stopping SSE thread");
        self.stop_flag.store(true, Ordering::SeqCst);
        self.thread_handle.take();
    }
}

impl Drop for SseHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Configuration for SSE connection
pub struct SseConfig {
    pub endpoint: String,
    pub api_key: String,
}

/// Start the SSE background thread
pub fn start_sse_thread(config: SseConfig) -> SseHandle {
    let (sender, receiver) = mpsc::channel();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();

    let thread_handle = thread::spawn(move || {
        sse_thread_main(config, sender, stop_flag_clone);
    });

    SseHandle {
        receiver,
        stop_flag,
        thread_handle: Some(thread_handle),
    }
}

/// How long the server gets to hand over a token.
const TOKEN_TIMEOUT: Duration = Duration::from_secs(10);

/// An HTTP client with the same trust settings as the rest of sync.
fn client(connect: Duration, read: Duration) -> Result<reqwest::blocking::Client, SseError> {
    reqwest::blocking::Client::builder()
        .connect_timeout(connect)
        .timeout(read)
        .build()
        .map_err(|e| SseError::Unreachable(format!("Failed to set up the HTTP client: {e}")))
}

/// Why a connection to the event stream ended.
#[derive(Debug, PartialEq, Eq)]
enum SseError {
    /// The server turned this device away when it asked for a token. Trying again won't
    /// help, and the next sync will say why.
    Refused(String),
    /// The stream refused the token, which can have run out. A new one says why.
    TokenRefused,
    /// Never got as far as a stream
    Unreachable(String),
    /// A stream that was working ended
    Lost(String),
}

impl std::fmt::Display for SseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(reason) | Self::Unreachable(reason) | Self::Lost(reason) => f.write_str(reason),
            Self::TokenRefused => f.write_str("The event stream refused its token"),
        }
    }
}

/// Main function for the SSE background thread
fn sse_thread_main(config: SseConfig, sender: Sender<SseMessage>, stop_flag: Arc<AtomicBool>) {
    let mut reconnect_delay = Duration::from_secs(1);
    let max_reconnect_delay = Duration::from_secs(30);
    let mut consecutive_failures = 0;
    let max_failures = 10;
    let mut new_token_just_tried = false;

    debug!("SSE thread started for endpoint: {}", config.endpoint);

    while !stop_flag.load(Ordering::SeqCst) {
        match connect_and_listen(&config, &sender, &stop_flag) {
            Ok(()) => {
                // Clean exit (stop flag set)
                debug!("SSE connection closed cleanly");
                break;
            }
            Err(SseError::Refused(reason)) => {
                warn!("SSE refused: {reason}");
                let _ = sender.send(SseMessage::Failed(reason));
                break;
            }
            // Once at once. Asking for the token is what finds out why this happened,
            // and a server that refuses a new token too gets the usual wait.
            Err(SseError::TokenRefused) if !new_token_just_tried => {
                debug!("SSE token refused, asking for a new one");
                new_token_just_tried = true;
            }
            Err(e) => {
                // Only count as a failure if we never connected (immediate error).
                // Read timeouts after a successful connection are expected and
                // should not count towards the failure budget.
                if matches!(e, SseError::Lost(_)) {
                    // Read timeout or stream end — reset failures since the connection was alive
                    consecutive_failures = 0;
                    reconnect_delay = Duration::from_secs(1);
                    new_token_just_tried = false;
                } else {
                    consecutive_failures += 1;
                }
                warn!(
                    "SSE connection error (attempt {}/{}): {}",
                    consecutive_failures, max_failures, e
                );

                let _ = sender.send(SseMessage::Disconnected);

                if consecutive_failures >= max_failures {
                    error!("SSE max reconnect attempts reached, giving up");
                    let _ = sender.send(SseMessage::Failed(format!(
                        "Failed after {} attempts: {}",
                        max_failures, e
                    )));
                    break;
                }

                // Wait before reconnecting (with stop flag checks)
                let wait_until = std::time::Instant::now() + reconnect_delay;
                while std::time::Instant::now() < wait_until {
                    if stop_flag.load(Ordering::SeqCst) {
                        debug!("SSE thread stopping during reconnect wait");
                        return;
                    }
                    thread::sleep(Duration::from_millis(100));
                }

                // Exponential backoff
                reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay);
            }
        }

    }

    debug!("SSE thread exiting");
}

/// Get a short-lived SSE token from the server
///
/// This exchanges the API key for a temporary token that can be safely used in URLs.
/// The token expires after 5 minutes but the SSE connection will remain open.
fn get_sse_token(endpoint: &str, api_key: &str) -> Result<String, SseError> {
    let url = format!(
        "{}/api/v1/sync/events/token",
        endpoint.trim_end_matches('/')
    );

    debug!("Fetching SSE token from: {}", url);

    let response = client(TOKEN_TIMEOUT, TOKEN_TIMEOUT)?
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .map_err(|e| SseError::Unreachable(format!("Failed to get SSE token: {}", e.without_url())))?;

    let status = response.status();
    if matches!(status.as_u16(), 401 | 403) {
        return Err(SseError::Refused(format!("Failed to get SSE token: HTTP {status}")));
    }
    if !status.is_success() {
        return Err(SseError::Unreachable(format!("Failed to get SSE token: HTTP {status}")));
    }

    let body: serde_json::Value = response
        .json()
        .map_err(|e| SseError::Unreachable(format!("Failed to read SSE token response: {}", e.without_url())))?;

    body.get("token")
        .and_then(|t| t.as_str())
        .map(String::from)
        .ok_or_else(|| SseError::Unreachable("SSE token response missing 'token' field".to_string()))
}

/// Connect to SSE endpoint and listen for events
///
/// Security: Uses short-lived tokens instead of API keys in the URL.
/// The API key is exchanged for a token via an authenticated endpoint.
fn connect_and_listen(
    config: &SseConfig,
    sender: &Sender<SseMessage>,
    stop_flag: &Arc<AtomicBool>,
) -> Result<(), SseError> {
    use std::io::{BufRead, BufReader};

    // First, exchange the API key for a short-lived SSE token
    // This prevents the API key from appearing in URLs/logs
    let sse_token = get_sse_token(&config.endpoint, &config.api_key)?;

    let url = format!(
        "{}/api/v1/sync/events?token={}",
        config.endpoint.trim_end_matches('/'),
        sse_token
    );

    debug!("Connecting to SSE endpoint: {}", config.endpoint);

    // Connecting gets 30s, to fail fast if the server is unreachable. The other timeout
    // applies to each read, and the server sends heartbeats every 30s, so 45s allows for
    // some jitter while still letting the thread check its stop flag. On a timeout the
    // thread reconnects, after checking the flag. The URL holds the token, so it stays
    // out of the error.
    let response = client(Duration::from_secs(30), Duration::from_secs(45))?
        .get(&url)
        .send()
        .map_err(|e| SseError::Unreachable(format!("Failed to connect: {}", e.without_url())))?;

    let status = response.status();
    if status.as_u16() == 401 {
        return Err(SseError::TokenRefused);
    }
    if !status.is_success() {
        return Err(SseError::Unreachable(format!("Failed to connect: HTTP {status}")));
    }

    debug!("SSE connection established");
    let _ = sender.send(SseMessage::Connected);

    // Read the streaming response line by line
    let reader = BufReader::new(response);
    let mut current_event = String::new();

    for line in reader.lines() {
        // Check stop flag periodically
        if stop_flag.load(Ordering::SeqCst) {
            return Ok(());
        }

        let line = line.map_err(|e| SseError::Lost(format!("Read error: {e}")))?;

        // SSE format: lines starting with "event:", "data:", or empty line (end of event)
        //
        // Per the SSE spec, events are terminated by a blank line. However,
        // HTTP/2 and reverse proxies (e.g. Caddy) may buffer the trailing
        // blank line until the next chunk arrives (often the next heartbeat,
        // ~30 s later). Since our protocol only uses single event+data pairs,
        // we process the event as soon as the "data:" line arrives — matching
        // the iOS and web client behaviour.
        if line.is_empty() {
            // Blank line — reset state (handles late-arriving terminators
            // so we don't re-fire on the next blank line).
            current_event.clear();
        } else if let Some(event_type) = line.strip_prefix("event:") {
            current_event = event_type.trim().to_string();
            debug!("SSE event type: {}", current_event);
        } else if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            debug!("SSE data line: {}", data);
            // Process immediately once we have event + data
            if current_event == "sync" && data == "pull" {
                debug!("SSE: Received sync notification from server");
                let _ = sender.send(SseMessage::SyncNotification);
                current_event.clear();
            }
        } else if line.starts_with(":") {
            // Comment/heartbeat - keep connection alive
            debug!("SSE heartbeat received");
        }
    }

    // Stream ended unexpectedly
    Err(SseError::Lost("SSE stream ended".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicUsize;

    /// A stub of the two endpoints, answering the stream with each status in turn, and
    /// counting what it's asked for tokens.
    fn serve_events(token_status: &'static str, stream_statuses: Vec<&'static str>) -> (String, Arc<AtomicUsize>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let tokens = Arc::new(AtomicUsize::new(0));
        let asked = tokens.clone();

        thread::spawn(move || {
            let mut statuses = stream_statuses.into_iter();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let Some(request) = crate::test_support::read_request(&mut stream) else {
                    continue;
                };

                let (status, body) = if request.path.ends_with("/events/token") {
                    asked.fetch_add(1, Ordering::SeqCst);
                    (token_status, r#"{"token":"short-lived","expires_in":300}"#)
                } else {
                    (statuses.next().unwrap_or("401 Unauthorized"), ": hello\n\n")
                };

                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        (endpoint, tokens)
    }

    fn config(endpoint: String) -> SseConfig {
        SseConfig { endpoint, api_key: uuid::Uuid::new_v4().to_string() }
    }

    /// A token runs out after five minutes, so a reconnect can find it refused.
    #[test]
    fn a_refused_stream_gets_a_new_token_at_once() {
        let (endpoint, tokens) = serve_events("200 OK", vec!["401 Unauthorized", "200 OK"]);

        let mut handle = start_sse_thread(config(endpoint));

        assert!(matches!(handle.receiver.recv().unwrap(), SseMessage::Connected));
        assert_eq!(tokens.load(Ordering::SeqCst), 2);
        handle.stop();
    }

    /// A revoked device can't get a token, so asking again helps nobody.
    #[test]
    fn a_refused_token_stops_the_stream_for_good() {
        for status in ["401 Unauthorized", "403 Forbidden"] {
            let (endpoint, tokens) = serve_events(status, vec![]);

            let handle = start_sse_thread(config(endpoint));

            assert!(matches!(handle.receiver.recv().unwrap(), SseMessage::Failed(_)));
            assert!(handle.receiver.recv().is_err(), "the thread should have ended");
            assert_eq!(tokens.load(Ordering::SeqCst), 1);
        }
    }

    /// A quiet stream can keep its thread blocked for half a minute. The thread here only
    /// finishes after `stop` has returned, so waiting for it would never end.
    #[test]
    fn stopping_does_not_wait_for_the_thread() {
        let (release, blocked) = mpsc::channel::<()>();
        let (_sender, receiver) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let mut handle = SseHandle {
            receiver,
            stop_flag: stop_flag.clone(),
            thread_handle: Some(thread::spawn(move || {
                let _ = blocked.recv();
            })),
        };

        let (stopped, finished) = mpsc::channel();
        thread::spawn(move || {
            handle.stop();
            stopped.send(()).unwrap();
        });

        // Only a stop that waits for the thread runs into this, so it can be generous
        assert!(finished.recv_timeout(Duration::from_secs(120)).is_ok());
        assert!(stop_flag.load(Ordering::SeqCst));
        release.send(()).unwrap();
    }
}
