//! Helper functions for operator execution tests
//!
//! This module contains helper functions for operator deposit detection,
//! withdrawal execution, fee verification, and batch processing tests.

use crate::services::ServiceManager;
use crate::terra::TerraClient;
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

/// Extended timeout for operator approval creation (90 seconds)
pub const OPERATOR_APPROVAL_TIMEOUT: Duration = Duration::from_secs(90);

/// Poll interval for checking Terra approvals
pub const TERRA_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Maximum time to wait for Terra approval
pub const TERRA_APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

/// Default withdrawal execution timeout
pub const WITHDRAWAL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(60);

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

    // Encode depositNonce() function call
    // Function selector for depositNonce() - from ABI: "de35f5cb"
    let call_data = "0xde35f5cb";

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

/// Get Terra chain key from ChainRegistry
pub async fn get_terra_chain_key(config: &E2eConfig) -> Result<[u8; 32]> {
    let client = reqwest::Client::new();

    // Encode getChainKeyCOSMW(string) function call
    let chain_id = &config.terra.chain_id;

    // ABI encode: function selector + offset + length + data
    // Verified with: cast sig "getChainKeyCOSMW(string)" = 0x1b69b176
    let selector = "0x1b69b176";
    let offset = format!("{:064x}", 32); // offset to string data
    let length = format!("{:064x}", chain_id.len());
    let data_padded = format!("{:0<64}", hex::encode(chain_id.as_bytes()));

    let call_data = format!("{}{}{}{}", selector, offset, length, data_padded);

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
    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    let mut chain_key = [0u8; 32];
    if bytes.len() >= 32 {
        chain_key.copy_from_slice(&bytes[..32]);
    }
    Ok(chain_key)
}

/// Encode Terra address as bytes32 (right-padded hex)
pub fn encode_terra_address(address: &str) -> [u8; 32] {
    let mut result = [0u8; 32];
    let addr_bytes = address.as_bytes();
    let len = std::cmp::min(addr_bytes.len(), 32);
    result[..len].copy_from_slice(&addr_bytes[..len]);
    result
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
    // selector: 0x095ea7b3
    let spender_padded = format!("{:0>64}", hex::encode(spender.as_slice()));
    let amount_padded = format!("{:064x}", amount);
    let call_data = format!("0x095ea7b3{}{}", spender_padded, amount_padded);

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

    Ok(tx_hash)
}

