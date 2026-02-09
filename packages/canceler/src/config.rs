//! Canceler configuration

use eyre::{eyre, Result};
use std::env;

/// Canceler configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Unique canceler instance ID for multi-canceler deployments
    pub canceler_id: String,

    /// EVM RPC URL
    pub evm_rpc_url: String,
    /// EVM native chain ID (e.g. 31337 for Anvil)
    pub evm_chain_id: u64,
    /// EVM bridge contract address
    pub evm_bridge_address: String,
    /// EVM private key for cancel transactions
    pub evm_private_key: String,

    /// V2 registered chain ID for this EVM chain (bytes4, e.g. 0x00000001).
    /// This is the chain ID assigned by ChainRegistry, NOT the native chain ID.
    /// If not set, falls back to querying the bridge contract's getThisChainId().
    pub evm_v2_chain_id: Option<[u8; 4]>,

    /// V2 registered chain ID for Terra (bytes4, e.g. 0x00000002).
    /// This is the chain ID assigned by ChainRegistry for the Terra chain.
    /// If not set, defaults to querying or using a hardcoded mapping.
    pub terra_v2_chain_id: Option<[u8; 4]>,

    /// Terra LCD URL
    pub terra_lcd_url: String,
    /// Terra RPC URL (reserved for future WebSocket support)
    #[allow(dead_code)]
    pub terra_rpc_url: String,
    /// Terra chain ID
    pub terra_chain_id: String,
    /// Terra bridge contract address
    pub terra_bridge_address: String,
    /// Terra mnemonic for cancel transactions
    pub terra_mnemonic: String,

    /// Poll interval in milliseconds
    pub poll_interval_ms: u64,

    /// Health server port (default 9090)
    pub health_port: u16,
}

impl Config {
    /// Load configuration from environment
    pub fn load() -> Result<Self> {
        // Try to load .env file
        if let Ok(path) = dotenvy::dotenv() {
            tracing::debug!("Loaded .env from {:?}", path);
        }

        // Generate default canceler ID from hostname or random
        let default_id = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| format!("canceler-{}", std::process::id()));

        // Parse V2 chain IDs (e.g. "0x00000001" or "1")
        let evm_v2_chain_id = env::var("EVM_V2_CHAIN_ID").ok().and_then(|s| {
            let s = s.trim().trim_start_matches("0x");
            if let Ok(n) = u32::from_str_radix(s, 16) {
                Some(n.to_be_bytes())
            } else if let Ok(n) = s.parse::<u32>() {
                Some(n.to_be_bytes())
            } else {
                None
            }
        });

        let terra_v2_chain_id = env::var("TERRA_V2_CHAIN_ID").ok().and_then(|s| {
            let s = s.trim().trim_start_matches("0x");
            if let Ok(n) = u32::from_str_radix(s, 16) {
                Some(n.to_be_bytes())
            } else if let Ok(n) = s.parse::<u32>() {
                Some(n.to_be_bytes())
            } else {
                None
            }
        });

        Ok(Self {
            canceler_id: env::var("CANCELER_ID").unwrap_or(default_id),

            evm_rpc_url: env::var("EVM_RPC_URL").map_err(|_| eyre!("EVM_RPC_URL required"))?,
            evm_chain_id: env::var("EVM_CHAIN_ID")
                .map_err(|_| eyre!("EVM_CHAIN_ID required"))?
                .parse()
                .map_err(|_| eyre!("Invalid EVM_CHAIN_ID"))?,
            evm_bridge_address: env::var("EVM_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("EVM_BRIDGE_ADDRESS required"))?,
            evm_private_key: env::var("EVM_PRIVATE_KEY")
                .map_err(|_| eyre!("EVM_PRIVATE_KEY required"))?,

            evm_v2_chain_id,
            terra_v2_chain_id,

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

            // Default health port 9099 — avoids conflict with LocalTerra gRPC (9090),
            // gRPC-web (9091), and operator API (9092)
            health_port: env::var("HEALTH_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9099),
        })
    }
}
