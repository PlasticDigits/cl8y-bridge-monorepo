//! Transfer helper functions for E2E cross-chain testing
//!
//! This module provides utilities for complete cross-chain transfer testing:
//! - ERC20 token deployment via forge create
//! - Operator polling loop to wait for relay completion
//! - Destination chain approval/withdrawal verification functions

use alloy::primitives::{Address, B256, U256};
use eyre::{eyre, Result};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

use alloy::primitives::keccak256;

use crate::evm::AnvilTimeClient;
use crate::E2eConfig;

/// Compute the 4-byte function selector from a Solidity function signature.
fn selector(sig: &str) -> String {
    hex::encode(&keccak256(sig.as_bytes())[..4])
}

/// Default timeout for polling operations (120 seconds)
/// Increased to account for operator processing and block confirmation
pub const DEFAULT_POLL_TIMEOUT: Duration = Duration::from_secs(120);

/// Initial interval between poll attempts (500ms for fast initial polling)
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Maximum interval between poll attempts (5 seconds)
pub const MAX_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Withdrawal delay in seconds (default: 300s for watchtower pattern)
pub const DEFAULT_WITHDRAW_DELAY: u64 = 300;

// ============================================================================
// ERC20 Token Deployment
// ============================================================================

/// Result of deploying a test ERC20 token
#[derive(Debug, Clone)]
pub struct TokenDeployResult {
    /// Deployed token address
    pub address: Address,
    /// Token name
    pub name: String,
    /// Token symbol
    pub symbol: String,
    /// Initial supply minted
    pub initial_supply: U256,
    /// Deployer/owner address
    pub deployer: Address,
}

/// Deploy a test ERC20 token using forge create
///
/// This deploys an ERC20PresetMinterPauser token with the given parameters.
/// The deployer (derived from private key) will have minter role.
///
/// # Arguments
/// * `config` - E2E configuration containing RPC URL and private key
/// * `name` - Token name (e.g., "Test Token")
/// * `symbol` - Token symbol (e.g., "TEST")
/// * `initial_supply` - Initial supply to mint to deployer (in smallest units)
///
/// # Returns
/// * `TokenDeployResult` with deployment details
pub async fn deploy_erc20_token(
    config: &E2eConfig,
    name: &str,
    symbol: &str,
    initial_supply: U256,
) -> Result<TokenDeployResult> {
    info!("Deploying ERC20 token: {} ({})", name, symbol);

    let private_key = format!("{:?}", config.test_accounts.evm_private_key);
    let rpc_url = config.evm.rpc_url.as_str();

    // Use forge create to deploy the token
    let output = tokio::process::Command::new("forge")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "create",
            "--rpc-url",
            rpc_url,
            "--private-key",
            &private_key,
            "lib/openzeppelin-contracts/contracts/token/ERC20/presets/ERC20PresetMinterPauser.sol:ERC20PresetMinterPauser",
            "--constructor-args",
            name,
            symbol,
            "--json",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to deploy ERC20 token: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| eyre!("Failed to parse forge output: {}", e))?;

    let deployed_to = json["deployedTo"]
        .as_str()
        .ok_or_else(|| eyre!("No deployedTo in forge output"))?;

    let token_address: Address = deployed_to.parse()?;
    info!("ERC20 token deployed at: {}", token_address);

    // Mint initial supply if specified
    if initial_supply > U256::ZERO {
        mint_erc20_tokens(
            config,
            token_address,
            config.test_accounts.evm_address,
            initial_supply,
        )
        .await?;
    }

    Ok(TokenDeployResult {
        address: token_address,
        name: name.to_string(),
        symbol: symbol.to_string(),
        initial_supply,
        deployer: config.test_accounts.evm_address,
    })
}

/// Mint ERC20 tokens to an address (for ERC20PresetMinterPauser tokens)
///
/// # Arguments
/// * `config` - E2E configuration
/// * `token` - Token contract address
/// * `to` - Recipient address
/// * `amount` - Amount to mint
pub async fn mint_erc20_tokens(
    config: &E2eConfig,
    token: Address,
    to: Address,
    amount: U256,
) -> Result<B256> {
    info!("Minting {} tokens to {}", amount, to);

    let private_key = format!("{:?}", config.test_accounts.evm_private_key);
    let rpc_url = config.evm.rpc_url.as_str();

    // Call mint(address,uint256) on the token
    let output = tokio::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            &private_key,
            &format!("{}", token),
            "mint(address,uint256)",
            &format!("{}", to),
            &format!("{}", amount),
            "--json",
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to mint tokens: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_default();

    let tx_hash = json["transactionHash"]
        .as_str()
        .unwrap_or("0x0000000000000000000000000000000000000000000000000000000000000000");

    let tx_hash = B256::from_slice(&hex::decode(tx_hash.trim_start_matches("0x"))?);
    info!("Mint transaction: 0x{}", hex::encode(tx_hash));

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(tx_hash)
}

