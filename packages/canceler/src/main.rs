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
//! - Logs verification results (cancel submission not yet implemented)
//!
//! # Future Development
//!
//! Full implementation will add:
//! - Actual cancel transaction submission
//! - Multi-chain support
//! - Stake/slashing integration
//! - Distributed coordination with other cancelers

mod config;
mod hash;
mod verifier;
mod watcher;

use config::Config;
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
        evm_rpc = %config.evm_rpc_url,
        terra_lcd = %config.terra_lcd_url,
        "Configuration loaded"
    );

    let mut watcher = CancelerWatcher::new(&config).await?;

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
