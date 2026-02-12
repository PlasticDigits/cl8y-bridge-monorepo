//! HTTP server for health and metrics endpoints

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use eyre::eyre;
use prometheus::{Encoder, IntCounter, IntGauge, Registry, TextEncoder};
use serde::Serialize;
use tokio::sync::RwLock;
use tracing::info;

/// Canceler statistics shared between watcher and HTTP server
#[derive(Debug, Default, Clone)]
pub struct CancelerStats {
    /// Number of approvals verified as valid
    pub verified_valid: u64,
    /// Number of approvals verified as invalid (cancelled)
    pub verified_invalid: u64,
    /// Number of cancel transactions submitted
    pub cancelled_count: u64,
    /// Last polled EVM block number
    pub last_evm_block: u64,
    /// Last polled Terra height
    pub last_terra_height: u64,
    /// Canceler instance ID
    pub canceler_id: String,
}

/// Prometheus metrics
pub struct Metrics {
    pub verified_valid_total: IntCounter,
    pub verified_invalid_total: IntCounter,
    pub cancelled_total: IntCounter,
    pub last_evm_block: IntGauge,
    pub last_terra_height: IntGauge,
    /// C4: Times the EVM pre-check circuit breaker tripped
    pub evm_precheck_circuit_breaker_trips_total: IntCounter,
    /// C3: Current entries in verified-hashes cache
    pub dedupe_verified_size: IntGauge,
    /// C3: Current entries in cancelled-hashes cache
    pub dedupe_cancelled_size: IntGauge,
    /// C2: Total pending Terra withdrawals seen this poll
    pub terra_pending_queue_depth: IntGauge,
    /// C2: Approvals skipped due to page cap
    pub terra_unprocessed_approvals: IntGauge,
    pub registry: Registry,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let verified_valid_total = IntCounter::new(
            "canceler_approvals_verified_valid_total",
            "Total number of approvals verified as valid",
        )
        .expect("constant metric name is valid");

        let verified_invalid_total = IntCounter::new(
            "canceler_approvals_verified_invalid_total",
            "Total number of approvals verified as invalid (fraudulent)",
        )
        .expect("constant metric name is valid");

        let cancelled_total = IntCounter::new(
            "canceler_approvals_cancelled_total",
            "Total number of cancel transactions submitted",
        )
        .expect("constant metric name is valid");

        let last_evm_block = IntGauge::new(
            "canceler_last_evm_block_processed",
            "Last EVM block number processed",
        )
        .expect("constant metric name is valid");

        let last_terra_height = IntGauge::new(
            "canceler_last_terra_height_processed",
            "Last Terra block height processed",
        )
        .expect("constant metric name is valid");

        let evm_precheck_circuit_breaker_trips_total = IntCounter::new(
            "canceler_evm_precheck_circuit_breaker_trips_total",
            "Times the EVM pre-check circuit breaker tripped",
        )
        .expect("constant metric name is valid");

        let dedupe_verified_size = IntGauge::new(
            "canceler_dedupe_verified_size",
            "Current entries in verified-hashes cache",
        )
        .expect("constant metric name is valid");

        let dedupe_cancelled_size = IntGauge::new(
            "canceler_dedupe_cancelled_size",
            "Current entries in cancelled-hashes cache",
        )
        .expect("constant metric name is valid");

        let terra_pending_queue_depth = IntGauge::new(
            "canceler_terra_pending_queue_depth",
            "Total pending Terra withdrawals seen this poll",
        )
        .expect("constant metric name is valid");

        let terra_unprocessed_approvals = IntGauge::new(
            "canceler_terra_unprocessed_approvals",
            "Approvals skipped due to Terra pagination page cap",
        )
        .expect("constant metric name is valid");

        // Register all metrics — expect is safe here because names are unique
        // constants and registration is called exactly once at startup
        registry
            .register(Box::new(verified_valid_total.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(verified_invalid_total.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(cancelled_total.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(last_evm_block.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(last_terra_height.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(evm_precheck_circuit_breaker_trips_total.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(dedupe_verified_size.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(dedupe_cancelled_size.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(terra_pending_queue_depth.clone()))
            .expect("metric registration must not be called twice");
        registry
            .register(Box::new(terra_unprocessed_approvals.clone()))
            .expect("metric registration must not be called twice");

        Self {
            verified_valid_total,
            verified_invalid_total,
            cancelled_total,
            last_evm_block,
            last_terra_height,
            evm_precheck_circuit_breaker_trips_total,
            dedupe_verified_size,
            dedupe_cancelled_size,
            terra_pending_queue_depth,
            terra_unprocessed_approvals,
            registry,
        }
    }
}

/// Shared state for the HTTP server
pub type SharedStats = Arc<RwLock<CancelerStats>>;
pub type SharedMetrics = Arc<Metrics>;

/// Combined app state
#[derive(Clone)]
pub struct AppState {
    pub stats: SharedStats,
    pub metrics: SharedMetrics,
}

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub canceler_id: String,
    pub verified_valid: u64,
    pub verified_invalid: u64,
    pub cancelled_count: u64,
    pub last_evm_block: u64,
    pub last_terra_height: u64,
}

/// Health check endpoint handler
async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let stats = state.stats.read().await;
    Json(HealthResponse {
        status: "healthy".to_string(),
        canceler_id: stats.canceler_id.clone(),
        verified_valid: stats.verified_valid,
        verified_invalid: stats.verified_invalid,
        cancelled_count: stats.cancelled_count,
        last_evm_block: stats.last_evm_block,
        last_terra_height: stats.last_terra_height,
    })
}

/// Liveness probe (always returns OK if server is running)
async fn liveness() -> &'static str {
    "OK"
}

/// Readiness probe (checks if watcher has started processing)
async fn readiness(State(state): State<AppState>) -> &'static str {
    let stats = state.stats.read().await;
    // Ready if we've polled at least one block on either chain
    if stats.last_evm_block > 0 || stats.last_terra_height > 0 {
        "OK"
    } else {
        "NOT_READY"
    }
}

/// Prometheus metrics endpoint
async fn prometheus_metrics(State(state): State<AppState>) -> Response {
    // Update gauges from current stats
    let stats = state.stats.read().await;
    state
        .metrics
        .last_evm_block
        .set(stats.last_evm_block as i64);
    state
        .metrics
        .last_terra_height
        .set(stats.last_terra_height as i64);
    drop(stats);

    // Encode metrics
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();

    if encoder.encode(&metric_families, &mut buffer).is_err() {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to encode metrics",
        )
            .into_response();
    }

    match Response::builder()
        .header(header::CONTENT_TYPE, encoder.format_type())
        .body(axum::body::Body::from(buffer))
    {
        Ok(resp) => resp,
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build metrics response",
        )
            .into_response(),
    }
}

/// Start the HTTP server for health and metrics
pub async fn start_server(
    bind_address: &str,
    port: u16,
    stats: SharedStats,
    prom_metrics: SharedMetrics,
) -> eyre::Result<()> {
    let state = AppState {
        stats,
        metrics: prom_metrics,
    };

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/healthz", get(liveness))
        .route("/readyz", get(readiness))
        .route("/metrics", get(prometheus_metrics))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", bind_address, port)
        .parse()
        .map_err(|e| eyre!("Invalid bind address {}:{}: {}", bind_address, port, e))?;
    info!("Health server listening on {}", addr);
    info!("  /health  - Full health status (JSON)");
    info!("  /metrics - Prometheus metrics");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
