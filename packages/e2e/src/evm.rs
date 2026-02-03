//! EVM contract interactions for E2E tests
//!
//! This module provides type-safe wrappers around EVM contract calls
//! using the `alloy` library, replacing bash-based `cast` commands.

use alloy::primitives::{Address, B256, U256};
use alloy::providers::Provider;
use alloy::sol;
use eyre::Result;
use std::time::Duration;
use tracing::info;

// Define contract ABIs using alloy::sol! macro
sol! {
    /// Bridge contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IBridge {
        function depositNonce() public view returns (uint256);
        function withdrawDelay() public view returns (uint256);
        function deposit(
            address token,
            uint256 amount,
            bytes32 destChainKey,
            bytes32 destAccount
        ) external returns (uint256);
    }

    /// Access Manager contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IAccessManager {
        function grantRole(uint64 roleId, address account, uint32 delay) external;
        function hasRole(uint64 roleId, address account) public view returns (bool, uint32);
    }

    /// Chain Registry contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IChainRegistry {
        function addCOSMWChainKey(string calldata chainId) external returns (bytes32);
        function getChainKeyCOSMW(string calldata chainId) public view returns (bytes32);
        function getChainKeyEVM(uint256 chainId) public view returns (bytes32);
    }

    /// Token Registry contract ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract ITokenRegistry {
        function addToken(address token, uint8 bridgeType) external;
        function addTokenDestChainKey(
            address token,
            bytes32 destChainKey,
            bytes32 destTokenAddress,
            uint8 decimals
        ) external;
        function isTokenDestChainKeyRegistered(address token, bytes32 destChainKey) public view returns (bool);
    }

    /// ERC20 token ABI
    #[derive(Debug)]
    #[sol(rpc)]
    contract IERC20 {
        function balanceOf(address account) public view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

/// Role constants for Access Manager
pub const OPERATOR_ROLE: u64 = 1;
pub const CANCELER_ROLE: u64 = 2;

/// EVM Bridge interaction helper
pub struct EvmBridgeClient<P> {
    provider: P,
    bridge_address: Address,
    router_address: Address,
}

impl<P: Provider + Clone> EvmBridgeClient<P> {
    /// Create a new EvmBridgeClient instance
    pub fn new(provider: P, bridge_address: Address, router_address: Address) -> Self {
        Self {
            provider,
            bridge_address,
            router_address,
        }
    }

    /// Get current deposit nonce
    pub async fn deposit_nonce(&self) -> Result<u64> {
        let bridge = IBridge::new(self.bridge_address, &self.provider);
        let result = bridge.depositNonce().call().await?;
        Ok(result._0.try_into().unwrap_or(0))
    }

    /// Get withdraw delay in seconds
    pub async fn withdraw_delay(&self) -> Result<u64> {
        let bridge = IBridge::new(self.bridge_address, &self.provider);
        let result = bridge.withdrawDelay().call().await?;
        Ok(result._0.try_into().unwrap_or(0))
    }

    /// Get token balance
    pub async fn balance_of(&self, token: Address, account: Address) -> Result<U256> {
        let token_contract = IERC20::new(token, &self.provider);
        let result = token_contract.balanceOf(account).call().await?;
        Ok(result._0)
    }
}

/// Access Manager helper for role management
pub struct AccessManagerClient<P> {
    provider: P,
    address: Address,
}

impl<P: Provider + Clone> AccessManagerClient<P> {
    /// Create a new AccessManagerClient instance
    pub fn new(provider: P, address: Address) -> Self {
        Self { provider, address }
    }

    /// Check if account has role
    pub async fn has_role(&self, role_id: u64, account: Address) -> Result<bool> {
        let am = IAccessManager::new(self.address, &self.provider);
        let result = am.hasRole(role_id, account).call().await?;
        Ok(result._0)
    }
}

/// Chain Registry helper
pub struct ChainRegistryClient<P> {
    provider: P,
    address: Address,
}

impl<P: Provider + Clone> ChainRegistryClient<P> {
    /// Create a new ChainRegistryClient instance
    pub fn new(provider: P, address: Address) -> Self {
        Self { provider, address }
    }

    /// Get chain key for COSMW chain
    pub async fn get_chain_key_cosmw(&self, chain_id: &str) -> Result<B256> {
        let cr = IChainRegistry::new(self.address, &self.provider);
        let result = cr.getChainKeyCOSMW(chain_id.to_string()).call().await?;
        Ok(result._0)
    }

    /// Get chain key for EVM chain
    pub async fn get_chain_key_evm(&self, chain_id: u64) -> Result<B256> {
        let cr = IChainRegistry::new(self.address, &self.provider);
        let result = cr.getChainKeyEVM(U256::from(chain_id)).call().await?;
        Ok(result._0)
    }
}

/// Token Registry helper
pub struct TokenRegistryClient<P> {
    provider: P,
    address: Address,
}

impl<P: Provider + Clone> TokenRegistryClient<P> {
    /// Create a new TokenRegistryClient instance
    pub fn new(provider: P, address: Address) -> Self {
        Self { provider, address }
    }

    /// Check if token is registered for destination chain
    pub async fn is_token_registered(&self, token: Address, dest_chain_key: B256) -> Result<bool> {
        let tr = ITokenRegistry::new(self.address, &self.provider);
        let result = tr
            .isTokenDestChainKeyRegistered(token, dest_chain_key)
            .call()
            .await?;
        Ok(result._0)
    }
}

/// Wait for transaction confirmation
pub async fn wait_for_tx<P: Provider>(
    provider: &P,
    tx_hash: B256,
    timeout: Duration,
) -> Result<bool> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(receipt) = provider.get_transaction_receipt(tx_hash).await? {
            let success = receipt.status();
            if success {
                info!("Transaction confirmed: 0x{}", tx_hash);
                return Ok(true);
            }
            info!("Transaction failed: 0x{}", tx_hash);
            return Ok(false);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    info!("Transaction timeout: 0x{}", tx_hash);
    Ok(false)
}

/// Check transaction success
pub async fn check_tx_success<P: Provider>(provider: &P, tx_hash: B256) -> Result<bool> {
    match provider.get_transaction_receipt(tx_hash).await? {
        Some(receipt) => {
            let success = receipt.status();
            if success {
                info!("Transaction succeeded: 0x{}", tx_hash);
            } else {
                info!("Transaction failed: 0x{}", tx_hash);
            }
            Ok(success)
        }
        None => {
            info!("Transaction receipt not found: 0x{}", tx_hash);
            Ok(false)
        }
    }
}

/// Anvil time manipulation client
///
/// Provides methods for manipulating time on Anvil for testing watchtower delays.
pub struct AnvilTimeClient {
    rpc_url: String,
    client: reqwest::Client,
}

impl AnvilTimeClient {
    /// Create a new AnvilTimeClient
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Skip time on Anvil (for testing watchtower delays)
    ///
    /// This increases the timestamp by the specified number of seconds
    /// and mines a block to apply the change.
    pub async fn increase_time(&self, seconds: u64) -> Result<()> {
        info!("Increasing Anvil time by {} seconds", seconds);

        // Call evm_increaseTime
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "evm_increaseTime",
                "params": [seconds],
                "id": 1
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(eyre::eyre!(
                "evm_increaseTime failed: {}",
                response.status()
            ));
        }

        let body: serde_json::Value = response.json().await?;
        if body.get("error").is_some() {
            return Err(eyre::eyre!("evm_increaseTime error: {}", body["error"]));
        }

        // Mine a block to apply the time change
        self.mine_block().await?;

        info!("Time increased by {} seconds and block mined", seconds);
        Ok(())
    }

    /// Mine a single block on Anvil
    pub async fn mine_block(&self) -> Result<()> {
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "evm_mine",
                "params": [],
                "id": 1
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(eyre::eyre!("evm_mine failed: {}", response.status()));
        }

        let body: serde_json::Value = response.json().await?;
        if body.get("error").is_some() {
            return Err(eyre::eyre!("evm_mine error: {}", body["error"]));
        }

        Ok(())
    }

    /// Get current block timestamp
    pub async fn get_block_timestamp(&self) -> Result<u64> {
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "eth_getBlockByNumber",
                "params": ["latest", false],
                "id": 1
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(eyre::eyre!(
                "eth_getBlockByNumber failed: {}",
                response.status()
            ));
        }

        let body: serde_json::Value = response.json().await?;
        let timestamp_hex = body["result"]["timestamp"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("No timestamp in block"))?;

        let timestamp = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)?;
        Ok(timestamp)
    }

    /// Set the next block timestamp to a specific value
    pub async fn set_next_block_timestamp(&self, timestamp: u64) -> Result<()> {
        info!("Setting next block timestamp to {}", timestamp);

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "evm_setNextBlockTimestamp",
                "params": [timestamp],
                "id": 1
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(eyre::eyre!(
                "evm_setNextBlockTimestamp failed: {}",
                response.status()
            ));
        }

        let body: serde_json::Value = response.json().await?;
        if body.get("error").is_some() {
            return Err(eyre::eyre!(
                "evm_setNextBlockTimestamp error: {}",
                body["error"]
            ));
        }

        Ok(())
    }
}
