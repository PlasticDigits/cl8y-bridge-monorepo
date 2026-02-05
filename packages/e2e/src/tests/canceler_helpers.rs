//! Helper functions for canceler execution tests
//!
//! This module contains helper functions for creating fraudulent approvals,
//! checking cancellation status, and computing chain keys.

use crate::E2eConfig;
use alloy::primitives::{keccak256, Address, B256};
use std::time::Duration;
use tracing::{debug, info};

// ============================================================================
// Chain Key Computation
// ============================================================================

/// Compute EVM chain key (matches ChainRegistry.getChainKeyEVM)
///
/// Computes: keccak256(abi.encode("EVM", bytes32(chainId)))
pub fn compute_evm_chain_key(chain_id: u64) -> [u8; 32] {
    // abi.encode("EVM", bytes32(chainId))
    // - Word 0: offset to string data (0x40 = 64)
    // - Word 1: chainId as bytes32 (big-endian, right-aligned)
    // - Word 2: string length (3 for "EVM")
    // - Word 3: string data "EVM" padded to 32 bytes
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

    keccak256(data).into()
}

/// Compute Cosmos/Terra chain key (matches ChainRegistry.getChainKeyCOSMW)
///
/// Computes: keccak256(abi.encode("COSMW", keccak256(abi.encode(chainId))))
pub fn compute_cosmos_chain_key(chain_id: &str) -> [u8; 32] {
    // Step 1: Compute inner hash of abi.encode(string chainId)
    let chain_id_bytes = chain_id.as_bytes();
    let len = chain_id_bytes.len();
    let padded_len = ((len + 31) / 32) * 32;
    let total_inner_size = 64 + padded_len.max(32);

    let mut inner_data = vec![0u8; total_inner_size];
    inner_data[31] = 0x20; // Offset to string data (32)
    inner_data[63] = len as u8; // String length
    inner_data[64..64 + len].copy_from_slice(chain_id_bytes);

    let inner_hash: [u8; 32] = keccak256(&inner_data).into();

    // Step 2: Compute outer hash with chain type "COSMW"
    // abi.encode("COSMW", bytes32(innerHash))
    let mut outer_data = [0u8; 128];
    outer_data[31] = 0x40; // Offset to string data (64)
    outer_data[32..64].copy_from_slice(&inner_hash); // Inner hash
    outer_data[64 + 31] = 5; // String length "COSMW"
    outer_data[96..101].copy_from_slice(b"COSMW"); // String data

    keccak256(outer_data).into()
}

// ============================================================================
// Fraudulent Approval Helpers
// ============================================================================

/// Result of creating a fraudulent approval
#[derive(Debug, Clone)]
pub struct FraudulentApprovalResult {
    /// The transaction hash
    pub tx_hash: B256,
    /// The computed withdraw hash (for querying approval status)
    pub withdraw_hash: B256,
}

/// Create a fraudulent approval on the bridge
///
/// Function signature: approveWithdraw(bytes32,address,address,bytes32,uint256,uint256,uint256,address,bool)
/// This creates an approval that has no matching deposit (fraud scenario)
pub async fn create_fraudulent_approval(
    config: &E2eConfig,
    src_chain_key: B256,
    token: Address,
    recipient: Address,
    amount: &str,
    nonce: u64,
) -> eyre::Result<FraudulentApprovalResult> {
    use super::helpers::compute_withdraw_hash;
    use alloy::primitives::U256;

    let client = reqwest::Client::new();

    // Function selector for approveWithdraw
    let selector = "7f86a1a8";

    // Create destAccount (the recipient's address as bytes32, left-padded)
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(recipient.as_slice());
    let dest_account_b256 = B256::from(dest_account_bytes);
    let dest_account = format!("{:0>64}", hex::encode(recipient.as_slice()));

    // Parse amount to u256
    let amount_u256: u128 = amount.parse().unwrap_or(1234567890123456789);

    // Compute the withdraw hash for later querying
    let withdraw_hash = compute_withdraw_hash(
        src_chain_key,
        config.evm.chain_id,
        token,
        dest_account_b256,
        U256::from(amount_u256),
        nonce,
    );

    // ABI encode all parameters
    let src_chain_key_hex = hex::encode(src_chain_key.as_slice());
    let token_padded = format!("{:0>64}", hex::encode(token.as_slice()));
    let to_padded = format!("{:0>64}", hex::encode(recipient.as_slice()));
    let amount_padded = format!("{:064x}", amount_u256);
    let nonce_padded = format!("{:064x}", nonce);
    let fee_padded = format!("{:064x}", 0u128);
    let fee_recipient_padded = format!("{:0>64}", "00");
    let deduct_from_amount = format!("{:064x}", 0u8);

    let call_data = format!(
        "0x{}{}{}{}{}{}{}{}{}{}",
        selector,
        src_chain_key_hex,
        token_padded,
        to_padded,
        dest_account,
        amount_padded,
        nonce_padded,
        fee_padded,
        fee_recipient_padded,
        deduct_from_amount
    );

    debug!(
        "Creating fraudulent approval: srcChainKey=0x{}, token={}, recipient={}, amount={}, nonce={}",
        hex::encode(&src_chain_key.as_slice()[..8]),
        token,
        recipient,
        amount,
        nonce
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
                "gas": "0x100000"
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("approveWithdraw failed: {}", error));
    }

    let tx_hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let tx_hash = B256::from_slice(&hex::decode(tx_hash_hex.trim_start_matches("0x"))?);
    info!(
        "Fraudulent approval transaction: 0x{}, withdrawHash: 0x{}",
        hex::encode(tx_hash),
        hex::encode(withdraw_hash)
    );

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(FraudulentApprovalResult {
        tx_hash,
        withdraw_hash,
    })
}

