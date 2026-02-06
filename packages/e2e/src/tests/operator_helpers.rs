//! Helper functions for operator execution tests
//!
//! This module contains helper functions for operator deposit detection,
//! withdrawal execution, fee verification, and batch processing tests.

use crate::services::ServiceManager;
use crate::terra::TerraClient;
use crate::tests::helpers::{chain_id4_to_bytes32, selector};
use crate::E2eConfig;
use alloy::primitives::{Address, B256, U256};
use eyre::Result;
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

// ============================================================================
// Constants
// ============================================================================

/// Default transfer amount for tests (1 token with 6 decimals)
pub const DEFAULT_TRANSFER_AMOUNT: u128 = 1_000_000;

/// Extended timeout for operator approval creation (120 seconds)
/// Increased to account for:
/// - Operator polling interval (5s)
/// - Transaction broadcast time
/// - Block confirmation time (~1s LocalTerra, ~6s mainnet)
/// - Transaction indexing delay
pub const OPERATOR_APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

/// Initial poll interval for checking Terra approvals (fast at start)
pub const TERRA_POLL_INITIAL_INTERVAL: Duration = Duration::from_millis(500);

/// Maximum poll interval for checking Terra approvals
pub const TERRA_POLL_MAX_INTERVAL: Duration = Duration::from_secs(5);

/// Maximum time to wait for Terra approval (180 seconds)
/// Increased to handle:
/// - Operator detection delay
/// - Transaction confirmation waiting
/// - Block propagation time
pub const TERRA_APPROVAL_TIMEOUT: Duration = Duration::from_secs(180);

/// Default withdrawal execution timeout (90 seconds)
pub const WITHDRAWAL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(90);

/// Legacy alias for backwards compatibility
pub const TERRA_POLL_INTERVAL: Duration = TERRA_POLL_INITIAL_INTERVAL;

// ============================================================================
// Operator Service Helpers
// ============================================================================

/// Check if operator service is running
pub fn is_operator_running() -> bool {
    let project_root = Path::new("/home/answorld/repos/cl8y-bridge-monorepo");
    let manager = ServiceManager::new(project_root);
    manager.is_operator_running()
}

/// Check operator health endpoint (if available)
pub async fn check_operator_health() -> bool {
    // Operator may have a health endpoint - try common ports
    let health_ports = [9090, 9091, 9098];
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    for port in health_ports {
        let url = format!("http://localhost:{}/health", port);
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return true;
            }
        }
    }

    // Fall back to checking if process is running
    is_operator_running()
}

// ============================================================================
// Deposit Helpers
// ============================================================================

/// Query deposit nonce from bridge contract
pub async fn query_deposit_nonce(config: &E2eConfig) -> Result<u64> {
    let client = reqwest::Client::new();

    let call_data = format!("0x{}", selector("depositNonce()"));

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

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "EVM RPC returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    // Parse hex to u64
    let nonce = u64::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(nonce)
}

/// Get Terra chain ID (bytes4) from ChainRegistry using identifier "terraclassic_{chain_id}"
///
/// Uses computeIdentifierHash + getChainIdFromHash to look up the bytes4 chain ID.
pub async fn get_terra_chain_key(config: &E2eConfig) -> Result<[u8; 4]> {
    let client = reqwest::Client::new();
    let identifier = format!("terraclassic_{}", config.terra.chain_id);

    // Step 1: computeIdentifierHash(string)
    let sel1 = selector("computeIdentifierHash(string)");
    let offset = format!("{:064x}", 32);
    let length = format!("{:064x}", identifier.len());
    let data_padded = format!("{:0<64}", hex::encode(identifier.as_bytes()));
    let call_data = format!("0x{}{}{}{}", sel1, offset, length, data_padded);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.chain_registry),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;
    let hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result from computeIdentifierHash"))?;

    // Step 2: getChainIdFromHash(bytes32)
    let sel2 = selector("getChainIdFromHash(bytes32)");
    let hash_clean = hash_hex.trim_start_matches("0x");
    let call_data2 = format!("0x{}{}", sel2, hash_clean);

    let response2 = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.chain_registry),
                "data": call_data2
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body2: serde_json::Value = response2.json().await?;
    let chain_id_hex = body2["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result from getChainIdFromHash"))?;

    // Parse bytes4 from ABI-encoded result (left-aligned in 32 bytes)
    let bytes = hex::decode(chain_id_hex.trim_start_matches("0x"))?;
    if bytes.len() < 4 {
        return Err(eyre::eyre!("Invalid chain ID response: too short"));
    }
    let mut chain_id = [0u8; 4];
    chain_id.copy_from_slice(&bytes[..4]);
    Ok(chain_id)
}