// ============================================================================
// Operator Polling / Relay Completion Detection
// ============================================================================

/// Poll for operator to relay deposit and create approval
///
/// This polls the destination chain bridge contract for an approval matching
/// the given deposit nonce. Uses exponential backoff with configurable timeout.
///
/// # Arguments
/// * `config` - E2E configuration
/// * `deposit_nonce` - The deposit nonce to look for
/// * `timeout` - Maximum time to wait for approval
///
/// # Returns
/// * `ApprovalInfo` if found, error if timeout or failure
pub async fn poll_for_approval(
    config: &E2eConfig,
    deposit_nonce: u64,
    timeout: Duration,
) -> Result<ApprovalInfo> {
    info!(
        nonce = deposit_nonce,
        timeout_secs = timeout.as_secs(),
        "Polling for EVM approval"
    );

    let start = Instant::now();
    let mut interval = DEFAULT_POLL_INTERVAL;
    let mut attempt = 0;

    while start.elapsed() < timeout {
        attempt += 1;

        match query_approval_by_nonce(config, deposit_nonce).await {
            Ok(Some(approval)) => {
                info!(
                    nonce = deposit_nonce,
                    attempt = attempt,
                    elapsed_secs = start.elapsed().as_secs(),
                    hash = hex::encode(&approval.withdraw_hash.as_slice()[..8]),
                    "Found EVM approval"
                );
                return Ok(approval);
            }
            Ok(None) => {
                // Log progress periodically
                if attempt % 10 == 0 {
                    debug!(
                        nonce = deposit_nonce,
                        attempt = attempt,
                        elapsed_secs = start.elapsed().as_secs(),
                        "Still waiting for EVM approval"
                    );
                }
            }
            Err(e) => {
                debug!(
                    nonce = deposit_nonce,
                    attempt = attempt,
                    error = %e,
                    "Error querying EVM approval (will retry)"
                );
            }
        }

        tokio::time::sleep(interval).await;

        // Exponential backoff with cap
        interval = std::cmp::min(interval * 2, MAX_POLL_INTERVAL);
    }

    Err(eyre!(
        "Timeout waiting for approval of nonce {} after {:?} ({} attempts)",
        deposit_nonce,
        timeout,
        attempt
    ))
}

/// Poll for withdrawal to be ready (delay period passed)
///
/// After an approval is created, there's a withdrawal delay before funds
/// can be claimed. This function polls until the delay has passed.
///
/// # Arguments
/// * `config` - E2E configuration
/// * `withdraw_hash` - The withdrawal hash to check
/// * `timeout` - Maximum time to wait
///
/// # Returns
/// * `true` if withdrawal is ready, error if timeout
pub async fn poll_for_withdrawal_ready(
    config: &E2eConfig,
    withdraw_hash: B256,
    timeout: Duration,
) -> Result<bool> {
    info!(
        "Polling for withdrawal ready: 0x{} (timeout: {:?})",
        hex::encode(withdraw_hash),
        timeout
    );

    let start = Instant::now();
    let interval = Duration::from_secs(5);

    while start.elapsed() < timeout {
        match is_withdrawal_ready(config, withdraw_hash).await {
            Ok(true) => {
                info!(
                    "Withdrawal 0x{} is ready for execution",
                    hex::encode(withdraw_hash)
                );
                return Ok(true);
            }
            Ok(false) => {
                debug!(
                    "Withdrawal 0x{} not ready yet...",
                    hex::encode(&withdraw_hash.as_slice()[..8])
                );
            }
            Err(e) => {
                warn!("Error checking withdrawal readiness: {}", e);
            }
        }

        tokio::time::sleep(interval).await;
    }

    Err(eyre!(
        "Timeout waiting for withdrawal ready after {:?}",
        timeout
    ))
}

