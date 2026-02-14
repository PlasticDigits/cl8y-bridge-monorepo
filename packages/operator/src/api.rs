//! Health & Status API endpoints
//!
//! Provides HTTP endpoints for monitoring and status:
//! - GET /health - Simple health check (public)
//! - GET /metrics - Prometheus metrics (public)
//! - GET /status - Queue counts, uptime, chain sync status (auth-gated when OPERATOR_API_TOKEN set)
//! - GET /pending - List pending transactions (auth-gated when OPERATOR_API_TOKEN set)

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use eyre::Result;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use prometheus::{Encoder, TextEncoder};
use serde::Serialize;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use crate::db;
use crate::metrics;

/// Shared application state for all handlers.
#[derive(Clone)]
struct AppState {
    db: PgPool,
    api_token: Option<Arc<str>>,
    start_time: Instant,
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

/// Start the API server using axum.
pub async fn start_api_server(addr: SocketAddr, db: PgPool) -> Result<()> {
    // Load optional bearer token for auth-gated endpoints (/status, /pending).
    // When set, requests to those endpoints must include `Authorization: Bearer <token>`.
    let api_token: Option<Arc<str>> = std::env::var("OPERATOR_API_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(|t| Arc::from(t.as_str()));

    if api_token.is_some() {
        tracing::info!("OPERATOR_API_TOKEN set — /status and /pending require authentication");
    }

    // Rate limiting (configurable via env)
    let rate_per_second: u64 = std::env::var("RATE_LIMIT_PER_SECOND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let rate_burst_size: u32 = std::env::var("RATE_LIMIT_BURST_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let governor_conf = GovernorConfigBuilder::default()
        .per_second(rate_per_second)
        .burst_size(rate_burst_size)
        .finish()
        .ok_or_else(|| eyre::eyre!("Invalid rate limit config"))?;

    tracing::info!(
        per_second = rate_per_second,
        burst_size = rate_burst_size,
        "API rate limiting enabled"
    );

    // Mark relayer as up
    metrics::UP.set(1.0);

    let state = AppState {
        db,
        api_token,
        start_time: Instant::now(),
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/status", get(status_handler))
        .route("/pending", get(pending_handler))
        .with_state(state)
        .layer(GovernorLayer::new(governor_conf));

    tracing::info!(%addr, "API server started");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Use into_make_service_with_connect_info so Governor can extract peer IP
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// Health check — public, always returns 200 OK.
async fn health_handler() -> &'static str {
    "OK"
}

/// Prometheus metrics — public.
async fn metrics_handler() -> Response {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();

    if encoder.encode(&metric_families, &mut buffer).is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to encode metrics",
        )
            .into_response();
    }

    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        buffer,
    )
        .into_response()
}

/// Status — auth-gated when OPERATOR_API_TOKEN is set.
async fn status_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !check_auth(&headers, state.api_token.as_deref()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let uptime = state.start_time.elapsed().as_secs();

    let queues = QueueStatus {
        pending_deposits: db::count_pending_deposits(&state.db).await.unwrap_or(0),
        pending_approvals: db::count_pending_approvals(&state.db).await.unwrap_or(0),
        pending_releases: db::count_pending_releases(&state.db).await.unwrap_or(0),
        submitted_approvals: db::count_submitted_approvals(&state.db).await.unwrap_or(0),
        submitted_releases: db::count_submitted_releases(&state.db).await.unwrap_or(0),
    };

    let status = StatusResponse {
        status: "ok".to_string(),
        uptime_seconds: uptime,
        queues,
    };

    Json(status).into_response()
}

/// Pending transactions — auth-gated when OPERATOR_API_TOKEN is set.
async fn pending_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !check_auth(&headers, state.api_token.as_deref()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let approvals = db::get_pending_and_submitted_approvals(&state.db, 50, 0)
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

    let releases = db::get_pending_and_submitted_releases(&state.db, 50, 0)
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

    let pending = PendingResponse {
        approvals,
        releases,
    };

    Json(pending).into_response()
}

// ─── Auth ───────────────────────────────────────────────────────────────────

/// Check Authorization header using axum's typed `HeaderMap`.
///
/// Returns `true` if:
/// - No token is configured (open access), OR
/// - A valid `Authorization: Bearer <token>` header is present.
fn check_auth(headers: &HeaderMap, required_token: Option<&str>) -> bool {
    let token = match required_token {
        Some(t) if !t.is_empty() => t,
        _ => return true,
    };

    match headers.get(header::AUTHORIZATION) {
        Some(value) => match value.to_str() {
            Ok(value_str) => value_str
                .strip_prefix("Bearer ")
                .or_else(|| value_str.strip_prefix("bearer "))
                .map(|t| t.trim() == token)
                .unwrap_or(false),
            Err(_) => false,
        },
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_check_auth_no_token_configured() {
        let headers = HeaderMap::new();
        assert!(check_auth(&headers, None));
        assert!(check_auth(&headers, Some("")));
    }

    #[test]
    fn test_check_auth_valid_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer my-secret"),
        );
        assert!(check_auth(&headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_lowercase_bearer_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("bearer my-secret"),
        );
        assert!(check_auth(&headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_wrong_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer wrong"),
        );
        assert!(!check_auth(&headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_missing_header() {
        let headers = HeaderMap::new();
        assert!(!check_auth(&headers, Some("my-secret")));
    }

    #[test]
    fn test_check_auth_no_bearer_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );
        assert!(!check_auth(&headers, Some("my-secret")));
    }
}
