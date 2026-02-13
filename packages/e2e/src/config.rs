//! Configuration for E2E tests
//!
//! This module provides typed configuration that replaces bash environment variables
//! with compile-time safe structs.

use alloy::primitives::{Address, B256};
use eyre::{eyre, Result};
use serde::Deserialize;
use std::path::Path;
use url::Url;

/// Root configuration for E2E tests
#[derive(Debug, Clone, Default)]
pub struct E2eConfig {
    pub evm: EvmConfig,
    /// Secondary EVM chain (anvil1, V2 chain ID = 3)
    pub evm2: Option<EvmConfig>,
    pub terra: TerraConfig,
    pub docker: DockerConfig,
    pub operator: OperatorConfig,
    pub test_accounts: TestAccounts,
}

impl E2eConfig {
    /// Load configuration from environment variables
    ///
    /// The secondary EVM chain (`evm2`) is auto-configured by default because
    /// `anvil1` is always started by docker-compose. Contract addresses start
    /// as `Address::ZERO` and are populated during setup after forge deploys.
    ///
    /// To disable evm2, set `EVM2_DISABLED=1`.
    pub fn from_env() -> Result<Self> {
        // Load secondary EVM config:
        // - If EVM2_RPC_URL is explicitly set, use that
        // - If EVM2_DISABLED is set, skip evm2
        // - Otherwise, auto-detect with defaults (anvil1 on port 8546)
        let evm2_disabled = std::env::var("EVM2_DISABLED")
            .ok()
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        let evm2 = if evm2_disabled {
            None
        } else if std::env::var("EVM2_RPC_URL").is_ok() {
            Some(EvmConfig::from_env_with_prefix("EVM2")?)
        } else {
            // Auto-detect: anvil1 is always in docker-compose on port 8546.
            // Contract addresses start as ZERO and get populated during setup
            // after forge script deploys to the secondary chain.
            Some(EvmConfig {
                rpc_url: Url::parse("http://localhost:8546")?,
                chain_id: 31338,
                v2_chain_id: 3,
                private_key: B256::ZERO,
                contracts: EvmContracts::default(),
            })
        };

        Ok(Self {
            evm: EvmConfig::from_env()?,
            evm2,
            terra: TerraConfig::from_env()?,
            docker: DockerConfig::from_env()?,
            operator: OperatorConfig::from_env()?,
            test_accounts: TestAccounts::from_env()?,
        })
    }

    /// Parse a forge broadcast JSON file to extract deployed contract addresses
    pub fn from_broadcast(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let broadcast: BroadcastFile = serde_json::from_str(&content)?;

        let contracts = EvmContracts::from_broadcast(&broadcast)?;

        Ok(Self {
            evm: EvmConfig {
                contracts,
                ..EvmConfig::default()
            },
            ..Self::default()
        })
    }
}

/// EVM chain configuration
#[derive(Debug, Clone)]
pub struct EvmConfig {
    pub rpc_url: Url,
    pub chain_id: u64,
    /// V2 4-byte chain ID from ChainRegistry (NOT the native chain ID)
    pub v2_chain_id: u32,
    pub private_key: B256,
    pub contracts: EvmContracts,
}

impl Default for EvmConfig {
    fn default() -> Self {
        Self {
            rpc_url: Url::parse("http://localhost:8545").expect("valid default URL"),
            chain_id: 31337,
            v2_chain_id: 1,
            private_key: B256::ZERO,
            contracts: EvmContracts::default(),
        }
    }
}

