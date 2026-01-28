---
context_files: []
output_dir: src/
output_file: main.rs
depends_on:
  - relayer_009_writers_evm
  - relayer_010_writers_terra
---

# Main Entry Point for CL8Y Bridge Relayer

## Requirements

Implement the main entry point that:
1. Loads configuration
2. Initializes database connection
3. Starts watchers and writers
4. Handles graceful shutdown on SIGINT/SIGTERM

## Module Declarations

```rust
mod config;
mod db;
mod types;
mod watchers;
mod writers;

use config::Config;
use watchers::WatcherManager;
use writers::WriterManager;
```

## Main Function

```rust
#[tokio::main]
async fn main() -> eyre::Result<()> {
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
    
    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let (shutdown_tx2, shutdown_rx2) = tokio::sync::mpsc::channel::<()>(1);
    
    // Setup signal handlers
    let shutdown_tx_signal = shutdown_tx.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        let _ = shutdown_tx_signal.send(()).await;
        let _ = shutdown_tx2.send(()).await;
    });
    
    // Create managers
    let watcher_manager = WatcherManager::new(&config, db.clone()).await?;
    let writer_manager = WriterManager::new(&config, db.clone()).await?;
    
    tracing::info!("Managers initialized, starting processing");
    
    // Run watchers and writers concurrently
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
    }
    
    tracing::info!("CL8Y Bridge Relayer stopped");
    Ok(())
}
```

## Logging Initialization

```rust
fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,cl8y_relayer=debug"));
    
    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true).with_thread_ids(true))
        .with(filter)
        .init();
}
```

## Shutdown Signal Handler

```rust
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
```

## Error Handling

```rust
// The main function should handle errors gracefully
// and provide useful error messages

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
    // ... main logic from above
}
```

## Constraints

- Use `tokio` as the async runtime with multi-threaded executor
- Use `tracing` and `tracing-subscriber` for structured logging
- Use `eyre` (or `color-eyre`) for error handling
- Handle SIGINT (Ctrl+C) and SIGTERM for graceful shutdown
- Use `dotenvy` to load .env file before config
- All errors should be logged before propagating
- No `unwrap()` calls except for signal handler setup
- Exit cleanly on shutdown signal

## Dependencies

```rust
use eyre::Result;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{info, error};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
```

## Cargo.toml Binary

Ensure the binary is defined in Cargo.toml:

```toml
[[bin]]
name = "cl8y-relayer"
path = "src/main.rs"
```
