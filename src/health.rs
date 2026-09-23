use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::{db, types::Data};

const DEFAULT_ADDR: &str = "0.0.0.0:8080";

/// Address the health check server binds to (`HEALTH_CHECK_ADDR`, default `0.0.0.0:8080`).
pub fn health_check_addr() -> String {
    std::env::var("HEALTH_CHECK_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.into())
}

/// Shared liveness signal for the background poller. The poller updates it after each
/// cycle and the health check reads it. Starts healthy so a freshly booted bot passes
/// before the first poll completes.
#[derive(Clone)]
pub struct PollerHealth {
    healthy: Arc<AtomicBool>,
}

impl Default for PollerHealth {
    fn default() -> Self {
        Self {
            healthy: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl PollerHealth {
    pub fn mark_healthy(&self) {
        self.healthy.store(true, Ordering::Relaxed);
    }

    pub fn mark_failed(&self) {
        self.healthy.store(false, Ordering::Relaxed);
    }

    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Relaxed)
    }
}

/// Spawns a minimal HTTP server that answers `GET /health` for liveness/readiness probes.
/// It reports healthy when the SQLite connection can serve a query and the most recent
/// poller cycle succeeded.
pub fn start_health_server(data: Arc<Data>, addr: String, poller: PollerHealth) {
    tokio::spawn(async move {
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(error) => {
                eprintln!("Failed to bind health check server to {addr}: {error}");
                return;
            }
        };
        println!("Health check server listening on {addr}");

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let data = data.clone();
                    let poller = poller.clone();
                    tokio::spawn(async move {
                        if let Err(error) = handle_connection(stream, &data, &poller).await {
                            eprintln!("Health check connection error: {error}");
                        }
                    });
                }
                Err(error) => eprintln!("Health check accept error: {error}"),
            }
        }
    });
}

async fn handle_connection(
    mut stream: TcpStream,
    data: &Data,
    poller: &PollerHealth,
) -> std::io::Result<()> {
    let mut buffer = [0u8; 1024];
    let read = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let (method, path) = request_line(&request);

    let response = match (method, path) {
        ("GET", "/health") | ("GET", "/healthz") => {
            let db_ok = db_healthy(data).await;
            let poller_ok = poller.is_healthy();
            if db_ok && poller_ok {
                http_response(200, "OK", &status_body("ok", db_ok, poller_ok))
            } else {
                http_response(
                    503,
                    "Service Unavailable",
                    &status_body("unavailable", db_ok, poller_ok),
                )
            }
        }
        _ => http_response(404, "Not Found", "{\"status\":\"not_found\"}"),
    };

    stream.write_all(response.as_bytes()).await?;
    stream.flush().await
}

fn request_line(request: &str) -> (&str, &str) {
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    (parts.next().unwrap_or(""), parts.next().unwrap_or(""))
}

async fn db_healthy(data: &Data) -> bool {
    let conn = data.db.lock().await;
    db::ping(&conn).is_ok()
}

fn status_body(status: &str, db_ok: bool, poller_ok: bool) -> String {
    format!(
        "{{\"status\":\"{status}\",\"database\":\"{}\",\"poller\":\"{}\"}}",
        check_label(db_ok),
        check_label(poller_ok)
    )
}

fn check_label(ok: bool) -> &'static str {
    if ok { "ok" } else { "failing" }
}

fn http_response(status: u16, reason: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
