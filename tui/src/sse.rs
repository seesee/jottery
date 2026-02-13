//! Server-Sent Events (SSE) client for real-time sync notifications
//!
//! Runs in a background thread and sends notifications through a channel
//! when other devices sync changes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tracing::{debug, error, info, warn};

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
    /// Check for pending messages without blocking
    pub fn try_recv(&self) -> Option<SseMessage> {
        self.receiver.try_recv().ok()
    }

    /// Stop the SSE thread
    pub fn stop(&mut self) {
        debug!("Stopping SSE thread");
        self.stop_flag.store(true, Ordering::SeqCst);

        // Wait for thread to finish (with timeout)
        if let Some(handle) = self.thread_handle.take() {
            // Give the thread a moment to notice the stop flag
            thread::sleep(Duration::from_millis(100));

            // We can't really force-kill the thread, but the stop flag
            // will cause it to exit on next iteration
            let _ = handle.join();
        }
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

/// Main function for the SSE background thread
fn sse_thread_main(config: SseConfig, sender: Sender<SseMessage>, stop_flag: Arc<AtomicBool>) {
    let mut reconnect_delay = Duration::from_secs(1);
    let max_reconnect_delay = Duration::from_secs(30);
    let mut consecutive_failures = 0;
    let max_failures = 10;

    info!("SSE thread started for endpoint: {}", config.endpoint);

    while !stop_flag.load(Ordering::SeqCst) {
        match connect_and_listen(&config, &sender, &stop_flag) {
            Ok(()) => {
                // Clean exit (stop flag set)
                debug!("SSE connection closed cleanly");
                break;
            }
            Err(e) => {
                consecutive_failures += 1;
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

        // Reset on successful connection
        if consecutive_failures > 0 {
            consecutive_failures = 0;
            reconnect_delay = Duration::from_secs(1);
        }
    }

    info!("SSE thread exiting");
}

/// Get a short-lived SSE token from the server
///
/// This exchanges the API key for a temporary token that can be safely used in URLs.
/// The token expires after 5 minutes but the SSE connection will remain open.
fn get_sse_token(endpoint: &str, api_key: &str) -> Result<String, String> {
    let url = format!(
        "{}/api/v1/sync/events/token",
        endpoint.trim_end_matches('/')
    );

    debug!("Fetching SSE token from: {}", url);

    let response = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", api_key))
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("Failed to get SSE token: {}", e))?;

    if response.status() != 200 {
        return Err(format!(
            "Failed to get SSE token: HTTP {} {}",
            response.status(),
            response.status_text()
        ));
    }

    let body_str = response
        .into_string()
        .map_err(|e| format!("Failed to read SSE token response: {}", e))?;

    let body: serde_json::Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("Failed to parse SSE token response: {}", e))?;

    body.get("token")
        .and_then(|t| t.as_str())
        .map(String::from)
        .ok_or_else(|| "SSE token response missing 'token' field".to_string())
}

/// Connect to SSE endpoint and listen for events
///
/// Security: Uses short-lived tokens instead of API keys in the URL.
/// The API key is exchanged for a token via an authenticated endpoint.
fn connect_and_listen(
    config: &SseConfig,
    sender: &Sender<SseMessage>,
    stop_flag: &Arc<AtomicBool>,
) -> Result<(), String> {
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

    // Build an agent with appropriate timeouts for SSE:
    // - Short connect timeout (30s) to fail fast if server is unreachable
    // - Long read timeout (1 hour) to keep connection open for SSE events
    //   (server sends heartbeats every 30s, so 1 hour allows for transient issues)
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(3600)) // 1 hour - SSE needs long-lived connection
        .build();

    let response = agent
        .get(&url)
        .call()
        .map_err(|e| format!("Failed to connect: {}", e))?;

    if response.status() != 200 {
        return Err(format!("HTTP {}: {}", response.status(), response.status_text()));
    }

    info!("SSE connection established");
    let _ = sender.send(SseMessage::Connected);

    // Read the streaming response line by line
    let reader = BufReader::new(response.into_reader());
    let mut current_event = String::new();

    for line in reader.lines() {
        // Check stop flag periodically
        if stop_flag.load(Ordering::SeqCst) {
            return Ok(());
        }

        let line = line.map_err(|e| format!("Read error: {}", e))?;

        // SSE format: lines starting with "event:", "data:", or empty line (end of event)
        if line.is_empty() {
            // End of event - process it
            if current_event == "sync" {
                info!("SSE: Received sync notification from server");
                let _ = sender.send(SseMessage::SyncNotification);
            }
            current_event.clear();
        } else if let Some(event_type) = line.strip_prefix("event:") {
            current_event = event_type.trim().to_string();
            debug!("SSE event type: {}", current_event);
        } else if line.starts_with("data:") {
            // We don't need the data for sync events, just the event type
            debug!("SSE data line: {}", line);
        } else if line.starts_with(":") {
            // Comment/heartbeat - keep connection alive
            debug!("SSE heartbeat received");
        }
    }

    // Stream ended unexpectedly
    Err("SSE stream ended".to_string())
}
