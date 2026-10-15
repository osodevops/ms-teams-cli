mod handler;
mod queue;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::sync::Semaphore;

use crate::error::{Result, TeamsError};
use crate::output;

/// Connections served at once; further ones wait in the accept backlog.
const MAX_CONNECTIONS: usize = 64;
/// Time allowed to send request headers, including on an idle keep-alive
/// connection, so a stalled client releases its connection slot.
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);

/// Run the webhook listener HTTP server on `bind:port`.
///
/// The server handles:
/// - GET `/` — health check (200 OK)
/// - POST with `?validationToken=xxx` — Graph subscription validation (echoes token)
/// - POST with JSON body — change notifications, printed as NDJSON to stdout
///   by a writer thread; 202 once queued, 503 when the queue is full
///
/// On Ctrl+C or SIGTERM it stops admitting notifications (503) and writes out
/// everything already acknowledged; a second signal abandons the rest. If any
/// acknowledged notification is not written, the count goes to stderr and the
/// process exits 1.
///
/// With `client_state`, notifications carrying another or no `clientState`
/// are dropped.
///
/// Note: Microsoft Graph webhooks require HTTPS. Use a reverse proxy (e.g., ngrok)
/// in front of this listener for production use.
pub async fn run_listener(bind: IpAddr, port: u16, client_state: Option<String>) -> Result<()> {
    let addr = SocketAddr::new(bind, port);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| TeamsError::ConfigError(format!("cannot listen on {addr}: {e}")))?;
    let local = listener.local_addr().unwrap_or(addr);

    let queue = queue::Queue::new(queue::OUTPUT_BUDGET, output::try_write_stdout_line);
    let state = Arc::new(handler::State::new(client_state.as_deref(), queue));
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));

    eprintln!("Webhook listener started on http://{local}");
    if client_state.is_none() {
        eprintln!(
            "Warning: notifications are not authenticated; set --client-state to verify them"
        );
    }
    eprintln!("Press Ctrl+C to stop");

    let mut signals = Signals::new()?;
    loop {
        tokio::select! {
            (slot, accept) = async {
                let slot = slots.clone().acquire_owned().await;
                (slot, listener.accept().await)
            } => {
                let Ok(slot) = slot else { continue };
                match accept {
                    Ok((stream, _remote)) => {
                        let io = TokioIo::new(stream);
                        let state = state.clone();
                        tokio::spawn(async move {
                            let _slot = slot;
                            let service = service_fn(move |req| {
                                handler::handle_request(req, state.clone())
                            });
                            let _ = hyper::server::conn::http1::Builder::new()
                                .timer(TokioTimer::new())
                                .header_read_timeout(HEADER_TIMEOUT)
                                .serve_connection(io, service)
                                .await;
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to accept connection: {e}");
                    }
                }
            }
            _ = signals.recv() => {
                eprintln!("\nShutting down webhook listener; Ctrl+C again to abandon queued output");
                break;
            }
            _ = state.queue().writer_failed() => break,
        }
    }

    drop(listener);
    let lost = state.queue().shutdown(signals.recv()).await;
    if lost > 0 {
        // Exit here: the error envelope would go to the stdout that failed.
        eprintln!("Error: {lost} acknowledged notifications were not written to stdout");
        std::process::exit(1);
    }
    Ok(())
}

/// Ctrl+C, and SIGTERM on Unix. Kept for the whole run so a signal between
/// polls, or a second one during shutdown, is not missed.
struct Signals {
    #[cfg(unix)]
    int: tokio::signal::unix::Signal,
    #[cfg(unix)]
    term: tokio::signal::unix::Signal,
}

impl Signals {
    #[cfg(unix)]
    fn new() -> Result<Self> {
        use tokio::signal::unix::{signal, SignalKind};
        let register = |kind| signal(kind).map_err(|e| TeamsError::Other(anyhow::Error::from(e)));
        Ok(Self {
            int: register(SignalKind::interrupt())?,
            term: register(SignalKind::terminate())?,
        })
    }

    #[cfg(not(unix))]
    fn new() -> Result<Self> {
        Ok(Self {})
    }

    async fn recv(&mut self) {
        #[cfg(unix)]
        tokio::select! {
            _ = self.int.recv() => {}
            _ = self.term.recv() => {}
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    }
}
