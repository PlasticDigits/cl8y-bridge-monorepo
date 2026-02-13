//! Multi-EVM Chain Configuration
//!
//! Re-exports shared multi-EVM types from `multichain_rs::multi_evm` and adds
//! operator-specific extensions (e.g., converting to operator `EvmConfig`).

#![allow(dead_code)]

// Re-export shared types
pub use multichain_rs::multi_evm::{load_from_env, EvmChainConfig, MultiEvmConfig};

/// Extension trait for operator-specific functionality on `EvmChainConfig`.
pub trait EvmChainConfigExt {
    /// Convert to the operator's `EvmConfig` format.
    ///
    /// Requires the shared private key from `MultiEvmConfig` since
    /// individual chain configs don't store it.
    fn to_operator_evm_config(&self, private_key: &str) -> crate::config::EvmConfig;
}

impl EvmChainConfigExt for EvmChainConfig {
    fn to_operator_evm_config(&self, private_key: &str) -> crate::config::EvmConfig {
        crate::config::EvmConfig {
            rpc_url: self.rpc_url.clone(),
            chain_id: self.chain_id,
            bridge_address: self.bridge_address.clone(),
            private_key: private_key.to_string(),
            finality_blocks: self.finality_blocks,
            this_chain_id: Some(self.this_chain_id.to_u32()),
            use_v2_events: Some(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multichain_rs::types::ChainId;

    #[test]
    fn test_to_operator_evm_config() {
        let chain = EvmChainConfig {
            name: "anvil".to_string(),
            chain_id: 31337,
            this_chain_id: ChainId::from_u32(1),
            rpc_url: "http://localhost:8545".to_string(),
            bridge_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
            finality_blocks: 0,
            enabled: true,
        };
        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let config = chain.to_operator_evm_config(pk);
        assert_eq!(config.chain_id, 31337);
        assert_eq!(config.this_chain_id, Some(1));
        assert_eq!(config.use_v2_events, Some(true));
    }

    #[test]
    fn test_multi_evm_config_creation() {
        let chains = vec![
            EvmChainConfig {
                name: "anvil".to_string(),
                chain_id: 31337,
                this_chain_id: ChainId::from_u32(1),
                rpc_url: "http://localhost:8545".to_string(),
                bridge_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
                finality_blocks: 0,
                enabled: true,
            },
            EvmChainConfig {
                name: "anvil1".to_string(),
                chain_id: 31338,
                this_chain_id: ChainId::from_u32(3),
                rpc_url: "http://localhost:8546".to_string(),
                bridge_address: "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
                finality_blocks: 0,
                enabled: true,
            },
        ];

        let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let config = MultiEvmConfig::new(chains, pk.to_string()).unwrap();
        assert_eq!(config.enabled_count(), 2);
        assert!(config.get_chain(31337).is_some());
        assert!(config.get_chain(31338).is_some());
    }
}