/// Encode Terra address as bytes32 using bech32 decode + left-pad
///
/// Uses the unified encoding from multichain-rs to ensure hash consistency
/// across EVM deposits, Terra WithdrawSubmit, and operator hash computation.
/// The bech32 address is decoded to raw 20 bytes, then left-padded to 32 bytes.
pub fn encode_terra_address(address: &str) -> [u8; 32] {
    multichain_rs::hash::encode_terra_address_to_bytes32(address).unwrap_or_else(|e| {
        panic!("Failed to encode Terra address '{}': {}", address, e);
    })
}

/// Approve ERC20 token spend
pub async fn approve_erc20(
    config: &E2eConfig,
    token: Address,
    spender: Address,
    amount: u128,
) -> Result<B256> {
    let client = reqwest::Client::new();

    // Encode approve(address,uint256) function call
    let sel = selector("approve(address,uint256)");
    let spender_padded = format!("{:0>64}", hex::encode(spender.as_slice()));
    let amount_padded = format!("{:064x}", amount);
    let call_data = format!("0x{}{}{}", sel, spender_padded, amount_padded);

    debug!(
        "Approving token {} for spender {} amount {}",
        token, spender, amount
    );

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", token),
                "data": call_data,
                "gas": "0x30000"
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("Approve failed: {}", error));
    }

    let tx_hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let tx_hash = B256::from_slice(&hex::decode(tx_hash_hex.trim_start_matches("0x"))?);

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify transaction succeeded
    match verify_tx_receipt(config, tx_hash).await {
        Ok(true) => {
            debug!("Token approval confirmed successfully");
        }
        Ok(false) => {
            return Err(eyre::eyre!(
                "Token approval reverted on-chain (tx=0x{}). \
                 Check that the token address is valid and the test account has sufficient balance.",
                hex::encode(&tx_hash.as_slice()[..8])
            ));
        }
        Err(e) => {
            warn!("Could not verify approval receipt: {}", e);
            // Continue anyway - may still have succeeded
        }
    }

    Ok(tx_hash)
}

