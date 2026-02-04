//! Token Factory Module for E2E Tests
//!
//! This module provides a consolidated `TokenFactory` for creating and managing
//! tokens during E2E testing. It supports multiple token types:
//!
//! - **ERC20 Mintable**: Standard ERC20 with public mint function for testing
//! - **CW20 Base**: CosmWasm CW20 token for Terra chain
//! - **CW20 Mintable**: CW20 with minting capability
//! - **ERC20 Preset**: OpenZeppelin preset tokens with roles
//!
//! # Usage
//!
//! ```ignore
//! use cl8y_e2e::tokens::{TokenFactory, TokenType, TokenConfig};
//!
//! let factory = TokenFactory::new(&config);
//!
//! // Deploy a simple mintable ERC20
//! let token = factory.deploy(TokenType::Erc20Mintable, TokenConfig {
//!     name: "Test Token".to_string(),
//!     symbol: "TT".to_string(),
//!     decimals: 18,
//!     initial_supply: 1_000_000_000_000_000_000_000_000u128, // 1M tokens
//! }).await?;
//!
//! // Mint additional tokens
//! factory.mint_erc20(token.address, recipient, amount).await?;
//! ```
//!
//! # Token Types
//!
//! | Type | Chain | Features |
//! |------|-------|----------|
//! | `Erc20Mintable` | EVM | Public mint, configurable decimals |
//! | `Erc20Preset` | EVM | Role-based mint/burn/pause |
//! | `Cw20Base` | Terra | Standard CW20 with initial balance |
//! | `Cw20Mintable` | Terra | CW20 with minting capability |
//!
//! # Extensibility
//!
//! The factory is designed for extensibility. Add new token types by:
//! 1. Adding a variant to `TokenType`
//! 2. Implementing the deployment logic in `TokenFactory::deploy_internal`
//! 3. Adding appropriate minting/balance query methods

use alloy::primitives::{Address, U256};
use eyre::{eyre, Result};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::config::E2eConfig;
use crate::terra::TerraClient;

// ============================================================================
// Token Types and Configuration
// ============================================================================

/// Supported token types for E2E testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    /// ERC20 with public mint function (MockMintableToken)
    Erc20Mintable,
    /// OpenZeppelin ERC20PresetMinterPauser with role-based access
    Erc20Preset,
    /// Standard CosmWasm CW20 token
    Cw20Base,
    /// CW20 with minting capability
    Cw20Mintable,
}

impl TokenType {
    /// Get the chain this token type is deployed on
    pub fn chain(&self) -> TokenChain {
        match self {
            TokenType::Erc20Mintable | TokenType::Erc20Preset => TokenChain::Evm,
            TokenType::Cw20Base | TokenType::Cw20Mintable => TokenChain::Terra,
        }
    }

    /// Get human-readable name for the token type
    pub fn name(&self) -> &'static str {
        match self {
            TokenType::Erc20Mintable => "ERC20 Mintable",
            TokenType::Erc20Preset => "ERC20 Preset (Roles)",
            TokenType::Cw20Base => "CW20 Base",
            TokenType::Cw20Mintable => "CW20 Mintable",
        }
    }
}

/// Chain on which a token is deployed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenChain {
    Evm,
    Terra,
}

/// Configuration for token deployment
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// Token name (e.g., "Test Bridge Token")
    pub name: String,
    /// Token symbol (e.g., "TBT")
    pub symbol: String,
    /// Number of decimals (typically 18 for ERC20, 6 for CW20)
    pub decimals: u8,
    /// Initial supply to mint to deployer (in smallest unit)
    pub initial_supply: u128,
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            name: "Test Bridge Token".to_string(),
            symbol: "TBT".to_string(),
            decimals: 18,
            initial_supply: 1_000_000_000_000_000_000_000_000, // 1M with 18 decimals
        }
    }
}

impl TokenConfig {
    /// Create a new token config with custom name and symbol
    pub fn new(name: impl Into<String>, symbol: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            symbol: symbol.into(),
            ..Default::default()
        }
    }

    /// Set the number of decimals
    pub fn with_decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    /// Set the initial supply (in smallest unit)
    pub fn with_initial_supply(mut self, supply: u128) -> Self {
        self.initial_supply = supply;
        self
    }

    /// Create a CW20-compatible config (6 decimals)
    pub fn cw20_default() -> Self {
        Self {
            name: "Test CW20 Token".to_string(),
            symbol: "TCW".to_string(),
            decimals: 6,
            initial_supply: 1_000_000_000_000, // 1M with 6 decimals
        }
    }
}

