#![allow(dead_code)]

use eyre::{eyre, Result, WrapErr};
use serde::Deserialize;
use std::env;
use std::fmt;
use std::path::Path;

use crate::multi_evm::MultiEvmConfig;

/// Main configuration for the relayer
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub evm: EvmConfig,
    pub terra: TerraConfig,
    pub relayer: RelayerConfig,
    pub fees: FeeConfig,
    /// Optional multi-EVM chain configuration for EVM-to-EVM bridging.
    /// When set, the operator can handle deposits between multiple EVM chains
    /// (e.g., BSC→opBNB, ETH→Polygon). Loaded from EVM_CHAINS_COUNT env vars.
    #[serde(skip)]
    pub multi_evm: Option<MultiEvmConfig>,
}

/// Database configuration
#[derive(Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

/// Custom Debug that redacts the database URL (may contain credentials).
impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("url", &"<redacted>")
            .finish()
    }
}

/// EVM configuration
#[derive(Clone, Deserialize)]
pub struct EvmConfig {
    pub rpc_url: String,
    /// Additional RPC URLs for fallback (tried in order when primary fails)
    #[serde(default)]
    pub rpc_fallback_urls: Vec<String>,
    pub chain_id: u64,
    pub bridge_address: String,
    pub private_key: String,
    #[serde(default = "default_finality_blocks")]
    pub finality_blocks: u64,
    /// This chain's registered chain ID (4-byte V2 format)
    /// If not set, defaults to 1
    #[serde(default)]
    pub this_chain_id: Option<u32>,
    /// Use V2 event format (Deposit instead of DepositRequest)
    /// Defaults to true for new deployments
    #[serde(default)]
    pub use_v2_events: Option<bool>,
}

/// Custom Debug that redacts private_key to prevent accidental log leakage.
impl fmt::Debug for EvmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvmConfig")
            .field("rpc_url", &self.rpc_url)
            .field("rpc_fallback_urls", &self.rpc_fallback_urls)
            .field("chain_id", &self.chain_id)
            .field("bridge_address", &self.bridge_address)
            .field("private_key", &"<redacted>")
            .field("finality_blocks", &self.finality_blocks)
            .field("this_chain_id", &self.this_chain_id)
            .field("use_v2_events", &self.use_v2_events)
            .finish()
    }
}

impl EvmConfig {
    /// All RPC URLs: primary followed by fallbacks.
    pub fn all_rpc_urls(&self) -> Vec<String> {
        let mut urls = vec![self.rpc_url.clone()];
        urls.extend(self.rpc_fallback_urls.iter().cloned());
        urls
    }
}

/// Terra configuration
#[derive(Clone, Deserialize)]
pub struct TerraConfig {
    pub rpc_url: String,
    pub lcd_url: String,
    pub chain_id: String,
    pub bridge_address: String,
    pub mnemonic: String,
    /// Optional fee recipient address for Terra withdrawals
    #[serde(default)]
    pub fee_recipient: Option<String>,
    /// This chain's registered chain ID (4-byte V2 format)
    /// If not set, will be queried from contract or default to 4 (terraclassic_columbus-5)
    #[serde(default)]
    pub this_chain_id: Option<u32>,
}

/// Custom Debug that redacts mnemonic to prevent accidental log leakage.
impl fmt::Debug for TerraConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TerraConfig")
            .field("rpc_url", &self.rpc_url)
            .field("lcd_url", &self.lcd_url)
            .field("chain_id", &self.chain_id)
            .field("bridge_address", &self.bridge_address)
            .field("mnemonic", &"<redacted>")
            .field("fee_recipient", &self.fee_recipient)
            .field("this_chain_id", &self.this_chain_id)
            .finish()
    }
}

/// Relayer configuration
#[derive(Debug, Clone, Deserialize)]
pub struct RelayerConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
}

/// Fee configuration
#[derive(Debug, Clone, Deserialize)]
pub struct FeeConfig {
    #[serde(default = "default_fee_bps")]
    pub default_fee_bps: u32,
    pub fee_recipient: String,
}

/// Default functions
fn default_finality_blocks() -> u64 {
    1
}