/// Execute deposit on Bridge via depositERC20
///
/// Function signature: depositERC20(address,uint256,bytes4,bytes32)
///
/// Automatically approves both the Bridge contract (for fee transfer) and
/// the LockUnlock adapter (for token locking) before executing the deposit.
pub async fn execute_deposit(
    config: &E2eConfig,
    token: Address,
    amount: u128,
    dest_chain_id: [u8; 4],
    dest_account: [u8; 32],
) -> Result<B256> {
    // Approve both Bridge (for fee transferFrom) and LockUnlock (for lock transferFrom)
    approve_erc20(config, token, config.evm.contracts.bridge, amount).await?;
    approve_erc20(config, token, config.evm.contracts.lock_unlock, amount).await?;

    let client = reqwest::Client::new();

    let sel = selector("depositERC20(address,uint256,bytes4,bytes32)");

    // ABI encode parameters
    let token_padded = format!("{:0>64}", hex::encode(token.as_slice()));
    let amount_padded = format!("{:064x}", amount);
    // bytes4 is ABI-encoded left-aligned in 32 bytes (right-padded with zeros)
    let chain_id_padded = hex::encode(chain_id4_to_bytes32(dest_chain_id));
    let dest_account_hex = hex::encode(dest_account);

    let call_data = format!(
        "0x{}{}{}{}{}",
        sel, token_padded, amount_padded, chain_id_padded, dest_account_hex
    );

    debug!(
        "Executing depositERC20: token={}, amount={}, destChain=0x{}, destAccount=0x{}",
        token,
        amount,
        hex::encode(dest_chain_id),
        hex::encode(&dest_account[..8])
    );

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data,
                "gas": "0x200000"
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("Deposit failed: {}", error));
    }

    let tx_hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let tx_hash = B256::from_slice(&hex::decode(tx_hash_hex.trim_start_matches("0x"))?);
    info!("Deposit transaction submitted: 0x{}", hex::encode(tx_hash));

    // Wait for confirmation and verify transaction succeeded
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify transaction receipt status - critical to detect on-chain reverts
    match verify_tx_receipt(config, tx_hash).await {
        Ok(true) => {
            debug!("Deposit transaction confirmed successfully");
        }
        Ok(false) => {
            return Err(eyre::eyre!(
                "Deposit transaction reverted on-chain (tx=0x{}). \
                 Common causes: token not registered in TokenRegistry for destination chain, \
                 guard check failed, or access control denied. \
                 Check contract state and token registration.",
                hex::encode(&tx_hash.as_slice()[..8])
            ));
        }
        Err(e) => {
            warn!("Could not verify transaction receipt: {}", e);
            // Continue anyway - may still have succeeded
        }
    }

    Ok(tx_hash)
}

/// Verify a transaction succeeded by checking its receipt status
///
/// Returns Ok(true) if status is 0x1 (success), Ok(false) if status is 0x0 (reverted),
/// or Err if the receipt is not yet available.
async fn verify_tx_receipt(config: &E2eConfig, tx_hash: B256) -> Result<bool> {
    let client = reqwest::Client::new();

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [format!("0x{}", hex::encode(tx_hash))],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if body["result"].is_null() {
        return Err(eyre::eyre!(
            "Transaction receipt not found (tx may still be pending)"
        ));
    }

    let status = body["result"]["status"].as_str().unwrap_or("0x0");

    Ok(status == "0x1")
}

// ============================================================================
// Terra WithdrawSubmit Helper (V2 user-initiated step)
// ============================================================================

/// Submit a V2 WithdrawSubmit on Terra to create the pending withdrawal entry.
///
/// In the V2 flow, the **user** must call `WithdrawSubmit` on the destination chain
/// (Terra) before the operator can approve it. This function simulates that step.
///
/// # Arguments
/// * `terra_client` - Terra client for executing the transaction
/// * `bridge_address` - Terra bridge contract address
/// * `src_chain_id` - Source chain ID (4 bytes, e.g., `[0,0,0,1]` for EVM)
/// * `src_account` - EVM depositor address encoded as bytes32
/// * `token` - Terra-side token denom (e.g., "uluna")
/// * `recipient` - Terra recipient address
/// * `amount` - Amount in source chain decimals
/// * `nonce` - Deposit nonce from the source chain
pub async fn submit_withdraw_on_terra(
    terra_client: &TerraClient,
    bridge_address: &str,
    src_chain_id: [u8; 4],
    src_account: [u8; 32],
    token: &str,
    recipient: &str,
    amount: u128,
    nonce: u64,
) -> Result<String> {
    use base64::Engine;
    let encoder = base64::engine::general_purpose::STANDARD;

    let msg = serde_json::json!({
        "withdraw_submit": {
            "src_chain": encoder.encode(src_chain_id),
            "src_account": encoder.encode(src_account),
            "token": token,
            "recipient": recipient,
            "amount": amount.to_string(),
            "nonce": nonce
        }
    });

    info!(
        nonce = nonce,
        token = token,
        recipient = recipient,
        amount = amount,
        src_chain = hex::encode(src_chain_id),
        "Submitting WithdrawSubmit on Terra (V2 user step)"
    );

    let tx_hash = terra_client
        .execute_contract(bridge_address, &msg, None)
        .await
        .map_err(|e| eyre::eyre!("WithdrawSubmit on Terra failed: {}", e))?;

    info!(
        nonce = nonce,
        tx_hash = %tx_hash,
        "WithdrawSubmit transaction submitted on Terra"
    );

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(6)).await;

    Ok(tx_hash)
}