/// Skip Anvil time to pass withdrawal delay
///
/// For testing, we can use Anvil's time manipulation to skip the
/// withdrawal delay period instead of waiting in real-time.
///
/// # Arguments
/// * `config` - E2E configuration
/// * `extra_seconds` - Extra seconds to add beyond the delay
pub async fn skip_withdrawal_delay(config: &E2eConfig, extra_seconds: u64) -> Result<()> {
    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());

    // Query the actual cancel window from contract
    let delay = query_cancel_window_seconds(config)
        .await
        .unwrap_or(DEFAULT_WITHDRAW_DELAY);

    let skip_time = delay + extra_seconds;
    info!(
        "Skipping {} seconds on Anvil (delay={}, extra={})",
        skip_time, delay, extra_seconds
    );

    anvil.increase_time(skip_time).await?;

    Ok(())
}

// ============================================================================
// Approval & Withdrawal Verification
// ============================================================================

/// Approval information returned from bridge
#[derive(Debug, Clone)]
pub struct ApprovalInfo {
    /// The withdraw hash for this approval
    pub withdraw_hash: B256,
    /// Source chain key
    pub src_chain_key: B256,
    /// Token address
    pub token: Address,
    /// Recipient address
    pub recipient: Address,
    /// Amount approved
    pub amount: U256,
    /// Deposit nonce
    pub nonce: u64,
    /// Timestamp when approved
    pub approved_at: u64,
    /// Whether already cancelled
    pub cancelled: bool,
    /// Whether withdrawal was executed
    pub executed: bool,
}

/// Query approval by deposit nonce
///
/// Searches for an approval matching the given nonce by querying recent
/// ApproveWithdraw events from the bridge contract.
async fn query_approval_by_nonce(config: &E2eConfig, nonce: u64) -> Result<Option<ApprovalInfo>> {
    let client = reqwest::Client::new();

    // Query ApproveWithdraw event logs
    // Event signature: ApproveWithdraw(bytes32 indexed srcChainKey, bytes32 indexed withdrawHash, ...)
    let approval_topic = "0x" // ApproveWithdraw event topic (first 32 bytes of keccak256)
        .to_string()
        + &hex::encode(alloy::primitives::keccak256(
            b"ApproveWithdraw(bytes32,bytes32,address,address,uint256,uint256)",
        ));

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("{}", config.evm.contracts.bridge),
                "topics": [approval_topic]
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(logs) = body["result"].as_array() {
        for log in logs {
            // Parse log data to find matching nonce
            let data = log["data"].as_str().unwrap_or("");
            let data_bytes = hex::decode(data.trim_start_matches("0x")).unwrap_or_default();

            // Nonce is the 5th 32-byte slot in data (offset 160)
            if data_bytes.len() >= 192 {
                let log_nonce =
                    u64::from_be_bytes(data_bytes[184..192].try_into().unwrap_or([0u8; 8]));

                if log_nonce == nonce {
                    // Found matching approval - parse full info
                    let empty_vec = vec![];
                    let topics = log["topics"].as_array().unwrap_or(&empty_vec);
                    let src_chain_key = if topics.len() > 1 {
                        B256::from_slice(
                            &hex::decode(topics[1].as_str().unwrap_or("").trim_start_matches("0x"))
                                .unwrap_or_default(),
                        )
                    } else {
                        B256::ZERO
                    };

                    let withdraw_hash = if topics.len() > 2 {
                        B256::from_slice(
                            &hex::decode(topics[2].as_str().unwrap_or("").trim_start_matches("0x"))
                                .unwrap_or_default(),
                        )
                    } else {
                        B256::ZERO
                    };

                    // Parse remaining fields from data
                    let token = Address::from_slice(&data_bytes[12..32]);
                    let recipient = Address::from_slice(&data_bytes[44..64]);
                    let amount = U256::from_be_slice(&data_bytes[64..96]);

                    return Ok(Some(ApprovalInfo {
                        withdraw_hash,
                        src_chain_key,
                        token,
                        recipient,
                        amount,
                        nonce,
                        approved_at: 0, // Would need additional query
                        cancelled: false,
                        executed: false,
                    }));
                }
            }
        }
    }

    Ok(None)
}

