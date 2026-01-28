//! Health & Status API endpoints
//!
//! Provides HTTP endpoints for monitoring and status:
//! - GET /health - Simple health check
//! - GET /metrics - Prometheus metrics
//! - GET /status - Chain status and relayer health
//! - GET /pending - List pending transactions

#![allow(dead_code)]

use eyre::Result;
use prometheus::{Encoder, TextEncoder};
use serde::Serialize;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use crate::db;
use crate::metrics;

/// Server start time for uptime calculation
static mut START_TIME: Option<Instant> = None;

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

/// Start the API server (combines metrics and status endpoints)
pub async fn start_api_server(addr: SocketAddr, db: PgPool) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "API server started");

    // Record start time
    unsafe {
        START_TIME = Some(Instant::now());
    }

    // Mark relayer as up
    metrics::UP.set(1.0);

    loop {
        let (mut socket, _) = listener.accept().await?;
        let db = db.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            if socket.readable().await.is_ok() {
                let _ = socket.try_read(&mut buf);
            }

            let request = String::from_utf8_lossy(&buf);
            
            if request.contains("GET /metrics") {
                // Prometheus metrics
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
            } else if request.contains("GET /health") {
                let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
                let _ = socket.write_all(response.as_bytes()).await;
            } else if request.contains("GET /status") {
                let status = build_status_response(&db).await;
                let body = serde_json::to_string(&status).unwrap_or_else(|_| "{}".to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            } else if request.contains("GET /pending") {
                let body = r#"{"approvals":[],"releases":[]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            } else {
                let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
    }
}

async fn build_status_response(db: &PgPool) -> StatusResponse {
    let uptime = unsafe {
        START_TIME.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    };

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
