pub mod address_codec;
mod api;
mod bounded_cache;
mod config;
mod confirmation;
mod contracts;
mod db;
mod discovery;
pub mod hash;
mod metrics;
mod multi_evm;
mod rpc_fallback;
mod terra_client;
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

    // Install a panic hook that logs via tracing BEFORE the default handler runs.
    // Rust panics only write to stderr; if stderr isn't captured in the same log
    // stream as tracing (stdout), panic messages are silently lost.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Box<dyn Any>".to_string()
        };
        // Log to tracing so it appears in the same log stream as everything else
        tracing::error!(
            panic.payload = %payload,
            panic.location = %location,
            "PANIC: task panicked — this will crash the operator"
        );
        default_hook(info);
    }));

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

    // Log multi-EVM configuration if present
    if let Some(ref multi) = config.multi_evm {
        let chain_ids = multi.chain_ids();
        tracing::info!(
            chains = chain_ids.len(),
            chain_ids = ?chain_ids,
            "Multi-EVM configuration loaded for EVM-to-EVM bridging"
        );
    }

    // Connect to database
    let db = db::create_pool(&config.database.url).await?;
    tracing::info!("Database connected");

    // Run migrations (can be skipped via SKIP_MIGRATIONS env var)
    if std::env::var("SKIP_MIGRATIONS").is_ok_and(|v| v == "1" || v.to_lowercase() == "true") {
        tracing::info!("Skipping database migrations (SKIP_MIGRATIONS=true)");
    } else {
        db::run_migrations(&db).await?;
        tracing::info!("Database migrations complete");
    }

    // Create shutdown channels
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx2, shutdown_rx2) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx3, shutdown_rx3) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx4, shutdown_rx4) = tokio::sync::mpsc::channel::<()>(1);

    // Setup signal handlers
    let shutdown_tx_signal = shutdown_tx.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx_signal.send(()).await;
        let _ = shutdown_tx2.send(()).await;
        let _ = shutdown_tx3.send(()).await;
        let _ = shutdown_tx4.send(()).await;
    });

    // Create managers
    let watcher_manager = WatcherManager::new(&config, db.clone()).await?;
    let mut writer_manager = WriterManager::new(&config, db.clone()).await?;
    let mut confirmation_tracker = ConfirmationTracker::new(&config, db.clone()).await?;

    tracing::info!("Managers initialized, starting processing");

    // Start metrics/API server
    // Default port 9092 — avoids conflict with LocalTerra gRPC (9090) and gRPC-web (9091)
    let api_port: u16 = std::env::var("OPERATOR_API_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9092);
    let api_bind: std::net::IpAddr = std::env::var("OPERATOR_API_BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1".to_string())
        .parse()
        .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));
    let api_addr = std::net::SocketAddr::from((api_bind, api_port));
    tracing::info!(port = api_port, "Starting API server");
    let api_db = db.clone();
    tokio::spawn(async move {
        if let Err(e) = api::start_api_server(api_addr, api_db).await {
            tracing::error!(error = %e, "API server error");
        }
    });

    // Start chain discovery task (runs on startup and every 4 hours)
    let discovery_config = config.clone();
    tokio::spawn(async move {
        if let Err(e) = discovery::run_discovery_task(&discovery_config, shutdown_rx4).await {
            tracing::error!(error = %e, "Chain discovery task error");
        }
    });

    // Run watchers, writers, and confirmation tracker concurrently.
    // Each branch is wrapped in spawn_blocking(catch_unwind) so a panic in one
    // task is logged and surfaced rather than silently killing the process.
    let watcher_handle = tokio::spawn(async move { watcher_manager.run(shutdown_rx).await });
    let writer_handle = tokio::spawn(async move { writer_manager.run(shutdown_rx2).await });
    let confirmation_handle =
        tokio::spawn(async move { confirmation_tracker.run(shutdown_rx3).await });

    tokio::select! {
        result = watcher_handle => {
            match result {
                Ok(Ok(())) => tracing::info!("Watcher manager exited cleanly"),
                Ok(Err(e)) => tracing::error!(error = %e, "Watcher manager error"),
                Err(e) => tracing::error!(error = %e, "Watcher manager task panicked"),
            }
        }
        result = writer_handle => {
            match result {
                Ok(Ok(())) => tracing::info!("Writer manager exited cleanly"),
                Ok(Err(e)) => tracing::error!(error = %e, "Writer manager error"),
                Err(e) => tracing::error!(error = %e, "Writer manager task panicked"),
            }
        }
        result = confirmation_handle => {
            match result {
                Ok(Ok(())) => tracing::info!("Confirmation tracker exited cleanly"),
                Ok(Err(e)) => tracing::error!(error = %e, "Confirmation tracker error"),
                Err(e) => tracing::error!(error = %e, "Confirmation tracker task panicked"),
            }
        }
    }

    tracing::info!("CL8Y Bridge Relayer stopped");

    // Flush stderr to ensure all log output is visible in cloud log collectors
    // (Render, Docker, systemd) before the process exits. Without this, the last
    // few lines (including error messages explaining WHY we exited) can be lost.
    use std::io::Write;
    let _ = std::io::stderr().flush();

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