/// Execute deposit on BridgeRouter
///
/// Function signature: deposit(address,uint256,bytes32,bytes32)
/// Selector: 0x0efe6a8b (keccak256 first 4 bytes)
pub async fn execute_deposit(
    config: &E2eConfig,
    router: Address,
    token: Address,
    amount: u128,
    dest_chain_key: [u8; 32],
    dest_account: [u8; 32],
) -> Result<B256> {
    let client = reqwest::Client::new();

    // Function selector for deposit(address,uint256,bytes32,bytes32)
    // keccak256("deposit(address,uint256,bytes32,bytes32)")[0:4] = 0x0efe6a8b
    let selector = "0efe6a8b";

    // ABI encode parameters (each 32 bytes, left-padded for addresses/ints, raw for bytes32)
    let token_padded = format!("{:0>64}", hex::encode(token.as_slice()));
    let amount_padded = format!("{:064x}", amount);
    let chain_key_hex = hex::encode(dest_chain_key);
    let dest_account_hex = hex::encode(dest_account);

    let call_data = format!(
        "0x{}{}{}{}{}",
        selector, token_padded, amount_padded, chain_key_hex, dest_account_hex
    );

    debug!(
        "Executing deposit: token={}, amount={}, destChain=0x{}, destAccount=0x{}",
        token,
        amount,
        hex::encode(&dest_chain_key[..8]),
        hex::encode(&dest_account[..8])
    );

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", router),
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

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

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

/// Poll Terra bridge for approval creation
///
/// Queries the Terra bridge contract for pending approvals matching the given nonce.
pub async fn poll_terra_for_approval(
    terra_client: &TerraClient,
    bridge_address: &str,
    nonce: u64,
    timeout: Duration,
) -> Result<TerraApprovalInfo> {
    let start = Instant::now();

    while start.elapsed() < timeout {
        // Query pending approvals from Terra bridge
        let query = serde_json::json!({
            "pending_approvals": {
                "start_after": nonce.saturating_sub(1),
                "limit": 10u32
            }
        });

        match terra_client
            .query_contract_cli::<serde_json::Value>(bridge_address, &query)
            .await
        {
            Ok(result) => {
                if let Some(approvals) = result.get("approvals").and_then(|a| a.as_array()) {
                    for approval in approvals {
                        let approval_nonce =
                            approval.get("nonce").and_then(|n| n.as_u64()).unwrap_or(0);

                        if approval_nonce == nonce {
                            let amount_str = approval
                                .get("amount")
                                .and_then(|a| a.as_str())
                                .unwrap_or("0");
                            let amount = U256::from_str_radix(amount_str, 10).unwrap_or(U256::ZERO);

                            return Ok(TerraApprovalInfo {
                                nonce,
                                token: approval
                                    .get("token")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                recipient: approval
                                    .get("recipient")
                                    .and_then(|r| r.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                amount,
                                approved_at: approval
                                    .get("approved_at")
                                    .and_then(|a| a.as_u64())
                                    .unwrap_or(0),
                                cancelled: approval
                                    .get("cancelled")
                                    .and_then(|c| c.as_bool())
                                    .unwrap_or(false),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                debug!("Terra query error: {}", e);
            }
        }

        tokio::time::sleep(TERRA_POLL_INTERVAL).await;
    }

    Err(eyre::eyre!(
        "Approval for nonce {} not found within {:?}",
        nonce,
        timeout
    ))
}

// ============================================================================
// Withdrawal Helpers
// ============================================================================

/// Query withdraw delay from bridge contract
pub async fn query_withdraw_delay(config: &E2eConfig) -> Result<u64> {
    let client = reqwest::Client::new();

    // Encode withdrawDelay() function call
    // Verified with: cast sig "withdrawDelay()"
    let call_data = "0x0288a39c";

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
    // selector: 0x70a08231
    let account_padded = format!("{:0>64}", hex::encode(account.as_slice()));
    let call_data = format!("0x70a08231{}", account_padded);

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

    // feeCollector() selector
    let call_data = "0xc415b95c";

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

    // feeBps() selector
    let call_data = "0x3e4086e5";

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
    dest_chain_key: [u8; 32],
    dest_account: [u8; 32],
) -> Result<Vec<B256>> {
    let mut tx_hashes = Vec::new();
    let lock_unlock = config.evm.contracts.lock_unlock;
    let router = config.evm.contracts.router;

    // Approve total amount
    let total_amount = amount_per_deposit * (num_deposits as u128);
    approve_erc20(config, token, lock_unlock, total_amount).await?;

    // Execute deposits
    for i in 0..num_deposits {
        info!("Executing batch deposit {}/{}", i + 1, num_deposits);
        match execute_deposit(
            config,
            router,
            token,
            amount_per_deposit,
            dest_chain_key,
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

    // getWithdrawApproval(bytes32) selector
    let selector = "9211345c";
    let withdraw_hash_hex = hex::encode(withdraw_hash.as_slice());
    let call_data = format!("0x{}{}", selector, withdraw_hash_hex);

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
    if bytes.len() < 96 {
        return Ok(false);
    }

    // approvedAt is at offset 64 (third slot)
    let approved_at = u64::from_be_bytes(bytes[88..96].try_into().unwrap_or([0u8; 8]));

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
    let timestamp_hex = block_body["result"]["timestamp"]
        .as_str()
        .unwrap_or("0x0");
    let current_time = u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16)?;

    Ok(current_time > approved_at + deadline_seconds)
}

// ============================================================================
// EVM Chain Key Computation
// ============================================================================

/// Compute EVM chain key (matches ChainRegistry.getChainKeyEVM)
pub fn compute_evm_chain_key(chain_id: u64) -> [u8; 32] {
    use alloy::primitives::keccak256;

    let mut data = [0u8; 128];

    // Offset to string data (64)
    data[31] = 0x40;

    // chainId as bytes32 (big-endian, right-aligned)
    let chain_id_bytes = chain_id.to_be_bytes();
    data[32 + 24..64].copy_from_slice(&chain_id_bytes);

    // String length (3)
    data[64 + 31] = 3;

    // String data "EVM"
    data[96..99].copy_from_slice(b"EVM");

    keccak256(&data).into()
}

/// Generate a unique nonce based on current timestamp
pub fn generate_unique_nonce() -> u64 {
    999_000_000
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_terra_address() {
        let address = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let encoded = encode_terra_address(address);
        assert_eq!(&encoded[..address.len()], address.as_bytes());
    }

    #[test]
    fn test_calculate_fee() {
        // 30 bps = 0.3%
        assert_eq!(calculate_fee(1_000_000, 30), 3_000);
        // 100 bps = 1%
        assert_eq!(calculate_fee(1_000_000, 100), 10_000);
    }

    #[test]
    fn test_evm_chain_key_computation() {
        let key = compute_evm_chain_key(31337);
        assert!(!key.iter().all(|&b| b == 0), "Chain key should not be all zeros");
    }
}
