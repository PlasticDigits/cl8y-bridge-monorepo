use eyre::Result;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::Config;

pub mod evm;
pub mod terra;

pub use evm::EvmWatcher;
pub use terra::TerraWatcher;

/// Manages multiple chain watchers
pub struct WatcherManager {
    evm_watcher: EvmWatcher,
    terra_watcher: TerraWatcher,
}

impl WatcherManager {
    /// Create a new watcher manager
    pub async fn new(config: &Config, db: PgPool) -> Result<Self> {
        let evm_watcher = EvmWatcher::new(&config.evm, db.clone()).await?;
        let terra_watcher = TerraWatcher::new(&config.terra, db).await?;

        Ok(Self {
            evm_watcher,
            terra_watcher,
        })
    }

    /// Run all watchers concurrently
    /// Returns when any watcher fails or shutdown signal received
    pub async fn run(&self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        tokio::select! {
            result = self.evm_watcher.run() => {
                error!("EVM watcher stopped: {:?}", result);
                result
            }
            result = self.terra_watcher.run() => {
                error!("Terra watcher stopped: {:?}", result);
                result
            }
            _ = shutdown.recv() => {
                info!("Shutdown signal received, stopping watchers");
                Ok(())
            }
        }
    }
}
