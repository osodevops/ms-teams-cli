mod handler;

use std::net::SocketAddr;

use hyper::service::service_fn;
use hyper_util::rt::TokioIo;

use crate::error::{Result, TeamsError};

/// Run the webhook listener HTTP server on the given port.
///
/// The server handles:
/// - GET `/` — health check (200 OK)
/// - POST with `?validationToken=xxx` — Graph subscription validation (echoes token)
/// - POST with JSON body — change notification processing (prints NDJSON to stdout)
///
/// Note: Microsoft Graph webhooks require HTTPS. Use a reverse proxy (e.g., ngrok)
/// in front of this listener for production use.
pub async fn run_listener(port: u16) -> Result<()> {
    let addr: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
        TeamsError::AuthError(format!("Failed to bind listener on port {port}: {e}"))
    })?;

    eprintln!("Webhook listener started on http://0.0.0.0:{port}");
    eprintln!("Press Ctrl+C to stop");

    loop {
        tokio::select! {
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _remote)) => {
                        let io = TokioIo::new(stream);
                        tokio::spawn(async move {
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(io, service_fn(handler::handle_request))
                                .await;
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Failed to accept connection: {e}");
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\nShutting down webhook listener");
                return Ok(());
            }
        }
    }
}
