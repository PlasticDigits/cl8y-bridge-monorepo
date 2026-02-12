//! Health & Status API endpoints
//!
//! Provides HTTP endpoints for monitoring and status:
//! - GET /health - Simple health check (public)
//! - GET /metrics - Prometheus metrics (public)
//! - GET /status - Queue counts, uptime, chain sync status (auth-gated when OPERATOR_API_TOKEN set)
//! - GET /pending - List pending transactions (auth-gated when OPERATOR_API_TOKEN set)

#![allow(dead_code)]

use eyre::Result;
use prometheus::{Encoder, TextEncoder};
use serde::Serialize;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use crate::db;
use crate::metrics;

/// Server start time for uptime calculation (safe one-time init).
static START_TIME: OnceLock<Instant> = OnceLock::new();

/// Maximum concurrent connections to the API server.
const MAX_CONNECTIONS: usize = 256;

/// Read timeout for incoming connections.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Parsed HTTP request
struct ParsedRequest {
    method: String,
    path: String,
    headers: String,
}

/// Status response
#[derive(Serialize)]
struct StatusResponse {
    status: String,
    uptime_seconds: u64,
    queues: QueueStatus,
}

#[derive(Serialize)]
struct QueueStatus {
    pending_deposits: i64,
    pending_approvals: i64,
    pending_releases: i64,
    submitted_approvals: i64,
    submitted_releases: i64,
}

/// Pending transactions response
#[derive(Serialize)]
struct PendingResponse {
    approvals: Vec<ApprovalInfo>,
    releases: Vec<ReleaseInfo>,
}

#[derive(Serialize)]
struct ApprovalInfo {
    id: i64,
    nonce: i64,
    recipient: String,
    amount: String,
    status: String,
}

#[derive(Serialize)]
struct ReleaseInfo {
    id: i64,
    nonce: i64,
    recipient: String,
    amount: String,
    status: String,
}

/// Start the API server (combines metrics and status endpoints)
pub async fn start_api_server(addr: SocketAddr, db: PgPool) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "API server started");

    // Record start time (safe, one-time initialization)
    START_TIME.get_or_init(Instant::now);

    // Mark relayer as up
    metrics::UP.set(1.0);

    // Load optional bearer token for auth-gated endpoints (/status, /pending).
    // When set, requests to those endpoints must include `Authorization: Bearer <token>`.
    let api_token: Option<Arc<str>> = std::env::var("OPERATOR_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|t| Arc::from(t.as_str()));

    if api_token.is_some() {
        tracing::info!("OPERATOR_API_TOKEN set — /status and /pending require authentication");
    }

    let semaphore = Arc::new(Semaphore::new(MAX_CONNECTIONS));

    loop {
        let (mut socket, _) = listener.accept().await?;
        let db = db.clone();
        let token = api_token.clone();
        let sem = semaphore.clone();

        tokio::spawn(async move {
            // Acquire connection permit (bounded concurrency)
            let _permit = match sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };

            // Read request with timeout
            let mut buf = [0u8; 4096];
            let n = match tokio::time::timeout(READ_TIMEOUT, socket.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => n,
                _ => return,
            };

            // Parse HTTP request line and headers (structured, not substring)
            let parsed = match parse_http_request(&buf[..n]) {
                Some(p) => p,
                None => {
                    let _ = socket
                        .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                        .await;
                    return;
                }
            };

            // Route based on exact method + path match
            match (parsed.method.as_str(), parsed.path.as_str()) {
                ("GET", "/metrics") => {
                    // Prometheus metrics — public
                    let encoder = TextEncoder::new();
                    let metric_families = prometheus::gather();
                    let mut buffer = Vec::new();
                    let _ = encoder.encode(&metric_families, &mut buffer);

                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n",
                        buffer.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.write_all(&buffer).await;
                }
                ("GET", "/health") => {
                    // Health check — public
                    let response =
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = socket.write_all(response.as_bytes()).await;
                }
                ("GET", "/status") => {
                    // Status — auth-gated
                    if !check_auth(&parsed.headers, token.as_deref()) {
                        let _ = socket
                            .write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n")
                            .await;
                        return;
                    }
                    let status = build_status_response(&db).await;
                    let body = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                }
                ("GET", "/pending") => {
                    // Pending — auth-gated
                    if !check_auth(&parsed.headers, token.as_deref()) {
                        let _ = socket
                            .write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n")
                            .await;
                        return;
                    }
                    let pending = build_pending_response(&db).await;
                    let body = serde_json::to_string(&pending).unwrap_or_else(|_| "{}".to_string());
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                }
                _ => {
                    let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            }
        });
    }
}

