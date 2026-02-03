//! CL8Y Bridge Canceler Node
//!
//! Minimal MVP canceler that monitors withdraw approvals and cancels
//! any that fail verification against source chain deposits.
//!
//! # Watchtower Pattern
//!
//! The canceler is part of the watchtower security pattern:
//! 1. Operator submits ApproveWithdraw to destination chain
//! 2. Delay period begins (e.g., 60 seconds for local, 5 minutes for production)
//! 3. Canceler nodes verify approval against source chain deposit
//! 4. If invalid, canceler submits CancelWithdrawApproval before delay expires
//! 5. If valid, anyone can execute after delay
//!
//! # MVP Scope
//!
//! This MVP implementation:
//! - Polls for new approvals on EVM and Terra
//! - Verifies approvals by checking source chain deposit hashes
//! - Submits cancel transactions for invalid approvals
//! - Exposes health endpoint for monitoring
//!
//! # Health Endpoint
//!
//! The canceler exposes HTTP endpoints for health monitoring:
//! - GET /health - Full health status with stats
//! - GET /healthz - Liveness probe
//! - GET /readyz - Readiness probe

mod config;
mod evm_client;
mod hash;
mod server;
mod terra_client;
mod verifier;
mod watcher;

use std::sync::Arc;

use config::Config;
use server::{CancelerStats, Metrics, SharedMetrics, SharedStats};
use tokio::sync::RwLock;
use tracing::info;
use watcher::CancelerWatcher;

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> eyre::Result<()> {
    init_logging();

    info!("Starting CL8Y Bridge Canceler Node");

    let config = Config::load()?;
    info!(
        canceler_id = %config.canceler_id,
        evm_rpc = %config.evm_rpc_url,
        terra_lcd = %config.terra_lcd_url,
        health_port = %config.health_port,
        "Configuration loaded"
    );

    // Create shared stats for health endpoint
    let stats: SharedStats = Arc::new(RwLock::new(CancelerStats::default()));

    // Create Prometheus metrics
    let metrics: SharedMetrics = Arc::new(Metrics::new());

    // Start HTTP server for health endpoint
    let health_port = config.health_port;
    let server_stats = Arc::clone(&stats);
    let server_metrics = Arc::clone(&metrics);
    tokio::spawn(async move {
        if let Err(e) = server::start_server(health_port, server_stats, server_metrics).await {
            tracing::error!(error = %e, "Health server error");
        }
    });

    // Create the watcher with shared stats and metrics
    let mut watcher =
        CancelerWatcher::new(&config, Arc::clone(&stats), Arc::clone(&metrics)).await?;

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Handle signals
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx.send(()).await;
    });

    // Run the watcher
    watcher.run(shutdown_rx).await?;

    info!("CL8Y Bridge Canceler stopped");
    Ok(())
}

fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,cl8y_canceler=debug"));

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true))
        .with(filter)
        .init();
}

async fn wait_for_shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, initiating shutdown");
        }
        _ = terminate => {
            info!("Received SIGTERM, initiating shutdown");
        }
    }
}