impl EvmConfig {
    pub fn from_env() -> Result<Self> {
        let rpc_url =
            std::env::var("EVM_RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string());
        let chain_id = std::env::var("EVM_CHAIN_ID")
            .unwrap_or_else(|_| "31337".to_string())
            .parse()
            .unwrap_or(31337);
        let private_key = std::env::var("EVM_PRIVATE_KEY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(B256::ZERO);

        Ok(Self {
            rpc_url: Url::parse(&rpc_url)?,
            chain_id,
            v2_chain_id: 1, // Default: primary chain is V2 ID 1
            private_key,
            contracts: EvmContracts::from_env()?,
        })
    }

    /// Load from environment with a prefix (e.g., "EVM2" for the secondary chain)
    pub fn from_env_with_prefix(prefix: &str) -> Result<Self> {
        let rpc_url = std::env::var(format!("{}_RPC_URL", prefix))
            .unwrap_or_else(|_| "http://localhost:8546".to_string());
        let chain_id: u64 = std::env::var(format!("{}_CHAIN_ID", prefix))
            .unwrap_or_else(|_| "31338".to_string())
            .parse()
            .unwrap_or(31338);
        let v2_chain_id: u32 = std::env::var(format!("{}_V2_CHAIN_ID", prefix))
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3);
        let private_key = std::env::var("EVM_PRIVATE_KEY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(B256::ZERO);

        Ok(Self {
            rpc_url: Url::parse(&rpc_url)?,
            chain_id,
            v2_chain_id,
            private_key,
            contracts: EvmContracts::from_env_with_prefix(prefix)?,
        })
    }
}

/// EVM contract addresses - all typed as Address, not String
#[derive(Debug, Clone, Default)]
pub struct EvmContracts {
    pub access_manager: Address,
    pub chain_registry: Address,
    pub token_registry: Address,
    pub mint_burn: Address,
    pub lock_unlock: Address,
    pub bridge: Address,
    /// Test ERC20 token address for E2E transfer tests
    pub test_token: Address,
}

impl EvmContracts {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            access_manager: parse_address_env("EVM_ACCESS_MANAGER_ADDRESS")?,
            chain_registry: parse_address_env("EVM_CHAIN_REGISTRY_ADDRESS")?,
            token_registry: parse_address_env("EVM_TOKEN_REGISTRY_ADDRESS")?,
            mint_burn: parse_address_env("EVM_MINT_BURN_ADDRESS")?,
            lock_unlock: parse_address_env("EVM_LOCK_UNLOCK_ADDRESS")?,
            bridge: parse_address_env("EVM_BRIDGE_ADDRESS")?,
            test_token: parse_address_env("TEST_TOKEN_ADDRESS").unwrap_or(Address::ZERO),
        })
    }

    /// Load from environment with a prefix (e.g., "EVM2")
    pub fn from_env_with_prefix(prefix: &str) -> Result<Self> {
        Ok(Self {
            access_manager: parse_address_env(&format!("{}_ACCESS_MANAGER_ADDRESS", prefix))?,
            chain_registry: parse_address_env(&format!("{}_CHAIN_REGISTRY_ADDRESS", prefix))?,
            token_registry: parse_address_env(&format!("{}_TOKEN_REGISTRY_ADDRESS", prefix))?,
            mint_burn: parse_address_env(&format!("{}_MINT_BURN_ADDRESS", prefix))?,
            lock_unlock: parse_address_env(&format!("{}_LOCK_UNLOCK_ADDRESS", prefix))?,
            bridge: parse_address_env(&format!("{}_BRIDGE_ADDRESS", prefix))?,
            test_token: parse_address_env(&format!("{}_TEST_TOKEN_ADDRESS", prefix))
                .unwrap_or(Address::ZERO),
        })
    }

    pub fn from_broadcast(broadcast: &BroadcastFile) -> Result<Self> {
        Ok(Self {
            access_manager: broadcast
                .find_contract("AccessManagerEnumerable")
                .unwrap_or(Address::ZERO),
            chain_registry: broadcast.find_contract("ChainRegistry")?,
            token_registry: broadcast.find_contract("TokenRegistry")?,
            mint_burn: broadcast.find_contract("MintBurn")?,
            lock_unlock: broadcast.find_contract("LockUnlock")?,
            bridge: broadcast.find_contract("Bridge")?,
            // Test token is deployed separately, not in the main broadcast
            test_token: Address::ZERO,
        })
    }
}

/// Terra chain configuration
#[derive(Debug, Clone)]
pub struct TerraConfig {
    pub rpc_url: Url,
    pub lcd_url: Url,
    pub chain_id: String,
    pub bridge_address: Option<String>,
    pub cw20_address: Option<String>,
    pub mnemonic: Option<String>,
}

impl Default for TerraConfig {
    fn default() -> Self {
        Self {
            rpc_url: Url::parse("http://localhost:26657").expect("valid default URL"),
            lcd_url: Url::parse("http://localhost:1317").expect("valid default URL"),
            chain_id: "localterra".to_string(),
            bridge_address: None,
            cw20_address: None,
            mnemonic: None,
        }
    }
}

impl TerraConfig {
    pub fn from_env() -> Result<Self> {
        let rpc_url =
            std::env::var("TERRA_RPC_URL").unwrap_or_else(|_| "http://localhost:26657".to_string());
        let lcd_url =
            std::env::var("TERRA_LCD_URL").unwrap_or_else(|_| "http://localhost:1317".to_string());

        Ok(Self {
            rpc_url: Url::parse(&rpc_url)?,
            lcd_url: Url::parse(&lcd_url)?,
            chain_id: std::env::var("TERRA_CHAIN_ID").unwrap_or_else(|_| "localterra".to_string()),
            bridge_address: std::env::var("TERRA_BRIDGE_ADDRESS").ok(),
            cw20_address: std::env::var("TERRA_CW20_ADDRESS").ok(),
            mnemonic: std::env::var("TERRA_MNEMONIC").ok(),
        })
    }
}

