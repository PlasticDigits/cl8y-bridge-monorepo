use eyre::Result;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::multi_evm::EvmChainConfigExt;

pub mod evm;
pub mod terra;

pub use evm::EvmWatcher;
pub use terra::TerraWatcher;

/// Manages multiple chain watchers
pub struct WatcherManager {
    evm_watchers: Vec<EvmWatcher>,
    terra_watcher: TerraWatcher,
}

impl WatcherManager {
    /// Create a new watcher manager
    pub async fn new(config: &Config, db: PgPool) -> Result<Self> {
        let mut evm_watchers = Vec::new();
        evm_watchers.push(EvmWatcher::new(&config.evm, db.clone()).await?);

        // Add watchers for all enabled EVM peers (if configured).
        // This is required for routes like Anvil1 -> Terra where the source
        // deposit originates on a different EVM chain.
        if let Some(ref multi) = config.multi_evm {
            for chain in multi.enabled_chains() {
                let chain_evm_config = chain.to_operator_evm_config(multi.private_key());
                match EvmWatcher::new(&chain_evm_config, db.clone()).await {
                    Ok(watcher) => {
                        info!(
                            chain_name = %chain.name,
                            chain_id = chain.chain_id,
                            "Created EVM watcher for multi-EVM source chain"
                        );
                        evm_watchers.push(watcher);
                    }
                    Err(e) => {
                        warn!(
                            chain_name = %chain.name,
                            chain_id = chain.chain_id,
                            error = %e,
                            "Failed to create EVM watcher for multi-EVM chain; continuing without it"
                        );
                    }
                }
            }
        }

        let terra_watcher = TerraWatcher::new(&config.terra, db).await?;

        // Detect duplicate watchers (misconfiguration where the same chain
        // appears in both primary EVM config and multi-EVM config)
        let mut seen_chain_ids = std::collections::HashMap::new();
        for watcher in &evm_watchers {
            let chain_id = watcher.chain_id();
            *seen_chain_ids.entry(chain_id).or_insert(0u32) += 1;
        }
        for (&chain_id, &count) in &seen_chain_ids {
            if count > 1 {
                warn!(
                    chain_id,
                    count,
                    "DUPLICATE EVM watcher detected — chain appears {} times! \
                     This wastes resources and may cause race conditions. \
                     Check if chain {} is in both EVM_CHAIN_ID and EVM_CHAINS config.",
                    count,
                    chain_id
                );
            }
        }
        info!(
            evm_watchers = evm_watchers.len(),
            evm_chain_ids = ?seen_chain_ids.keys().collect::<Vec<_>>(),
            "Watcher manager created"
        );

        Ok(Self {
            evm_watchers,
            terra_watcher,
        })
    }

    /// Run all watchers concurrently
    /// Returns when any watcher fails or shutdown signal received
    pub async fn run(self, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
        let mut join_set = tokio::task::JoinSet::new();

        for evm_watcher in self.evm_watchers {
            join_set.spawn(async move { evm_watcher.run().await });
        }
        join_set.spawn(async move { self.terra_watcher.run().await });

        tokio::select! {
            _ = shutdown.recv() => {
                info!("Shutdown signal received, stopping watchers");
                join_set.abort_all();
                Ok(())
            }
            maybe_done = join_set.join_next() => {
                match maybe_done {
                    Some(Ok(Ok(()))) => {
                        error!("A watcher exited unexpectedly without error");
                        Err(eyre::eyre!("watcher exited unexpectedly"))
                    }
                    Some(Ok(Err(e))) => {
                        error!("A watcher stopped with error: {:?}", e);
                        Err(e)
                    }
                    Some(Err(e)) => {
                        error!("A watcher task panicked: {:?}", e);
                        Err(eyre::eyre!("watcher task panicked: {}", e))
                    }
                    None => {
                        error!("All watcher tasks exited unexpectedly");
                        Err(eyre::eyre!("all watcher tasks exited unexpectedly"))
                    }
                }
            }
        }
    }
}
