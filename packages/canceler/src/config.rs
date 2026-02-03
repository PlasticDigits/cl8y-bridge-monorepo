//! Canceler configuration

use eyre::{eyre, Result};
use std::env;

/// Canceler configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// EVM RPC URL
    pub evm_rpc_url: String,
    /// EVM chain ID
    pub evm_chain_id: u64,
    /// EVM bridge contract address
    pub evm_bridge_address: String,
    /// EVM private key for cancel transactions
    pub evm_private_key: String,

    /// Terra LCD URL
    pub terra_lcd_url: String,
    /// Terra RPC URL
    pub terra_rpc_url: String,
    /// Terra chain ID
    pub terra_chain_id: String,
    /// Terra bridge contract address
    pub terra_bridge_address: String,
    /// Terra mnemonic for cancel transactions
    pub terra_mnemonic: String,

    /// Poll interval in milliseconds
    pub poll_interval_ms: u64,
}

impl Config {
    /// Load configuration from environment
    pub fn load() -> Result<Self> {
        // Try to load .env file
        if let Ok(path) = dotenvy::dotenv() {
            tracing::debug!("Loaded .env from {:?}", path);
        }

        Ok(Self {
            evm_rpc_url: env::var("EVM_RPC_URL").map_err(|_| eyre!("EVM_RPC_URL required"))?,
            evm_chain_id: env::var("EVM_CHAIN_ID")
                .map_err(|_| eyre!("EVM_CHAIN_ID required"))?
                .parse()
                .map_err(|_| eyre!("Invalid EVM_CHAIN_ID"))?,
            evm_bridge_address: env::var("EVM_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("EVM_BRIDGE_ADDRESS required"))?,
            evm_private_key: env::var("EVM_PRIVATE_KEY")
                .map_err(|_| eyre!("EVM_PRIVATE_KEY required"))?,

            terra_lcd_url: env::var("TERRA_LCD_URL")
                .map_err(|_| eyre!("TERRA_LCD_URL required"))?,
            terra_rpc_url: env::var("TERRA_RPC_URL")
                .map_err(|_| eyre!("TERRA_RPC_URL required"))?,
            terra_chain_id: env::var("TERRA_CHAIN_ID")
                .map_err(|_| eyre!("TERRA_CHAIN_ID required"))?,
            terra_bridge_address: env::var("TERRA_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("TERRA_BRIDGE_ADDRESS required"))?,
            terra_mnemonic: env::var("TERRA_MNEMONIC")
                .map_err(|_| eyre!("TERRA_MNEMONIC required"))?,

            poll_interval_ms: env::var("POLL_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
        })
    }
}
