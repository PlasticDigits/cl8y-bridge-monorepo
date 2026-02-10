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

use std::sync::Arc;

use alloy::signers::local::PrivateKeySigner;
use canceler::config::Config;
use canceler::server::{self, CancelerStats, Metrics, SharedMetrics, SharedStats};
use canceler::watcher::CancelerWatcher;
use tokio::sync::RwLock;
use tracing::{info, warn};

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

    // Derive and log the EVM address from the private key
    // This helps verify the address matches the one granted CANCELER_ROLE
    let evm_address = derive_evm_address(&config.evm_private_key);
    info!(
        canceler_id = %config.canceler_id,
        evm_canceler_address = %evm_address,
        evm_rpc = %config.evm_rpc_url,
        evm_bridge = %config.evm_bridge_address,
        terra_lcd = %config.terra_lcd_url,
        health_port = %config.health_port,
        "Configuration loaded - ENSURE this address has CANCELER_ROLE on the bridge"
    );

    // Create shared stats for health endpoint
    let stats: SharedStats = Arc::new(RwLock::new(CancelerStats::default()));

    // Create Prometheus metrics
    let metrics: SharedMetrics = Arc::new(Metrics::new());

    // Start HTTP server for health endpoint
    let health_bind_address = config.health_bind_address.clone();
    let health_port = config.health_port;
    let server_stats = Arc::clone(&stats);
    let server_metrics = Arc::clone(&metrics);
    tokio::spawn(async move {
        if let Err(e) = server::start_server(&health_bind_address, health_port, server_stats, server_metrics).await {
            tracing::error!(error = %e, "Health server error");
        }
    });

    // Create the watcher with shared stats and metrics
    let mut watcher =
        CancelerWatcher::new(&config, Arc::clone(&stats), Arc::clone(&metrics)).await?;

    // Create shutdown channels (watcher and discovery each need one)
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx2, shutdown_rx2) = tokio::sync::mpsc::channel::<()>(1);

    // Handle signals
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx.send(()).await;
        let _ = shutdown_tx2.send(()).await;
    });

    // Run watcher and discovery concurrently
    tokio::select! {
        result = watcher.run(shutdown_rx) => {
            result?;
        }
        result = canceler::discovery::run_discovery_task(&config, shutdown_rx2) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Chain discovery task error");
            }
        }
    }

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

/// Derive EVM address from private key for logging
fn derive_evm_address(private_key: &str) -> String {
    match private_key.parse::<PrivateKeySigner>() {
        Ok(signer) => format!("{}", signer.address()),
        Err(e) => {
            warn!(error = %e, "Failed to parse EVM private key");
            "INVALID_KEY".to_string()
        }
    }
}
