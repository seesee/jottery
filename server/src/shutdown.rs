//! Telling long-lived work that the server is stopping.

use tokio::sync::watch;

/// Set once when the process is asked to stop.
///
/// Graceful shutdown waits for every connection to finish, and an event stream
/// never does by itself, so streams listen for this and end.
#[derive(Clone, Debug)]
pub struct Shutdown(watch::Sender<bool>);

impl Default for Shutdown {
    fn default() -> Self {
        Self(watch::Sender::new(false))
    }
}

impl Shutdown {
    pub fn trigger(&self) {
        self.0.send_replace(true);
    }

    /// Resolves once `trigger` has been called, however long ago.
    pub async fn triggered(&self) {
        let mut stopping = self.0.subscribe();
        // The sender is `self`, so the channel can't close while this waits
        let _ = stopping.wait_for(|stopping| *stopping).await;
    }
}

/// Resolves when the process is asked to stop: SIGTERM from `docker stop` or a
/// service manager, or Ctrl-C.
pub async fn signal() {
    let interrupt = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Can't listen for Ctrl-C: {}", e);
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut terminate) => {
                terminate.recv().await;
            }
            Err(e) => {
                tracing::error!("Can't listen for SIGTERM: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = interrupt => {}
        _ = terminate => {}
    }
}