// ============================================================================
// Deployed Token Information
// ============================================================================

/// Information about a deployed token
#[derive(Debug, Clone)]
pub struct DeployedToken {
    /// Token type
    pub token_type: TokenType,
    /// Token address (EVM address or CW20 contract address)
    pub address: TokenAddress,
    /// Token configuration used during deployment
    pub config: TokenConfig,
    /// Transaction hash of deployment (if available)
    pub deploy_tx: Option<String>,
}

/// Token address that can be either EVM or Terra
#[derive(Debug, Clone)]
pub enum TokenAddress {
    /// EVM address (20 bytes)
    Evm(Address),
    /// Terra/CosmWasm contract address (bech32 string)
    Terra(String),
}

impl TokenAddress {
    /// Get as EVM address (returns None if Terra address)
    pub fn as_evm(&self) -> Option<Address> {
        match self {
            TokenAddress::Evm(addr) => Some(*addr),
            TokenAddress::Terra(_) => None,
        }
    }

    /// Get as Terra address (returns None if EVM address)
    pub fn as_terra(&self) -> Option<&str> {
        match self {
            TokenAddress::Evm(_) => None,
            TokenAddress::Terra(addr) => Some(addr),
        }
    }

    /// Check if this is an EVM address
    pub fn is_evm(&self) -> bool {
        matches!(self, TokenAddress::Evm(_))
    }

    /// Check if this is a Terra address
    pub fn is_terra(&self) -> bool {
        matches!(self, TokenAddress::Terra(_))
    }
}

impl std::fmt::Display for TokenAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenAddress::Evm(addr) => write!(f, "{}", addr),
            TokenAddress::Terra(addr) => write!(f, "{}", addr),
        }
    }
}

// ============================================================================
// Token Factory
// ============================================================================

/// Factory for creating and managing tokens during E2E tests
///
/// The factory provides a unified interface for deploying tokens on both
/// EVM and Terra chains, with support for minting and balance queries.
pub struct TokenFactory<'a> {
    config: &'a E2eConfig,
    project_root: &'a Path,
}

impl<'a> TokenFactory<'a> {
    /// Create a new TokenFactory
    ///
    /// # Arguments
    /// * `config` - E2E configuration with RPC URLs and private keys
    /// * `project_root` - Path to the monorepo root (for finding contracts)
    pub fn new(config: &'a E2eConfig, project_root: &'a Path) -> Self {
        Self {
            config,
            project_root,
        }
    }

    /// Deploy a new token
    ///
    /// # Arguments
    /// * `token_type` - Type of token to deploy
    /// * `token_config` - Configuration for the token
    ///
    /// # Returns
    /// Information about the deployed token including its address
    pub async fn deploy(
        &self,
        token_type: TokenType,
        token_config: TokenConfig,
    ) -> Result<DeployedToken> {
        info!(
            "Deploying {} token: {} ({})",
            token_type.name(),
            token_config.name,
            token_config.symbol
        );

        match token_type {
            TokenType::Erc20Mintable => self.deploy_erc20_mintable(&token_config).await,
            TokenType::Erc20Preset => self.deploy_erc20_preset(&token_config).await,
            TokenType::Cw20Base => self.deploy_cw20_base(&token_config).await,
            TokenType::Cw20Mintable => self.deploy_cw20_mintable(&token_config).await,
        }
    }

    /// Deploy the default test token (ERC20 Mintable with standard config)
    ///
    /// This is a convenience method for deploying the standard test token
    /// used in most E2E tests.
    pub async fn deploy_default_test_token(&self) -> Result<DeployedToken> {
        self.deploy(TokenType::Erc20Mintable, TokenConfig::default())
            .await
    }

    /// Deploy the default CW20 token for Terra tests
    pub async fn deploy_default_cw20(&self) -> Result<DeployedToken> {
        self.deploy(TokenType::Cw20Base, TokenConfig::cw20_default())
            .await
    }

    // ========================================================================
    // ERC20 Deployment Methods
    // ========================================================================

    /// Deploy an ERC20 Mintable token using MockMintableToken contract
    async fn deploy_erc20_mintable(&self, config: &TokenConfig) -> Result<DeployedToken> {
        let rpc_url = self.config.evm.rpc_url.as_str();
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);
        let contracts_dir = self.project_root.join("packages").join("contracts-evm");

        if !contracts_dir.exists() {
            return Err(eyre!(
                "contracts-evm directory not found at: {}",
                contracts_dir.display()
            ));
        }