/// Docker configuration for E2E infrastructure
#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub compose_profile: String,
    pub anvil_port: u16,
    pub postgres_port: u16,
    pub terra_rpc_port: u16,
    pub terra_lcd_port: u16,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            compose_profile: "e2e".to_string(),
            anvil_port: 8545,
            postgres_port: 5433,
            terra_rpc_port: 26657,
            terra_lcd_port: 1317,
        }
    }
}

impl DockerConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            compose_profile: std::env::var("DOCKER_COMPOSE_PROFILE")
                .unwrap_or_else(|_| "e2e".to_string()),
            anvil_port: std::env::var("E2E_ANVIL_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8545),
            postgres_port: std::env::var("E2E_POSTGRES_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5433),
            terra_rpc_port: std::env::var("E2E_TERRA_RPC_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(26657),
            terra_lcd_port: std::env::var("E2E_TERRA_LCD_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1317),
        })
    }
}

/// Operator service configuration
#[derive(Debug, Clone)]
pub struct OperatorConfig {
    pub database_url: String,
    pub finality_blocks: u64,
    pub poll_interval_ms: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

impl Default for OperatorConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://operator:operator@localhost:5433/operator".to_string(),
            finality_blocks: 1,
            poll_interval_ms: 1000,
            retry_attempts: 5,
            retry_delay_ms: 5000,
        }
    }
}

impl OperatorConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://operator:operator@localhost:5433/operator".to_string()
            }),
            finality_blocks: std::env::var("FINALITY_BLOCKS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1),
            poll_interval_ms: std::env::var("POLL_INTERVAL_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1000),
            retry_attempts: std::env::var("RETRY_ATTEMPTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            retry_delay_ms: std::env::var("RETRY_DELAY_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5000),
        })
    }
}

/// Test account configuration
#[derive(Debug, Clone)]
pub struct TestAccounts {
    pub evm_address: Address,
    pub evm_private_key: B256,
    pub terra_address: String,
    pub terra_key_name: String,
}

impl Default for TestAccounts {
    fn default() -> Self {
        // Anvil's default test account
        Self {
            evm_address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
                .parse()
                .expect("valid default address"),
            evm_private_key: "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .expect("valid default key"),
            terra_address: "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v".to_string(),
            terra_key_name: "test1".to_string(),
        }
    }
}

impl TestAccounts {
    pub fn from_env() -> Result<Self> {
        let evm_address = std::env::var("EVM_TEST_ADDRESS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| Self::default().evm_address);
        let evm_private_key = std::env::var("EVM_PRIVATE_KEY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| Self::default().evm_private_key);

        Ok(Self {
            evm_address,
            evm_private_key,
            terra_address: std::env::var("TERRA_TEST_ADDRESS")
                .unwrap_or_else(|_| Self::default().terra_address),
            terra_key_name: std::env::var("TERRA_KEY_NAME")
                .unwrap_or_else(|_| Self::default().terra_key_name),
        })
    }
}

// --- Broadcast file parsing ---

/// Forge broadcast file structure
#[derive(Debug, Deserialize)]
pub struct BroadcastFile {
    pub transactions: Vec<BroadcastTransaction>,
}

/// Single transaction in broadcast file
#[derive(Debug, Deserialize)]
pub struct BroadcastTransaction {
    #[serde(rename = "contractName")]
    pub contract_name: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<Address>,
    #[serde(rename = "transactionType")]
    pub transaction_type: String,
}

impl BroadcastFile {
    /// Find a contract address by name (CREATE transaction type)
    pub fn find_contract(&self, name: &str) -> Result<Address> {
        self.transactions
            .iter()
            .find(|tx| tx.transaction_type == "CREATE" && tx.contract_name.as_deref() == Some(name))
            .and_then(|tx| tx.contract_address)
            .ok_or_else(|| eyre!("Contract '{}' not found in broadcast file", name))
    }
}

// --- Helper functions ---

fn parse_address_env(key: &str) -> Result<Address> {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| eyre!("Missing or invalid address: {}", key))
        .or_else(|_| Ok(Address::ZERO))
}