/// Check if an approval was cancelled by querying getWithdrawApproval
pub async fn is_approval_cancelled(config: &E2eConfig, withdraw_hash: B256) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    // Function selector for getWithdrawApproval(bytes32)
    let selector = "9211345c";
    let withdraw_hash_hex = hex::encode(withdraw_hash.as_slice());
    let call_data = format!("0x{}{}", selector, withdraw_hash_hex);

    debug!(
        "Checking approval status for withdrawHash=0x{}",
        hex::encode(&withdraw_hash.as_slice()[..8])
    );

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

    if let Some(error) = body.get("error") {
        debug!("Query failed (approval may not exist): {}", error);
        return Ok(false);
    }

    let result_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let bytes = hex::decode(result_hex.trim_start_matches("0x")).unwrap_or_default();

    if bytes.len() < 224 {
        debug!(
            "Response too short ({}), approval may not exist",
            bytes.len()
        );
        return Ok(false);
    }

    let is_approved = bytes.get(96 + 31).copied().unwrap_or(0) != 0;
    if !is_approved {
        debug!("Approval does not exist (isApproved=false)");
        return Ok(false);
    }

    let is_cancelled = bytes.get(160 + 31).copied().unwrap_or(0) != 0;

    debug!(
        "Approval status: isApproved={}, cancelled={}",
        is_approved, is_cancelled
    );

    Ok(is_cancelled)
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

/// Check if canceler health endpoint is reachable
pub async fn check_canceler_health() -> bool {
    let health_port: u16 = std::env::var("CANCELER_HEALTH_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9099);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    let url = format!("http://localhost:{}/health", health_port);

    match client.get(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Cancel an approval directly via contract call (requires CANCELER_ROLE)
pub async fn cancel_approval_directly(
    config: &E2eConfig,
    withdraw_hash: B256,
) -> eyre::Result<B256> {
    let client = reqwest::Client::new();

    let selector = "ac23b667";
    let withdraw_hash_hex = hex::encode(withdraw_hash.as_slice());
    let call_data = format!("0x{}{}", selector, withdraw_hash_hex);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data,
                "gas": "0x50000"
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("Cancel failed: {}", error));
    }

    let tx_hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let tx_hash = B256::from_slice(&hex::decode(tx_hash_hex.trim_start_matches("0x"))?);
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(tx_hash)
}

/// Try to execute withdrawal (should fail for cancelled approvals)
pub async fn try_execute_withdrawal(config: &E2eConfig, withdraw_hash: B256) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    let selector = "2e1a7d4d";
    let hash_hex = hex::encode(withdraw_hash);
    let call_data = format!("0x{}{}", selector, hash_hex);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if body.get("error").is_some() {
        return Ok(false);
    }

    let result = body["result"].as_str().unwrap_or("0x");
    if result == "0x" || result == "0x0" {
        return Ok(false);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_chain_key_computation() {
        let key = compute_evm_chain_key(31337);
        assert!(
            !key.iter().all(|&b| b == 0),
            "Chain key should not be all zeros"
        );
    }

    #[test]
    fn test_cosmos_chain_key_computation() {
        let key = compute_cosmos_chain_key("localterra");
        assert!(
            !key.iter().all(|&b| b == 0),
            "Chain key should not be all zeros"
        );
    }
}
