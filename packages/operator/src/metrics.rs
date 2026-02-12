//! Prometheus metrics for the CL8Y Bridge Relayer
//!
//! Exposes metrics on /metrics endpoint for Prometheus scraping.

#![allow(dead_code)]

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge, register_gauge_vec, register_histogram_vec, CounterVec,
    Encoder, Gauge, GaugeVec, HistogramVec, TextEncoder,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

lazy_static! {
    // Block processing metrics
    pub static ref BLOCKS_PROCESSED: CounterVec = register_counter_vec!(
        "relayer_blocks_processed_total",
        "Total number of blocks processed",
        &["chain"]
    ).unwrap();

    pub static ref LATEST_BLOCK: GaugeVec = register_gauge_vec!(
        "relayer_latest_block",
        "Latest block number processed",
        &["chain"]
    ).unwrap();

    // Transaction metrics
    pub static ref DEPOSITS_DETECTED: CounterVec = register_counter_vec!(
        "relayer_deposits_detected_total",
        "Total number of deposit events detected",
        &["chain"]
    ).unwrap();

    pub static ref APPROVALS_SUBMITTED: CounterVec = register_counter_vec!(
        "relayer_approvals_submitted_total",
        "Total number of approvals submitted",
        &["chain", "status"]
    ).unwrap();

    pub static ref RELEASES_SUBMITTED: CounterVec = register_counter_vec!(
        "relayer_releases_submitted_total",
        "Total number of releases submitted",
        &["chain", "status"]
    ).unwrap();

    // Processing latency
    pub static ref PROCESSING_LATENCY: HistogramVec = register_histogram_vec!(
        "relayer_processing_latency_seconds",
        "Time to process a transaction from detection to submission",
        &["direction"],
        vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
    ).unwrap();

    // Queue sizes
    pub static ref PENDING_DEPOSITS: GaugeVec = register_gauge_vec!(
        "relayer_pending_deposits",
        "Number of deposits pending processing",
        &["chain"]
    ).unwrap();

    pub static ref PENDING_APPROVALS: GaugeVec = register_gauge_vec!(
        "relayer_pending_approvals",
        "Number of approvals pending submission",
        &["chain"]
    ).unwrap();

    // Error metrics
    pub static ref ERRORS: CounterVec = register_counter_vec!(
        "relayer_errors_total",
        "Total number of errors",
        &["chain", "type"]
    ).unwrap();

    pub static ref CONSECUTIVE_FAILURES: GaugeVec = register_gauge_vec!(
        "relayer_consecutive_failures",
        "Number of consecutive failures (circuit breaker)",
        &["chain"]
    ).unwrap();

    // Health metrics
    pub static ref UP: Gauge = register_gauge!(
        "relayer_up",
        "Whether the relayer is up and running"
    ).unwrap();

    pub static ref LAST_SUCCESSFUL_POLL: GaugeVec = register_gauge_vec!(
        "relayer_last_successful_poll_timestamp",
        "Unix timestamp of last successful poll",
        &["chain"]
    ).unwrap();

    // Fee metrics
    pub static ref FEES_COLLECTED: CounterVec = register_counter_vec!(
        "relayer_fees_collected_total",
        "Total fees collected (in base units)",
        &["chain", "token"]
    ).unwrap();

    pub static ref VOLUME_BRIDGED: CounterVec = register_counter_vec!(
        "relayer_volume_bridged_total",
        "Total volume bridged (in base units)",
        &["direction", "token"]
    ).unwrap();
}

/// Maximum concurrent connections to the metrics server.
const METRICS_MAX_CONNECTIONS: usize = 128;

/// Read timeout for incoming connections.
const METRICS_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// Start the metrics HTTP server
pub async fn start_metrics_server(addr: SocketAddr) -> eyre::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "Metrics server started");

    // Mark relayer as up
    UP.set(1.0);

    let semaphore = Arc::new(Semaphore::new(METRICS_MAX_CONNECTIONS));

    loop {
        let (mut socket, _) = listener.accept().await?;
        let sem = semaphore.clone();

        tokio::spawn(async move {
            // Acquire connection permit (bounded concurrency)
            let _permit = match sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };

            // Read request with timeout
            let mut buf = [0u8; 4096];
            let n = match tokio::time::timeout(METRICS_READ_TIMEOUT, socket.read(&mut buf)).await {
                Ok(Ok(n)) if n > 0 => n,
                _ => return,
            };

            // Parse HTTP request line (structured, not substring)
            let (method, path) = match parse_metrics_request(&buf[..n]) {
                Some(mp) => mp,
                None => {
                    let _ = socket
                        .write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n")
                        .await;
                    return;
                }
            };

            match (method.as_str(), path.as_str()) {
                ("GET", "/metrics") | ("GET", "/") => {
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
                    let response =
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nOK";
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

/// Parse HTTP method and path from raw request bytes (metrics server).
fn parse_metrics_request(buf: &[u8]) -> Option<(String, String)> {
    let request = std::str::from_utf8(buf).ok()?;
    let first_line = request.lines().next()?;
    let mut parts = first_line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    Some((method, path))
}

/// Record a block processed
pub fn record_block_processed(chain: &str, block_number: u64) {
    BLOCKS_PROCESSED.with_label_values(&[chain]).inc();
    LATEST_BLOCK
        .with_label_values(&[chain])
        .set(block_number as f64);
}

/// Record a deposit detected
pub fn record_deposit_detected(chain: &str) {
    DEPOSITS_DETECTED.with_label_values(&[chain]).inc();
}

/// Record an approval submitted
pub fn record_approval_submitted(chain: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    APPROVALS_SUBMITTED
        .with_label_values(&[chain, status])
        .inc();
}

/// Record a release submitted
pub fn record_release_submitted(chain: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    RELEASES_SUBMITTED.with_label_values(&[chain, status]).inc();
}

/// Record processing latency
pub fn record_latency(direction: &str, seconds: f64) {
    PROCESSING_LATENCY
        .with_label_values(&[direction])
        .observe(seconds);
}

/// Update pending counts
pub fn set_pending_deposits(chain: &str, count: i64) {
    PENDING_DEPOSITS
        .with_label_values(&[chain])
        .set(count as f64);
}

/// Update pending approvals
pub fn set_pending_approvals(chain: &str, count: i64) {
    PENDING_APPROVALS
        .with_label_values(&[chain])
        .set(count as f64);
}

/// Record an error
pub fn record_error(chain: &str, error_type: &str) {
    ERRORS.with_label_values(&[chain, error_type]).inc();
}

/// Update consecutive failures (circuit breaker)
pub fn set_consecutive_failures(chain: &str, count: u32) {
    CONSECUTIVE_FAILURES
        .with_label_values(&[chain])
        .set(count as f64);
}

/// Record last successful poll
pub fn record_successful_poll(chain: &str) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    LAST_SUCCESSFUL_POLL
        .with_label_values(&[chain])
        .set(timestamp);
}

/// Record fees collected
pub fn record_fees(chain: &str, token: &str, amount: f64) {
    FEES_COLLECTED
        .with_label_values(&[chain, token])
        .inc_by(amount);
}

/// Record volume bridged
pub fn record_volume(direction: &str, token: &str, amount: f64) {
    VOLUME_BRIDGED
        .with_label_values(&[direction, token])
        .inc_by(amount);
}
