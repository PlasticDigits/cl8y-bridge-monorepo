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

        // Deduplicate watchers: if the same native chain ID appears more than
        // once (e.g., in both primary EVM config and multi-EVM config), keep only
        // the first instance to avoid race conditions on DB writes.
        let mut seen_chain_ids = std::collections::HashSet::new();
        let before_count = evm_watchers.len();
        evm_watchers.retain(|w| seen_chain_ids.insert(w.chain_id()));
        if evm_watchers.len() < before_count {
            warn!(
                removed = before_count - evm_watchers.len(),
                "Removed duplicate EVM watchers — check if a chain appears in both \
                 EVM_CHAIN_ID and EVM_CHAINS config"
            );
        }
        info!(
            evm_watchers = evm_watchers.len(),
            evm_chain_ids = ?seen_chain_ids.into_iter().collect::<Vec<_>>(),
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
                let result = match maybe_done {
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
                };
                // Flush stderr so the error message above is visible in
                // block-buffered environments (Docker, Render, systemd pipes)
                // before the process exits.
                use std::io::Write;
                let _ = std::io::stderr().flush();
                result
            }
        }
    }
}