/// Check if a withdrawal is ready to execute (delay passed)
async fn is_withdrawal_ready(config: &E2eConfig, withdraw_hash: B256) -> Result<bool> {
    let client = reqwest::Client::new();

    // Query isWithdrawReady(bytes32) function
    let sel = selector("isWithdrawReady(bytes32)");
    let hash_hex = hex::encode(withdraw_hash);

    let call_data = format!("0x{}{}", sel, hash_hex);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    let result_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre!("No result in response"))?;

    // Result is a bool (32 bytes, last byte is 0 or 1)
    let bytes = hex::decode(result_hex.trim_start_matches("0x")).unwrap_or_default();
    Ok(bytes.last().copied().unwrap_or(0) != 0)
}

/// Query cancel window from bridge contract
async fn query_cancel_window_seconds(config: &E2eConfig) -> Result<u64> {
    let client = reqwest::Client::new();

    let call_data = format!("0x{}", selector("getCancelWindow()"));

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre!("No result in response"))?;

    let delay = u64::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(delay)
}

/// Verify a withdrawal was executed
///
/// Checks the Withdraw event logs for a matching withdraw hash.
pub async fn verify_withdrawal_executed(config: &E2eConfig, withdraw_hash: B256) -> Result<bool> {
    let client = reqwest::Client::new();

    // Withdraw event topic
    let withdraw_topic = "0x".to_string()
        + &hex::encode(alloy::primitives::keccak256(
            b"Withdraw(bytes32,address,address,uint256)",
        ));

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "0x0",
                "toBlock": "latest",
                "address": format!("{}", config.evm.contracts.bridge),
                "topics": [withdraw_topic, format!("0x{}", hex::encode(withdraw_hash))]
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(logs) = body["result"].as_array() {
        if !logs.is_empty() {
            info!(
                "Withdrawal 0x{} was executed",
                hex::encode(&withdraw_hash.as_slice()[..8])
            );
            return Ok(true);
        }
    }

    Ok(false)
}

/// Get ERC20 balance for an account
pub async fn get_erc20_balance(
    config: &E2eConfig,
    token: Address,
    account: Address,
) -> Result<U256> {
    let client = reqwest::Client::new();

    let sel = selector("balanceOf(address)");
    let account_padded = format!("{:0>64}", hex::encode(account.as_slice()));
    let call_data = format!("0x{}{}", sel, account_padded);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", token),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;
    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre!("No result in balance query"))?;

    let balance = U256::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(balance)
}

/// Verify destination balance increased after transfer
///
/// Compares the current balance to the expected increase.
pub async fn verify_balance_increased(
    config: &E2eConfig,
    token: Address,
    account: Address,
    initial_balance: U256,
    expected_increase: U256,
) -> Result<bool> {
    let current_balance = get_erc20_balance(config, token, account).await?;

    let actual_increase = if current_balance > initial_balance {
        current_balance - initial_balance
    } else {
        U256::ZERO
    };

    info!(
        "Balance check: initial={}, current={}, expected_increase={}, actual_increase={}",
        initial_balance, current_balance, expected_increase, actual_increase
    );

    Ok(actual_increase >= expected_increase)
}

// ============================================================================
// Transfer Cycle Helpers
// ============================================================================

/// Options for a complete transfer cycle test
#[derive(Debug, Clone)]
pub struct TransferCycleOptions {
    /// Token to transfer
    pub token: Address,
    /// Amount to transfer
    pub amount: U256,
    /// Destination chain key
    pub dest_chain_key: B256,
    /// Destination account (bytes32 encoded)
    pub dest_account: B256,
    /// Whether to skip Anvil time for withdrawal delay
    pub skip_time_for_delay: bool,
    /// Timeout for approval polling
    pub approval_timeout: Duration,
    /// Timeout for withdrawal ready polling
    pub withdrawal_timeout: Duration,
}

impl Default for TransferCycleOptions {
    fn default() -> Self {
        Self {
            token: Address::ZERO,
            amount: U256::from(1_000_000u64),
            dest_chain_key: B256::ZERO,
            dest_account: B256::ZERO,
            skip_time_for_delay: true,
            approval_timeout: Duration::from_secs(60),
            withdrawal_timeout: Duration::from_secs(30),
        }
    }
}

/// Result of a complete transfer cycle
#[derive(Debug, Clone)]
pub struct TransferCycleResult {
    /// Deposit transaction hash
    pub deposit_tx: B256,
    /// Deposit nonce assigned
    pub deposit_nonce: u64,
    /// Approval info (if received)
    pub approval: Option<ApprovalInfo>,
    /// Whether withdrawal was executed
    pub withdrawal_executed: bool,
    /// Final destination balance
    pub final_balance: U256,
    /// Total time taken
    pub duration: Duration,
}
