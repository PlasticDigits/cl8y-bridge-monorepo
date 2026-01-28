---
context_files: []
output_dir: src/watchers/
output_file: mod.rs
depends_on:
  - relayer_004_db_mod
---

# Watchers Module Coordinator

## Requirements

Implement the watchers module that coordinates EVM and Terra Classic event watching.
This module exports the watcher types and provides utilities for running them.

## Module Structure

```rust
pub mod evm;
pub mod terra;

pub use evm::EvmWatcher;
pub use terra::TerraWatcher;
```

## Coordinator Trait

```rust
use async_trait::async_trait;
use eyre::Result;
use tokio::sync::mpsc;

/// Trait for chain watchers
#[async_trait]
pub trait Watcher: Send + Sync {
    /// Start watching for events
    async fn start(&self, shutdown: mpsc::Receiver<()>) -> Result<()>;
    
    /// Get the chain identifier for logging
    fn chain_name(&self) -> &str;
}
```

## WatcherManager

```rust
use sqlx::PgPool;
use crate::config::Config;

/// Manages multiple chain watchers
pub struct WatcherManager {
    evm_watcher: EvmWatcher,
    terra_watcher: TerraWatcher,
}

impl WatcherManager {
    /// Create a new watcher manager
    pub async fn new(config: &Config, db: PgPool) -> Result<Self>;
    
    /// Run all watchers concurrently
    /// Returns when any watcher fails or shutdown signal received
    pub async fn run(&self, shutdown: mpsc::Receiver<()>) -> Result<()>;
}
```

## Implementation

```rust
impl WatcherManager {
    pub async fn new(config: &Config, db: PgPool) -> Result<Self> {
        let evm_watcher = EvmWatcher::new(&config.evm, db.clone()).await?;
        let terra_watcher = TerraWatcher::new(&config.terra, db).await?;
        
        Ok(Self {
            evm_watcher,
            terra_watcher,
        })
    }
    
    pub async fn run(&self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        // Use tokio::select! to run both watchers and handle shutdown
        tokio::select! {
            result = self.evm_watcher.run() => {
                tracing::error!("EVM watcher stopped: {:?}", result);
                result
            }
            result = self.terra_watcher.run() => {
                tracing::error!("Terra watcher stopped: {:?}", result);
                result
            }
            _ = shutdown.recv() => {
                tracing::info!("Shutdown signal received, stopping watchers");
                Ok(())
            }
        }
    }
}
```

## Constraints

- Use `async_trait` for async trait methods
- Use `tracing` for logging
- Use `tokio::select!` for concurrent execution
- Use `eyre::Result` for error handling
- No `unwrap()` calls
- Handle graceful shutdown via mpsc channel

## Dependencies

```rust
use async_trait::async_trait;
use eyre::Result;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{error, info};
```
