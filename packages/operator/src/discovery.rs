//! Chain discovery integration
//!
//! Queries the ChainRegistry on known EVM chains to discover all registered chains.
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
    let known_chains = build_known_chains(config)?;
    if known_chains.is_empty() {
        tracing::warn!("No EVM chains configured for discovery, skipping");
        return Ok(());
    }

    tracing::info!(
        chains = known_chains.len(),
        "Chain discovery starting (runs on startup and every 4 hours)"
    );

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

/// Build KnownChain list from operator config
pub(crate) fn build_known_chains(config: &Config) -> Result<Vec<KnownChain>> {
    let mut chains = Vec::new();

    // Primary EVM chain
    let bridge_addr = Address::from_str(&config.evm.bridge_address)
        .map_err(|e| eyre::eyre!("Invalid EVM_BRIDGE_ADDRESS: {}", e))?;
    chains.push(KnownChain {
        rpc_url: config.evm.rpc_url.clone(),
        bridge_address: bridge_addr,
        native_chain_id: config.evm.chain_id,
    });

    // Multi-EVM chains
    if let Some(ref multi) = config.multi_evm {
        for chain in multi.enabled_chains() {
            let bridge_addr = Address::from_str(&chain.bridge_address)
                .map_err(|e| eyre::eyre!("Invalid bridge_address for {}: {}", chain.name, e))?;
            chains.push(KnownChain {
                rpc_url: chain.rpc_url.clone(),
                bridge_address: bridge_addr,
                native_chain_id: chain.chain_id,
            });
        }
    }

    Ok(chains)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EvmConfig, FeeConfig, RelayerConfig};
    use crate::multi_evm::{EvmChainConfig, MultiEvmConfig};

    fn minimal_config() -> Config {
        Config {
            database: crate::config::DatabaseConfig {
                url: "postgres://localhost/test".to_string(),
            },
            evm: EvmConfig {
                rpc_url: "http://127.0.0.1:1".to_string(),
                rpc_fallback_urls: vec![],
                chain_id: 31337,
                bridge_address: "0x0000000000000000000000000000000000000001".to_string(),
                private_key: "0x0000000000000000000000000000000000000000000000000000000000000001"
                    .to_string(),
                finality_blocks: 1,
                this_chain_id: None,
                use_v2_events: None,
            },
            terra: crate::config::TerraConfig {
                rpc_url: "http://localhost:26657".to_string(),
                lcd_url: "http://localhost:1317".to_string(),
                chain_id: "localterra".to_string(),
                bridge_address: "terra1xxx".to_string(),
                mnemonic: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
                fee_recipient: None,
                this_chain_id: None,
            },
            relayer: RelayerConfig {
                poll_interval_ms: 1000,
                retry_attempts: 3,
                retry_delay_ms: 5000,
            },
            fees: FeeConfig {
                default_fee_bps: 30,
                fee_recipient: "0x0000000000000000000000000000000000000001".to_string(),
            },
            multi_evm: None,
        }
    }

    #[test]
    fn test_build_known_chains_primary_only() {
        let config = minimal_config();
        let chains = build_known_chains(&config).unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].rpc_url, config.evm.rpc_url);
        assert_eq!(chains[0].native_chain_id, 31337);
    }

    #[test]
    fn test_build_known_chains_with_multi_evm() {
        let mut config = minimal_config();
        let multi = MultiEvmConfig::new(
            vec![EvmChainConfig {
                name: "bsc".to_string(),
                chain_id: 56,
                this_chain_id: multichain_rs::types::ChainId::from_u32(2),
                rpc_url: "https://bsc-dataseed.binance.org".to_string(),
                rpc_fallback_urls: vec![],
                bridge_address: "0x0000000000000000000000000000000000000002".to_string(),
                finality_blocks: 12,
                enabled: true,
            }],
            "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
        )
        .unwrap();
        config.multi_evm = Some(multi);

        let chains = build_known_chains(&config).unwrap();
        assert_eq!(chains.len(), 2);
        assert_eq!(chains[0].native_chain_id, 31337);
        assert_eq!(chains[1].native_chain_id, 56);
        assert_eq!(chains[1].rpc_url, "https://bsc-dataseed.binance.org");
    }

    #[test]
    fn test_build_known_chains_invalid_bridge_address() {
        let mut config = minimal_config();
        config.evm.bridge_address = "invalid".to_string();
        let result = build_known_chains(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_discovery_task_exits_on_channel_close() {
        // Channel with dropped sender - recv returns None immediately after first loop iter
        let config = minimal_config();
        let (_tx, rx) = tokio::sync::mpsc::channel::<()>(1);
        drop(_tx);
        // run_discovery_task will run discovery_once (may fail/slow) then loop, recv will get None
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            run_discovery_task(&config, rx),
        )
        .await;
        assert!(result.is_ok(), "task should complete");
        assert!(
            result.unwrap().is_ok(),
            "run_discovery_task should return Ok"
        );
    }

    #[tokio::test]
    async fn test_run_discovery_task_receives_shutdown() {
        let config = minimal_config();
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let handle = tokio::spawn(async move { run_discovery_task(&config, rx).await });
        // Send shutdown immediately - task may still be in discovery_once, but will get it in loop
        let _ = tx.send(()).await;
        let result = tokio::time::timeout(std::time::Duration::from_secs(60), handle).await;
        assert!(result.is_ok(), "task should complete within timeout");
        assert!(
            result.unwrap().unwrap().is_ok(),
            "run_discovery_task should return Ok"
        );
    }
}