fn default_poll_interval() -> u64 {
    1000
}

fn default_retry_attempts() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    5000
}

fn default_fee_bps() -> u32 {
    30
}

impl Config {
    /// Load configuration from environment variables
    /// Loads .env file if present, then reads from environment
    pub fn load() -> Result<Self> {
        Self::load_from_file(".env").or_else(|_| Self::load_from_env())
    }

    /// Load from a specific .env file path
    pub fn load_from_file(path: &str) -> Result<Self> {
        if Path::new(path).exists() {
            dotenvy::from_filename(path)
                .wrap_err_with(|| format!("Failed to load .env file from {}", path))?;
        }
        Self::load_from_env()
    }

    /// Load configuration from environment variables
    fn load_from_env() -> Result<Self> {
        let database = DatabaseConfig {
            url: env::var("DATABASE_URL")
                .map_err(|_| eyre!("DATABASE_URL environment variable is required"))?,
        };

        let evm_rpc_raw = env::var("EVM_RPC_URL")
            .map_err(|_| eyre!("EVM_RPC_URL environment variable is required"))?;
        let evm_rpc_urls = crate::rpc_fallback::parse_rpc_urls(&evm_rpc_raw);
        if evm_rpc_urls.is_empty() {
            return Err(eyre!("EVM_RPC_URL cannot be empty"));
        }

        let evm = EvmConfig {
            rpc_url: evm_rpc_urls[0].clone(),
            rpc_fallback_urls: evm_rpc_urls[1..].to_vec(),
            chain_id: env::var("EVM_CHAIN_ID")
                .map_err(|_| eyre!("EVM_CHAIN_ID environment variable is required"))?
                .parse()
                .wrap_err("EVM_CHAIN_ID must be a valid u64")?,
            bridge_address: env::var("EVM_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("EVM_BRIDGE_ADDRESS environment variable is required"))?,
            private_key: env::var("EVM_PRIVATE_KEY")
                .map_err(|_| eyre!("EVM_PRIVATE_KEY environment variable is required"))?,
            finality_blocks: env::var("FINALITY_BLOCKS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_finality_blocks()),
            // V2 configuration
            this_chain_id: env::var("EVM_THIS_CHAIN_ID")
                .ok()
                .and_then(|v| v.parse().ok()),
            use_v2_events: env::var("EVM_USE_V2_EVENTS")
                .ok()
                .and_then(|v| v.parse().ok()),
        };

        let terra = TerraConfig {
            rpc_url: env::var("TERRA_RPC_URL")
                .map_err(|_| eyre!("TERRA_RPC_URL environment variable is required"))?,
            lcd_url: env::var("TERRA_LCD_URL")
                .map_err(|_| eyre!("TERRA_LCD_URL environment variable is required"))?,
            chain_id: env::var("TERRA_CHAIN_ID")
                .map_err(|_| eyre!("TERRA_CHAIN_ID environment variable is required"))?,
            bridge_address: env::var("TERRA_BRIDGE_ADDRESS")
                .map_err(|_| eyre!("TERRA_BRIDGE_ADDRESS environment variable is required"))?,
            mnemonic: env::var("TERRA_MNEMONIC")
                .map_err(|_| eyre!("TERRA_MNEMONIC environment variable is required"))?,
            fee_recipient: env::var("TERRA_FEE_RECIPIENT").ok(),
            // V2 configuration
            this_chain_id: env::var("TERRA_THIS_CHAIN_ID")
                .ok()
                .and_then(|v| v.parse().ok()),
        };

        let relayer = RelayerConfig {
            poll_interval_ms: env::var("POLL_INTERVAL_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_poll_interval()),
            retry_attempts: env::var("RETRY_ATTEMPTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_retry_attempts()),
            retry_delay_ms: env::var("RETRY_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_retry_delay()),
        };

        let fees = FeeConfig {
            default_fee_bps: env::var("DEFAULT_FEE_BPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default_fee_bps()),
            fee_recipient: env::var("FEE_RECIPIENT")
                .map_err(|_| eyre!("FEE_RECIPIENT environment variable is required"))?,
        };

        // Load optional multi-EVM configuration (for EVM-to-EVM bridging)
        let multi_evm = crate::multi_evm::load_from_env()?;

        let config = Config {
            database,
            evm,
            terra,
            relayer,
            fees,
            multi_evm,
        };

        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        // Validate database URL
        if self.database.url.is_empty() {
            return Err(eyre!("database.url cannot be empty"));
        }

        // Validate EVM RPC URL
        if self.evm.rpc_url.is_empty() {
            return Err(eyre!("evm.rpc_url cannot be empty"));
        }

        // Validate EVM bridge address
        if self.evm.bridge_address.len() != 42 || !self.evm.bridge_address.starts_with("0x") {
            return Err(eyre!(
                "evm.bridge_address must be a valid hex address (42 chars with 0x prefix)"
            ));
        }

        // Validate EVM private key
        if self.evm.private_key.len() != 66 || !self.evm.private_key.starts_with("0x") {
            return Err(eyre!(
                "evm.private_key must be 66 chars (0x + 64 hex chars)"
            ));
        }

        // Validate Terra RPC URL
        if self.terra.rpc_url.is_empty() {
            return Err(eyre!("terra.rpc_url cannot be empty"));
        }

        // Validate Terra LCD URL
        if self.terra.lcd_url.is_empty() {
            return Err(eyre!("terra.lcd_url cannot be empty"));
        }

        // Validate Terra chain ID
        if self.terra.chain_id.is_empty() {
            return Err(eyre!("terra.chain_id cannot be empty"));
        }

        // Validate Terra bridge address
        if self.terra.bridge_address.is_empty() {
            return Err(eyre!("terra.bridge_address cannot be empty"));
        }

        // Validate Terra mnemonic
        let mnemonic_words: Vec<&str> = self.terra.mnemonic.split_whitespace().collect();
        if mnemonic_words.len() < 12 {
            return Err(eyre!("terra.mnemonic must have at least 12 words"));
        }

        // Validate fee recipient
        if self.fees.fee_recipient.len() != 42 || !self.fees.fee_recipient.starts_with("0x") {
            return Err(eyre!(
                "fees.fee_recipient must be a valid EVM address (42 chars with 0x prefix)"
            ));
        }

        // Validate fee BPS is reasonable
        if self.fees.default_fee_bps > 100 {
            return Err(eyre!("fees.default_fee_bps cannot exceed 100"));
        }

        // Reject duplicate chain IDs across primary EVM and multi-EVM configs.
        // Running two watchers for the same chain causes race conditions on DB
        // writes (duplicate deposits, cursor conflicts) and will crash the operator.
        if let Some(ref multi) = self.multi_evm {
            for chain in multi.enabled_chains() {
                if chain.chain_id == self.evm.chain_id {
                    return Err(eyre!(
                        "FATAL: EVM chain {} appears in both EVM_CHAIN_ID and EVM_CHAINS. \
                         This creates duplicate watchers that race on DB writes and crash the operator. \
                         Remove it from one of the two configs.",
                        chain.chain_id
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_finality_blocks() {
        assert_eq!(default_finality_blocks(), 1);
    }

    #[test]
    fn test_default_poll_interval() {
        assert_eq!(default_poll_interval(), 1000);
    }

    #[test]
    fn test_default_retry_attempts() {
        assert_eq!(default_retry_attempts(), 3);
    }

    #[test]
    fn test_default_retry_delay() {
        assert_eq!(default_retry_delay(), 5000);
    }

    #[test]
    fn test_default_fee_bps() {
        assert_eq!(default_fee_bps(), 30);
    }

    #[test]
    fn test_evm_address_validation() {
        let mut config = Config {
            database: DatabaseConfig {
                url: "postgres://localhost/test".to_string(),
            },
            evm: EvmConfig {
                rpc_url: "http://localhost:8545".to_string(),
                rpc_fallback_urls: vec![],
                chain_id: 1,
                bridge_address: "0x0000000000000000000000000000000000000001".to_string(),
                private_key: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
                finality_blocks: 1,
                this_chain_id: None,
                use_v2_events: None,
            },
            terra: TerraConfig {
                rpc_url: "http://localhost:1317".to_string(),
                lcd_url: "http://localhost:1316".to_string(),
                chain_id: "columbus-5".to_string(),
                bridge_address: "terra1...".to_string(),
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
        };

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid private key length
        config.evm.private_key = "0x123".to_string();
        assert!(config.validate().is_err());

        // Invalid bridge address
        config.evm.private_key =
            "0x0000000000000000000000000000000000000000000000000000000000000001".to_string();
        config.evm.bridge_address = "invalid".to_string();
        assert!(config.validate().is_err());

        // Invalid fee recipient
        config.evm.bridge_address = "0x0000000000000000000000000000000000000001".to_string();
        config.fees.fee_recipient = "invalid".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_fee_bps_validation() {
        let mut config = Config {
            database: DatabaseConfig {
                url: "postgres://localhost/test".to_string(),
            },
            evm: EvmConfig {
                rpc_url: "http://localhost:8545".to_string(),
                rpc_fallback_urls: vec![],
                chain_id: 1,
                bridge_address: "0x0000000000000000000000000000000000000001".to_string(),
                private_key: "0x0000000000000000000000000000000000000000000000000000000000000001".to_string(),
                finality_blocks: 1,
                this_chain_id: None,
                use_v2_events: None,
            },
            terra: TerraConfig {
                rpc_url: "http://localhost:1317".to_string(),
                lcd_url: "http://localhost:1316".to_string(),
                chain_id: "columbus-5".to_string(),
                bridge_address: "terra1...".to_string(),
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
        };

        // Valid fee BPS
        assert!(config.validate().is_ok());

        // Fee BPS > 100 should fail
        config.fees.default_fee_bps = 101;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_duplicate_chain_id_rejected() {
        use multichain_rs::multi_evm::{EvmChainConfig, MultiEvmConfig};
        use multichain_rs::types::ChainId;

        let mut config = Config {
            database: DatabaseConfig {
                url: "postgres://localhost/test".to_string(),
            },
            evm: EvmConfig {
                rpc_url: "http://localhost:8545".to_string(),
                rpc_fallback_urls: vec![],
                chain_id: 204,
                bridge_address: "0x0000000000000000000000000000000000000001".to_string(),
                private_key:
                    "0x0000000000000000000000000000000000000000000000000000000000000001"
                        .to_string(),
                finality_blocks: 1,
                this_chain_id: Some(2),
                use_v2_events: None,
            },
            terra: TerraConfig {
                rpc_url: "http://localhost:1317".to_string(),
                lcd_url: "http://localhost:1316".to_string(),
                chain_id: "columbus-5".to_string(),
                bridge_address: "terra1...".to_string(),
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
        };

        // No multi-EVM — should pass
        assert!(config.validate().is_ok());

        // Multi-EVM with different chain — should pass
        let chains = vec![EvmChainConfig {
            name: "bsc".to_string(),
            chain_id: 56,
            this_chain_id: ChainId::from_u32(3),
            rpc_url: "http://localhost:8546".to_string(),
            rpc_fallback_urls: vec![],
            bridge_address: "0x0000000000000000000000000000000000000002".to_string(),
            finality_blocks: 0,
            enabled: true,
        }];
        let pk = "0x0000000000000000000000000000000000000000000000000000000000000001";
        config.multi_evm = Some(MultiEvmConfig::new(chains, pk.to_string()).unwrap());
        assert!(config.validate().is_ok());

        // Multi-EVM with DUPLICATE chain 204 — must fail
        let chains_dup = vec![EvmChainConfig {
            name: "opbnb".to_string(),
            chain_id: 204,
            this_chain_id: ChainId::from_u32(2),
            rpc_url: "http://localhost:8547".to_string(),
            rpc_fallback_urls: vec![],
            bridge_address: "0x0000000000000000000000000000000000000003".to_string(),
            finality_blocks: 0,
            enabled: true,
        }];
        config.multi_evm = Some(MultiEvmConfig::new(chains_dup, pk.to_string()).unwrap());
        let err = config.validate().unwrap_err();
        assert!(
            err.to_string().contains("204"),
            "Error should mention the duplicate chain ID: {}",
            err
        );
    }
}
