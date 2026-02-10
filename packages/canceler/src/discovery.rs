//! Chain discovery integration
//!
//! Queries the ChainRegistry on the known EVM chain to discover all registered chains.
//! Runs on startup and periodically (every 4 hours) to detect newly registered chains.

use alloy::primitives::Address;
use eyre::Result;
use multichain_rs::discovery::{discover_chains, KnownChain};
use std::str::FromStr;
use tokio::sync::mpsc;

use crate::config::Config;

/// Interval between discovery runs (4 hours)
const DISCOVERY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(4 * 60 * 60);

/// Run chain discovery on startup and every 4 hours until shutdown.
pub async fn run_discovery_task(config: &Config, mut shutdown: mpsc::Receiver<()>) -> Result<()> {
    let known_chain = build_known_chain(config)?;

    tracing::info!("Chain discovery starting (runs on startup and every 4 hours)");

    let known_chains = [known_chain];

    // Run immediately on startup
    run_discovery_once(&known_chains).await;

    let mut interval = tokio::time::interval(DISCOVERY_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                tracing::info!("Chain discovery shutdown");
                break;
            }
            _ = interval.tick() => {
                run_discovery_once(&known_chains).await;
            }
        }
    }

    Ok(())
}

/// Build KnownChain from canceler config
pub(crate) fn build_known_chain(config: &Config) -> Result<KnownChain> {
    let bridge_addr = Address::from_str(&config.evm_bridge_address)
        .map_err(|e| eyre::eyre!("Invalid EVM_BRIDGE_ADDRESS: {}", e))?;
    Ok(KnownChain {
        rpc_url: config.evm_rpc_url.clone(),
        bridge_address: bridge_addr,
        native_chain_id: config.evm_chain_id,
    })
}

/// Run discovery once and log results
async fn run_discovery_once(known_chains: &[KnownChain]) {
    match discover_chains(known_chains).await {
        Ok(discovered) => {
            let chain_ids: Vec<String> = discovered.iter().map(|c| c.chain_id.to_hex()).collect();
            tracing::info!(
                count = discovered.len(),
                chain_ids = ?chain_ids,
                "Chain discovery complete"
            );
            for c in &discovered {
                tracing::debug!(
                    chain_id = %c.chain_id.to_hex(),
                    "Discovered chain"
                );
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Chain discovery failed");
        }
    }
}