// ============================================================================
// Terra Approval Helpers
// ============================================================================

/// Terra approval information from Terra bridge
#[derive(Debug, Clone)]
pub struct TerraApprovalInfo {
    /// Deposit nonce
    pub nonce: u64,
    /// Token address/denom
    pub token: String,
    /// Recipient address
    pub recipient: String,
    /// Amount approved
    pub amount: U256,
    /// Approval timestamp
    pub approved_at: u64,
    /// Whether cancelled
    pub cancelled: bool,
}

/// Poll Terra bridge for an **approved** pending withdrawal matching the given nonce.
///
/// In the V2 flow, `WithdrawSubmit` creates the entry (with `approved: false`),
/// and the operator later calls `WithdrawApprove` (setting `approved: true`).
/// This function waits until the entry exists AND is approved.
///
/// Uses the `pending_withdrawals` paginated list query to discover entries,
/// then matches by nonce. Uses exponential backoff starting with fast polling
/// and slowing down over time.
pub async fn poll_terra_for_approval(
    terra_client: &TerraClient,
    bridge_address: &str,
    nonce: u64,
    timeout: Duration,
) -> Result<TerraApprovalInfo> {
    let start = Instant::now();
    let mut poll_interval = TERRA_POLL_INITIAL_INTERVAL;
    let mut attempt = 0;
    let mut found_unapproved = false;

    info!(
        nonce = nonce,
        timeout_secs = timeout.as_secs(),
        "Polling Terra for approved withdrawal (nonce match)"
    );

    while start.elapsed() < timeout {
        attempt += 1;

        // Paginate through all pending withdrawals looking for our nonce
        let mut start_after: Option<String> = None;
        let page_limit = 30u32;

        loop {
            let query = if let Some(ref cursor) = start_after {
                serde_json::json!({
                    "pending_withdrawals": {
                        "start_after": cursor,
                        "limit": page_limit
                    }
                })
            } else {
                serde_json::json!({
                    "pending_withdrawals": {
                        "limit": page_limit
                    }
                })
            };

            match terra_client
                .query_contract_cli::<serde_json::Value>(bridge_address, &query)
                .await
            {
                Ok(result) => {
                    if let Some(withdrawals) = result.get("withdrawals").and_then(|w| w.as_array())
                    {
                        for entry in withdrawals {
                            let entry_nonce =
                                entry.get("nonce").and_then(|n| n.as_u64()).unwrap_or(0);

                            if entry_nonce == nonce {
                                let is_approved = entry
                                    .get("approved")
                                    .and_then(|a| a.as_bool())
                                    .unwrap_or(false);

                                if !is_approved {
                                    // Entry exists but not yet approved by operator
                                    if !found_unapproved {
                                        info!(
                                            nonce = nonce,
                                            attempt = attempt,
                                            "Found unapproved withdrawal, waiting for operator approval..."
                                        );
                                        found_unapproved = true;
                                    }
                                    // Don't return yet - keep polling until approved
                                    break; // break inner pagination loop, retry outer
                                }

                                let amount_str =
                                    entry.get("amount").and_then(|a| a.as_str()).unwrap_or("0");
                                let amount =
                                    U256::from_str_radix(amount_str, 10).unwrap_or(U256::ZERO);

                                info!(
                                    nonce = nonce,
                                    attempt = attempt,
                                    elapsed_secs = start.elapsed().as_secs(),
                                    "Found approved Terra withdrawal"
                                );

                                return Ok(TerraApprovalInfo {
                                    nonce,
                                    token: entry
                                        .get("token")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    recipient: entry
                                        .get("recipient")
                                        .and_then(|r| r.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                    amount,
                                    approved_at: entry
                                        .get("approved_at")
                                        .and_then(|a| a.as_u64())
                                        .unwrap_or(0),
                                    cancelled: entry
                                        .get("cancelled")
                                        .and_then(|c| c.as_bool())
                                        .unwrap_or(false),
                                });
                            }
                        }

                        // If we got a full page, paginate to next page
                        if withdrawals.len() == page_limit as usize {
                            if let Some(last) = withdrawals.last() {
                                start_after = last
                                    .get("withdraw_hash")
                                    .and_then(|h| h.as_str())
                                    .map(|s| s.to_string());
                                continue; // fetch next page
                            }
                        }
                    }
                }
                Err(e) => {
                    debug!(
                        nonce = nonce,
                        attempt = attempt,
                        error = %e,
                        "Terra query error (will retry)"
                    );
                }
            }

            // No more pages to fetch for this attempt
            break;
        }

        // Log progress periodically
        if attempt % 10 == 0 {
            debug!(
                nonce = nonce,
                attempt = attempt,
                elapsed_secs = start.elapsed().as_secs(),
                found_unapproved = found_unapproved,
                "Still waiting for Terra approved withdrawal"
            );
        }

        tokio::time::sleep(poll_interval).await;

        // Exponential backoff with cap
        poll_interval = std::cmp::min(poll_interval * 2, TERRA_POLL_MAX_INTERVAL);
    }

    if found_unapproved {
        Err(eyre::eyre!(
            "Withdrawal for nonce {} was submitted but NOT approved by operator within {:?} ({} attempts). \
             The operator may have failed to call WithdrawApprove.",
            nonce,
            timeout,
            attempt
        ))
    } else {
        Err(eyre::eyre!(
            "Pending withdrawal for nonce {} not found within {:?} ({} attempts). \
             Ensure WithdrawSubmit was called on Terra after the EVM deposit.",
            nonce,
            timeout,
            attempt
        ))
    }
}

// ============================================================================
// Withdrawal Helpers
// ============================================================================

/// Query cancel window (formerly "withdraw delay") from bridge contract
pub async fn query_cancel_window(config: &E2eConfig) -> Result<u64> {
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

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "EVM RPC returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let delay = u64::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(delay)
}

/// Get ERC20 token balance
pub async fn get_erc20_balance(
    config: &E2eConfig,
    token: Address,
    account: Address,
) -> Result<U256> {
    let client = reqwest::Client::new();

    // Encode balanceOf(address) function call
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
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let balance = U256::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(balance)
}

// ============================================================================
// Fee Collection Helpers
// ============================================================================

/// Query fee collector address from bridge
pub async fn query_fee_collector(config: &E2eConfig) -> Result<Address> {
    let client = reqwest::Client::new();

    let call_data = format!("0x{}", selector("feeCollector()"));

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
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    if bytes.len() < 32 {
        return Err(eyre::eyre!("Invalid fee collector response"));
    }

    Ok(Address::from_slice(&bytes[12..32]))
}

/// Query fee BPS from bridge
pub async fn query_fee_bps(config: &E2eConfig) -> Result<u64> {
    let client = reqwest::Client::new();

    let call_data = format!("0x{}", selector("feeBps()"));

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
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let bps = u64::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(bps)
}

/// Calculate expected fee for an amount
pub fn calculate_fee(amount: u128, fee_bps: u64) -> u128 {
    (amount * fee_bps as u128) / 10_000
}

// ============================================================================
// Batch Processing Helpers
// ============================================================================

/// Execute multiple deposits in a batch
pub async fn execute_batch_deposits(
    config: &E2eConfig,
    token: Address,
    amount_per_deposit: u128,
    num_deposits: u32,
    dest_chain_id: [u8; 4],
    dest_account: [u8; 32],
) -> Result<Vec<B256>> {
    let mut tx_hashes = Vec::new();
    let lock_unlock = config.evm.contracts.lock_unlock;

    // Approve total amount
    let total_amount = amount_per_deposit * (num_deposits as u128);
    approve_erc20(config, token, lock_unlock, total_amount).await?;

    // Execute deposits
    for i in 0..num_deposits {
        info!("Executing batch deposit {}/{}", i + 1, num_deposits);
        match execute_deposit(
            config,
            token,
            amount_per_deposit,
            dest_chain_id,
            dest_account,
        )
        .await
        {
            Ok(tx) => tx_hashes.push(tx),
            Err(e) => {
                warn!("Batch deposit {} failed: {}", i + 1, e);
                return Err(e);
            }
        }
        // Small delay between deposits
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Ok(tx_hashes)
}

/// Wait for multiple approvals to be created
pub async fn wait_for_batch_approvals(
    config: &E2eConfig,
    start_nonce: u64,
    num_approvals: u32,
    timeout: Duration,
) -> Result<u32> {
    use crate::transfer_helpers::poll_for_approval;

    let start = Instant::now();
    let mut found = 0u32;

    for i in 0..num_approvals {
        let nonce = start_nonce + (i as u64) + 1;
        let remaining = timeout.saturating_sub(start.elapsed());

        if remaining.is_zero() {
            break;
        }

        match poll_for_approval(config, nonce, remaining).await {
            Ok(_) => {
                found += 1;
                info!("Found approval for nonce {}", nonce);
            }
            Err(e) => {
                warn!("Approval for nonce {} not found: {}", nonce, e);
            }
        }
    }

    Ok(found)
}

// ============================================================================
// Approval Timeout Helpers
// ============================================================================

/// Check if an approval has timed out (past its deadline)
pub async fn is_approval_timed_out(
    config: &E2eConfig,
    withdraw_hash: B256,
    deadline_seconds: u64,
) -> Result<bool> {
    let client = reqwest::Client::new();

    let sel = selector("pendingWithdraws(bytes32)");
    let withdraw_hash_hex = hex::encode(withdraw_hash.as_slice());
    let call_data = format!("0x{}{}", sel, withdraw_hash_hex);

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
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let bytes = hex::decode(result_hex.trim_start_matches("0x"))?;
    if bytes.len() < 13 * 32 {
        return Ok(false);
    }

    // approvedAt is at slot 9 (offset 288) in PendingWithdraw struct
    let approved_at_offset = 9 * 32;
    let approved_at = u64::from_be_bytes(
        bytes[approved_at_offset + 24..approved_at_offset + 32]
            .try_into()
            .unwrap_or([0u8; 8]),
    );

    if approved_at == 0 {
        return Ok(false);
    }

    // Get current block timestamp
    let block_response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": ["latest", false],
            "id": 1
        }))
        .send()
        .await?;

    let block_body: serde_json::Value = block_response.json().await?;
    let timestamp_hex = block_body["result"]["timestamp"].as_str().unwrap_or("0x0");
    let current_time = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)?;

    Ok(current_time > approved_at + deadline_seconds)
}

// ============================================================================
// Re-exports from token_diagnostics module
// ============================================================================

// Re-export token diagnostics functions for backwards compatibility
pub use super::token_diagnostics::{
    compute_evm_chain_key, generate_unique_nonce, is_token_registered_for_chain, verify_token_setup,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_terra_address() {
        let address = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let encoded = encode_terra_address(address);
        // Should be bech32-decoded raw 20 bytes, left-padded to 32 bytes
        // First 12 bytes should be zero-padding
        assert_eq!(&encoded[..12], &[0u8; 12]);
        // Last 20 bytes should be the raw address bytes (non-zero)
        assert_ne!(&encoded[12..32], &[0u8; 20]);
        // Same address should produce same encoding
        let encoded2 = encode_terra_address(address);
        assert_eq!(encoded, encoded2);
    }

    #[test]
    fn test_calculate_fee() {
        // 30 bps = 0.3%
        assert_eq!(calculate_fee(1_000_000, 30), 3_000);
        // 100 bps = 1%
        assert_eq!(calculate_fee(1_000_000, 100), 10_000);
    }
}
