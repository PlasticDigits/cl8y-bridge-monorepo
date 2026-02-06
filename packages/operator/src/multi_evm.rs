//! Multi-EVM Chain Configuration
//!
//! Supports multiple EVM chain configurations for EVM-to-EVM bridging.

#![allow(dead_code)]

use eyre::{eyre, Result};
use std::collections::HashMap;

use crate::types::ChainId;

/// Configuration for a single EVM chain
#[derive(Debug, Clone)]
pub struct EvmChainConfig {
    /// Human-readable name (e.g., "ethereum", "bsc")
    pub name: String,
    /// Chain ID (native EVM chain ID)
    pub chain_id: u64,
    /// 4-byte chain ID (V2 format)
    pub this_chain_id: ChainId,
    /// RPC endpoint
    pub rpc_url: String,
    /// Bridge contract address
    pub bridge_address: String,
    /// Required confirmations (default 12)
    pub finality_blocks: u64,
    /// Whether this chain is active
    pub enabled: bool,
}

impl Default for EvmChainConfig {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            chain_id: 0,
            this_chain_id: ChainId::from_u32(0),
            rpc_url: String::new(),
            bridge_address: String::new(),
            finality_blocks: 12,
            enabled: true,
        }
    }
}

/// Multi-EVM configuration manager
#[derive(Debug, Clone)]
pub struct MultiEvmConfig {
    chains: Vec<EvmChainConfig>,
    chain_id_map: HashMap<[u8; 4], usize>,
    private_key: String,
}

impl MultiEvmConfig {
    /// Create a new multi-EVM config
    pub fn new(chains: Vec<EvmChainConfig>, private_key: String) -> Result<Self> {
        let mut chain_id_map = HashMap::new();

        for (idx, chain) in chains.iter().enumerate() {
            chain_id_map.insert(chain.this_chain_id.0, idx);
        }

        let config = Self {
            chains,
            chain_id_map,
            private_key,
        };

        config.validate()?;
        Ok(config)
    }

    /// Get chain config by native EVM chain ID
    pub fn get_chain(&self, chain_id: u64) -> Option<&EvmChainConfig> {
        self.chains.iter().find(|c| c.chain_id == chain_id)
    }

    /// Get chain config by name
    pub fn get_chain_by_name(&self, name: &str) -> Option<&EvmChainConfig> {
        self.chains.iter().find(|c| c.name == name)
    }

    /// Get chain config by 4-byte chain ID (V2)
    pub fn get_chain_by_id(&self, id: &ChainId) -> Option<&EvmChainConfig> {
        self.chain_id_map.get(&id.0).map(|&idx| &self.chains[idx])
    }

    /// Get all enabled chains
    pub fn enabled_chains(&self) -> impl Iterator<Item = &EvmChainConfig> {
        self.chains.iter().filter(|c| c.enabled)
    }

    /// Get all chain IDs
    pub fn chain_ids(&self) -> Vec<u64> {
        self.chains.iter().map(|c| c.chain_id).collect()
    }

    /// Get the shared private key
    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if self.chains.is_empty() {
            return Err(eyre!("At least one EVM chain must be configured"));
        }

        // Check for duplicate chain IDs
        let mut seen_ids = std::collections::HashSet::new();
        for chain in &self.chains {
            if !seen_ids.insert(chain.chain_id) {
                return Err(eyre!("Duplicate chain ID: {}", chain.chain_id));
            }

            // Validate bridge address format
            if chain.bridge_address.len() != 42 || !chain.bridge_address.starts_with("0x") {
                return Err(eyre!(
                    "Invalid bridge address for chain {}: {}",
                    chain.name,
                    chain.bridge_address
                ));
            }
        }

        // Validate private key
        if self.private_key.len() != 66 || !self.private_key.starts_with("0x") {
            return Err(eyre!("Invalid private key format"));
        }

        Ok(())
    }
}

/// Load multi-EVM config from environment variables
pub fn load_from_env() -> Result<Option<MultiEvmConfig>> {
    let count_str = std::env::var("EVM_CHAINS_COUNT").ok();

    let count: usize = match count_str {
        Some(s) => s.parse().unwrap_or(0),
        None => return Ok(None), // Multi-EVM not configured
    };

    if count == 0 {
        return Ok(None);
    }

    let mut chains = Vec::with_capacity(count);

    for i in 1..=count {
        let prefix = format!("EVM_CHAIN_{}", i);

        let name =
            std::env::var(format!("{}_NAME", prefix)).unwrap_or_else(|_| format!("chain_{}", i));

        let chain_id: u64 = std::env::var(format!("{}_CHAIN_ID", prefix))
            .map_err(|_| eyre!("Missing {}_CHAIN_ID", prefix))?
            .parse()
            .map_err(|_| eyre!("Invalid {}_CHAIN_ID", prefix))?;

        // 4-byte chain ID (V2) - defaults to native chain ID as u32
        let this_chain_id: u32 = std::env::var(format!("{}_THIS_CHAIN_ID", prefix))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(chain_id as u32);

        let rpc_url = std::env::var(format!("{}_RPC_URL", prefix))
            .map_err(|_| eyre!("Missing {}_RPC_URL", prefix))?;

        let bridge_address = std::env::var(format!("{}_BRIDGE_ADDRESS", prefix))
            .map_err(|_| eyre!("Missing {}_BRIDGE_ADDRESS", prefix))?;

        let finality_blocks: u64 = std::env::var(format!("{}_FINALITY_BLOCKS", prefix))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(12);

        let enabled: bool = std::env::var(format!("{}_ENABLED", prefix))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);

        chains.push(EvmChainConfig {
            name,
            chain_id,
            this_chain_id: ChainId::from_u32(this_chain_id),
            rpc_url,
            bridge_address,
            finality_blocks,
            enabled,
        });
    }

    let private_key = std::env::var("EVM_PRIVATE_KEY")
        .map_err(|_| eyre!("Missing EVM_PRIVATE_KEY for multi-EVM config"))?;

    Ok(Some(MultiEvmConfig::new(chains, private_key)?))
}
