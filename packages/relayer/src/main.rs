mod api;
mod config;
mod confirmation;
mod contracts;
mod db;
mod metrics;
mod multi_evm;
mod types;
mod watchers;
mod writers;

use config::Config;
use confirmation::ConfirmationTracker;
use watchers::WatcherManager;
use writers::WriterManager;

fn main() -> eyre::Result<()> {
    // Install color-eyre for better error reporting
    color_eyre::install()?;

    // Run the async main
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main())
}

async fn async_main() -> eyre::Result<()> {
    // Initialize logging
    init_logging();
    
    tracing::info!("Starting CL8Y Bridge Relayer");
    
    // Load configuration
    let config = Config::load()?;
    tracing::info!(
        evm_chain_id = config.evm.chain_id,
        terra_chain_id = %config.terra.chain_id,
        "Configuration loaded"
    );
    
    // Connect to database
    let db = db::create_pool(&config.database.url).await?;
    tracing::info!("Database connected");
    
    // Run migrations
    db::run_migrations(&db).await?;
    tracing::info!("Database migrations complete");
    
    // Create shutdown channels
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx2, shutdown_rx2) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx3, shutdown_rx3) = tokio::sync::mpsc::channel::<()>(1);
    
    // Setup signal handlers
    let shutdown_tx_signal = shutdown_tx.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx_signal.send(()).await;
        let _ = shutdown_tx2.send(()).await;
        let _ = shutdown_tx3.send(()).await;
    });
    
    // Create managers
    let watcher_manager = WatcherManager::new(&config, db.clone()).await?;
    let mut writer_manager = WriterManager::new(&config, db.clone()).await?;
    let mut confirmation_tracker = ConfirmationTracker::new(&config, db.clone()).await?;
    
    tracing::info!("Managers initialized, starting processing");
    
    // Start metrics/API server
    let api_addr = std::net::SocketAddr::from(([0, 0, 0, 0], 9090));
    let api_db = db.clone();
    tokio::spawn(async move {
        if let Err(e) = api::start_api_server(api_addr, api_db).await {
            tracing::error!(error = %e, "API server error");
        }
    });
    
    // Run watchers, writers, and confirmation tracker concurrently
    tokio::select! {
        result = watcher_manager.run(shutdown_rx) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Watcher manager error");
            }
        }
        result = writer_manager.run(shutdown_rx2) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Writer manager error");
            }
        }
        result = confirmation_tracker.run(shutdown_rx3) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Confirmation tracker error");
            }
        }
    }
    
    tracing::info!("CL8Y Bridge Relayer stopped");
    Ok(())
}

/// Initialize tracing/logging with structured output
fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,cl8y_relayer=debug"));
    
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true).with_thread_ids(true))
        .with(filter)
        .init();
}

/// Wait for shutdown signals (SIGINT/SIGTERM)
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
            tracing::info!("Received Ctrl+C, initiating shutdown");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating shutdown");
        }
    }
}