/// Parse HTTP method, path, and headers from raw request bytes.
///
/// Returns owned strings to avoid lifetime issues across `.await` points.
fn parse_http_request(buf: &[u8]) -> Option<ParsedRequest> {
    let request = std::str::from_utf8(buf).ok()?;
    let first_line = request.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let headers_start = request.find('\n').map(|i| i + 1).unwrap_or(request.len());
    let headers = request[headers_start..].to_string();
    Some(ParsedRequest {
        method,
        path,
        headers,
    })
}

/// Check Authorization header against the configured bearer token.
///
/// Returns `true` if:
/// - No token is configured (open access), OR
/// - A valid `Authorization: Bearer <token>` header is present.
fn check_auth(headers: &str, required_token: Option<&str>) -> bool {
    let token = match required_token {
        Some(t) if !t.is_empty() => t,
        _ => return true,
    };
    for line in headers.lines() {
        // HTTP header names are case-insensitive (RFC 7230 §3.2)
        if line.len() > 14 && line[..14].eq_ignore_ascii_case("authorization:") {
            let value = line[14..].trim();
            if let Some(bearer_token) = value.strip_prefix("Bearer ") {
                return bearer_token.trim() == token;
            }
            if let Some(bearer_token) = value.strip_prefix("bearer ") {
                return bearer_token.trim() == token;
            }
        }
    }
    false
}

async fn build_status_response(db: &PgPool) -> StatusResponse {
    let uptime = START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0);

    let queues = QueueStatus {
        pending_deposits: db::count_pending_deposits(db).await.unwrap_or(0),
        pending_approvals: db::count_pending_approvals(db).await.unwrap_or(0),
        pending_releases: db::count_pending_releases(db).await.unwrap_or(0),
        submitted_approvals: db::count_submitted_approvals(db).await.unwrap_or(0),
        submitted_releases: db::count_submitted_releases(db).await.unwrap_or(0),
    };

    StatusResponse {
        status: "ok".to_string(),
        uptime_seconds: uptime,
        queues,
    }
}

async fn build_pending_response(db: &PgPool) -> PendingResponse {
    let approvals = db::get_pending_and_submitted_approvals(db, 50, 0)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|a| ApprovalInfo {
            id: a.id,
            nonce: a.nonce,
            recipient: a.recipient,
            amount: a.amount,
            status: a.status,
        })
        .collect();

    let releases = db::get_pending_and_submitted_releases(db, 50, 0)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|r| ReleaseInfo {
            id: r.id,
            nonce: r.nonce,
            recipient: r.recipient,
            amount: r.amount,
            status: r.status,
        })
        .collect();

    PendingResponse {
        approvals,
        releases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_request_valid() {
        let req = b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let parsed = parse_http_request(req).unwrap();
        assert_eq!(parsed.method, "GET");
        assert_eq!(parsed.path, "/health");
    }

    #[test]
    fn test_parse_http_request_invalid() {
        assert!(parse_http_request(b"").is_none());
        assert!(parse_http_request(b"\x00\xff").is_none());
    }

    #[test]
    fn test_check_auth_no_token_configured() {
        assert!(check_auth("", None));
        assert!(check_auth("", Some("")));
    }

    #[test]
    fn test_check_auth_valid_bearer() {
        let headers = "Host: localhost\r\nAuthorization: Bearer my-secret\r\n";
        assert!(check_auth(headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_case_insensitive_header() {
        let headers = "host: localhost\r\nauthorization: Bearer my-secret\r\n";
        assert!(check_auth(headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_wrong_token() {
        let headers = "Authorization: Bearer wrong\r\n";
        assert!(!check_auth(headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_missing_header() {
        let headers = "Host: localhost\r\n";
        assert!(!check_auth(headers, Some("my-secret")));
    }
}