        info!(
            "Deploying MockMintableToken: {} ({}) with {} decimals",
            config.name, config.symbol, config.decimals
        );

        // Use forge create to deploy MockMintableToken
        let output = std::process::Command::new("forge")
            .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
            .current_dir(&contracts_dir)
            .args([
                "create",
                "--rpc-url",
                rpc_url,
                "--private-key",
                &private_key,
                "--broadcast",
                "test/mocks/MockMintableToken.sol:MockMintableToken",
                "--constructor-args",
                &config.name,
                &config.symbol,
                &config.decimals.to_string(),
                "--json",
            ])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Err(eyre!(
                "Failed to deploy ERC20 Mintable: stdout={}, stderr={}",
                stdout,
                stderr
            ));
        }

        // Parse deployed address from forge output
        let address = self.parse_forge_deploy_output(&stdout, &stderr)?;
        info!("ERC20 Mintable deployed at: {}", address);

        // Mint initial supply if specified
        if config.initial_supply > 0 {
            self.mint_erc20(
                address,
                self.config.test_accounts.evm_address,
                config.initial_supply,
            )
            .await?;
        }

        Ok(DeployedToken {
            token_type: TokenType::Erc20Mintable,
            address: TokenAddress::Evm(address),
            config: config.clone(),
            deploy_tx: None,
        })
    }

    /// Deploy an ERC20 Preset token (with roles)
    async fn deploy_erc20_preset(&self, config: &TokenConfig) -> Result<DeployedToken> {
        // For now, use the same deployment as Mintable since MockMintableToken
        // doesn't have role restrictions. Future: deploy OpenZeppelin preset.
        self.deploy_erc20_mintable(config).await.map(|mut t| {
            t.token_type = TokenType::Erc20Preset;
            t
        })
    }

    /// Parse deployed address from forge create output
    fn parse_forge_deploy_output(&self, stdout: &str, stderr: &str) -> Result<Address> {
        // Try JSON parsing first
        if let Some(json_line) = stdout.lines().find(|line| line.trim().starts_with('{')) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_line) {
                if let Some(deployed_to) = json["deployedTo"].as_str() {
                    return deployed_to.parse().map_err(|e| {
                        eyre!("Failed to parse deployed address '{}': {}", deployed_to, e)
                    });
                }
            }
        }

        // Try plain text format: "Deployed to: 0x..."
        if let Some(line) = stdout.lines().find(|line| line.contains("Deployed to:")) {
            let addr_str = line
                .split("Deployed to:")
                .nth(1)
                .map(|s| s.trim())
                .ok_or_else(|| eyre!("Could not extract address from: {}", line))?;
            return addr_str
                .parse()
                .map_err(|e| eyre!("Failed to parse address '{}': {}", addr_str, e));
        }

        Err(eyre!(
            "Could not find deployed address in forge output: stdout={}, stderr={}",
            stdout,
            stderr
        ))
    }

    // ========================================================================
    // CW20 Deployment Methods
    // ========================================================================

    /// Deploy a CW20 Base token on Terra
    async fn deploy_cw20_base(&self, config: &TokenConfig) -> Result<DeployedToken> {
        info!(
            "Deploying CW20 Base: {} ({}) with {} decimals",
            config.name, config.symbol, config.decimals
        );

        let terra = TerraClient::new(&self.config.terra);

        // Check if LocalTerra is running
        if !terra.is_healthy().await? {
            return Err(eyre!(
                "LocalTerra is not running - cannot deploy CW20 token"
            ));
        }

        // Check for CW20 WASM
        let cw20_wasm = self
            .project_root
            .join("packages")
            .join("contracts-terraclassic")
            .join("artifacts")
            .join("cw20_mintable.wasm");

        if !cw20_wasm.exists() {
            return Err(eyre!("CW20 WASM not found at: {}", cw20_wasm.display()));
        }

        let test_address = &self.config.test_accounts.terra_address;

        // Deploy using the existing cw20_deploy module
        match crate::cw20_deploy::deploy_cw20_token(
            &cw20_wasm,
            &config.name,
            &config.symbol,
            config.decimals,
            config.initial_supply,
            test_address,
        )
        .await
        {
            Ok(result) => {
                info!("CW20 Base deployed at: {}", result.contract_address);
                Ok(DeployedToken {
                    token_type: TokenType::Cw20Base,
                    address: TokenAddress::Terra(result.contract_address),
                    config: config.clone(),
                    deploy_tx: None,
                })
            }
            Err(e) => Err(eyre!("CW20 deployment failed: {}", e)),
        }
    }

    /// Deploy a CW20 Mintable token on Terra
    async fn deploy_cw20_mintable(&self, config: &TokenConfig) -> Result<DeployedToken> {
        // CW20 Mintable uses the same base deployment with minter set
        // The cw20_deploy module handles minter configuration
        self.deploy_cw20_base(config).await.map(|mut t| {
            t.token_type = TokenType::Cw20Mintable;
            t
        })
    }

    // ========================================================================
    // Token Operations
    // ========================================================================

    /// Mint ERC20 tokens to an address
    pub async fn mint_erc20(&self, token: Address, to: Address, amount: u128) -> Result<()> {
        let rpc_url = self.config.evm.rpc_url.as_str();
        let private_key = format!("0x{:x}", self.config.test_accounts.evm_private_key);

        info!("Minting {} tokens to {}", amount, to);

        let output = std::process::Command::new("cast")
            .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
            .args([
                "send",
                "--rpc-url",
                rpc_url,
                "--private-key",
                &private_key,
                &format!("{:?}", token),
                "mint(address,uint256)",
                &format!("{:?}", to),
                &amount.to_string(),
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to mint tokens: {}", stderr));
        }

        debug!("Minted {} tokens to {}", amount, to);
        Ok(())
    }

    /// Query ERC20 balance
    pub async fn balance_of_erc20(&self, token: Address, account: Address) -> Result<U256> {
        let rpc_url = self.config.evm.rpc_url.as_str();

        let output = std::process::Command::new("cast")
            .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
            .args([
                "call",
                "--rpc-url",
                rpc_url,
                &format!("{:?}", token),
                "balanceOf(address)(uint256)",
                &format!("{:?}", account),
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(eyre!("Failed to query balance: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let balance_str = stdout.trim();

        // Parse the balance (cast returns decimal number)
        let balance: u128 = balance_str
            .parse()
            .map_err(|e| eyre!("Failed to parse balance '{}': {}", balance_str, e))?;

        Ok(U256::from(balance))
    }

    /// Query CW20 balance
    pub async fn balance_of_cw20(&self, token: &str, account: &str) -> Result<u128> {
        let terra = TerraClient::new(&self.config.terra);

        let query_msg = serde_json::json!({
            "balance": {
                "address": account
            }
        });

        let result: serde_json::Value = terra.query_contract(token, &query_msg).await?;

        result["balance"]
            .as_str()
            .ok_or_else(|| eyre!("No balance in CW20 query response"))?
            .parse()
            .map_err(|e| eyre!("Failed to parse CW20 balance: {}", e))
    }

    /// Mint CW20 tokens (requires minter permissions)
    pub async fn mint_cw20(&self, token: &str, to: &str, amount: u128) -> Result<String> {
        let terra = TerraClient::new(&self.config.terra);

        let mint_msg = serde_json::json!({
            "mint": {
                "recipient": to,
                "amount": amount.to_string()
            }
        });

        terra.execute_contract(token, &mint_msg, None).await
    }
}

// ============================================================================
// Batch Token Operations
// ============================================================================

/// Options for batch token deployment
#[derive(Debug, Clone)]
pub struct BatchDeployOptions {
    /// Number of ERC20 tokens to deploy
    pub erc20_count: usize,
    /// Number of CW20 tokens to deploy
    pub cw20_count: usize,
    /// Base name for tokens (will be suffixed with index)
    pub base_name: String,
    /// Base symbol for tokens (will be suffixed with index)
    pub base_symbol: String,
    /// Initial supply for each token
    pub initial_supply: u128,
}

impl Default for BatchDeployOptions {
    fn default() -> Self {
        Self {
            erc20_count: 1,
            cw20_count: 0,
            base_name: "Test Token".to_string(),
            base_symbol: "TT".to_string(),
            initial_supply: 1_000_000_000_000_000_000_000_000, // 1M with 18 decimals
        }
    }
}

/// Result of batch token deployment
#[derive(Debug, Clone)]
pub struct BatchDeployResult {
    /// Deployed ERC20 tokens
    pub erc20_tokens: Vec<DeployedToken>,
    /// Deployed CW20 tokens
    pub cw20_tokens: Vec<DeployedToken>,
    /// Errors encountered during deployment
    pub errors: Vec<String>,
}

impl BatchDeployResult {
    /// Get all deployed token addresses (EVM only)
    pub fn evm_addresses(&self) -> Vec<Address> {
        self.erc20_tokens
            .iter()
            .filter_map(|t| t.address.as_evm())
            .collect()
    }

    /// Get all deployed token addresses (Terra only)
    pub fn terra_addresses(&self) -> Vec<String> {
        self.cw20_tokens
            .iter()
            .filter_map(|t| t.address.as_terra().map(|s| s.to_string()))
            .collect()
    }

    /// Check if all deployments succeeded
    pub fn all_succeeded(&self) -> bool {
        self.errors.is_empty()
    }
}

impl<'a> TokenFactory<'a> {
    /// Deploy multiple tokens in batch
    ///
    /// Useful for throughput testing and parallel transfer tests.
    pub async fn deploy_batch(&self, options: BatchDeployOptions) -> BatchDeployResult {
        let mut result = BatchDeployResult {
            erc20_tokens: Vec::with_capacity(options.erc20_count),
            cw20_tokens: Vec::with_capacity(options.cw20_count),
            errors: Vec::new(),
        };

        // Deploy ERC20 tokens
        for i in 0..options.erc20_count {
            let config = TokenConfig {
                name: format!("{} {}", options.base_name, i + 1),
                symbol: format!("{}{}", options.base_symbol, i + 1),
                decimals: 18,
                initial_supply: options.initial_supply,
            };

            match self.deploy(TokenType::Erc20Mintable, config).await {
                Ok(token) => result.erc20_tokens.push(token),
                Err(e) => {
                    warn!("Failed to deploy ERC20 token {}: {}", i + 1, e);
                    result.errors.push(format!("ERC20 {}: {}", i + 1, e));
                }
            }
        }

        // Deploy CW20 tokens
        for i in 0..options.cw20_count {
            let config = TokenConfig {
                name: format!("{} CW20 {}", options.base_name, i + 1),
                symbol: format!("{}C{}", options.base_symbol, i + 1),
                decimals: 6,
                initial_supply: options.initial_supply / 1_000_000_000_000, // Adjust for 6 decimals
            };

            match self.deploy(TokenType::Cw20Base, config).await {
                Ok(token) => result.cw20_tokens.push(token),
                Err(e) => {
                    warn!("Failed to deploy CW20 token {}: {}", i + 1, e);
                    result.errors.push(format!("CW20 {}: {}", i + 1, e));
                }
            }
        }

        result
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_config_default() {
        let config = TokenConfig::default();
        assert_eq!(config.name, "Test Bridge Token");
        assert_eq!(config.symbol, "TBT");
        assert_eq!(config.decimals, 18);
        assert_eq!(config.initial_supply, 1_000_000_000_000_000_000_000_000);
    }

    #[test]
    fn test_token_config_builder() {
        let config = TokenConfig::new("My Token", "MTK")
            .with_decimals(8)
            .with_initial_supply(100_000_000);

        assert_eq!(config.name, "My Token");
        assert_eq!(config.symbol, "MTK");
        assert_eq!(config.decimals, 8);
        assert_eq!(config.initial_supply, 100_000_000);
    }

    #[test]
    fn test_token_config_cw20_default() {
        let config = TokenConfig::cw20_default();
        assert_eq!(config.decimals, 6);
        assert_eq!(config.initial_supply, 1_000_000_000_000);
    }

    #[test]
    fn test_token_type_chain() {
        assert_eq!(TokenType::Erc20Mintable.chain(), TokenChain::Evm);
        assert_eq!(TokenType::Erc20Preset.chain(), TokenChain::Evm);
        assert_eq!(TokenType::Cw20Base.chain(), TokenChain::Terra);
        assert_eq!(TokenType::Cw20Mintable.chain(), TokenChain::Terra);
    }

    #[test]
    fn test_token_address_display() {
        let evm_addr = TokenAddress::Evm(Address::ZERO);
        assert!(evm_addr.to_string().starts_with("0x"));

        let terra_addr = TokenAddress::Terra("terra1abc...".to_string());
        assert_eq!(terra_addr.to_string(), "terra1abc...");
    }

    #[test]
    fn test_token_address_conversion() {
        let evm = TokenAddress::Evm(Address::ZERO);
        assert!(evm.is_evm());
        assert!(!evm.is_terra());
        assert!(evm.as_evm().is_some());
        assert!(evm.as_terra().is_none());

        let terra = TokenAddress::Terra("terra1...".to_string());
        assert!(!terra.is_evm());
        assert!(terra.is_terra());
        assert!(terra.as_evm().is_none());
        assert!(terra.as_terra().is_some());
    }

    #[test]
    fn test_batch_deploy_options_default() {
        let opts = BatchDeployOptions::default();
        assert_eq!(opts.erc20_count, 1);
        assert_eq!(opts.cw20_count, 0);
    }
}
