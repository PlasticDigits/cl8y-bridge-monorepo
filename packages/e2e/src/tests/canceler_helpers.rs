//! Helper functions for canceler execution tests
//!
//! This module contains helper functions for creating fraudulent approvals,
//! checking cancellation status, and computing chain keys.
#![allow(dead_code)]

use crate::E2eConfig;
use alloy::primitives::{keccak256, Address, B256};
use std::time::Duration;
use tracing::{debug, info};

use super::helpers::{chain_id4_to_bytes32, selector};

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
    pub xchain_hash_id: B256,
}

/// Create a fraudulent approval on the bridge (2-step: withdrawSubmit + withdrawApprove)
///
/// This creates an approval that has no matching deposit (fraud scenario).
/// Uses the new 2-step withdraw flow.
///
/// IMPORTANT: `src_chain_key` must have a registered bytes4 chain ID in the first 4 bytes,
/// and `token` must be a registered token. The fraud aspect comes from using a nonce
/// that has no matching deposit on the source chain.
///
/// The xchainHashId is extracted from the WithdrawSubmit event log (topics[1])
/// rather than computed locally, ensuring it always matches the contract.
pub async fn create_fraudulent_approval(
    config: &E2eConfig,
    src_chain_key: B256,
    token: Address,
    recipient: Address,
    amount: &str,
    nonce: u64,
) -> eyre::Result<FraudulentApprovalResult> {
    let client = reqwest::Client::new();

    // Create destAccount (the recipient's address as bytes32, left-padded)
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(recipient.as_slice());

    // Create a fake srcAccount
    let mut src_account_bytes = [0u8; 32];
    src_account_bytes[0..8].copy_from_slice(&nonce.to_be_bytes());
    src_account_bytes[8] = 0xff;

    // Parse amount to u128
    let amount_u256: u128 = amount.parse().unwrap_or(1234567890123456789);

    // --- Step 1: withdrawSubmit ---
    let submit_sel =
        selector("withdrawSubmit(bytes4,bytes32,bytes32,address,uint256,uint64,uint8)");
    let src_chain_bytes4 = &src_chain_key.as_slice()[..4];
    let src_chain_padded = hex::encode(chain_id4_to_bytes32(src_chain_bytes4.try_into().unwrap()));
    let src_account_hex = hex::encode(src_account_bytes);
    let dest_account_hex = hex::encode(dest_account_bytes);
    let token_padded = format!("{:0>64}", hex::encode(token.as_slice()));
    let amount_padded = format!("{:064x}", amount_u256);
    let nonce_padded = format!("{:064x}", nonce);
    let src_decimals_padded = format!("{:064x}", 18u8); // ERC20 default 18 decimals

    let submit_data = format!(
        "0x{}{}{}{}{}{}{}{}",
        submit_sel,
        src_chain_padded,
        src_account_hex,
        dest_account_hex,
        token_padded,
        amount_padded,
        nonce_padded,
        src_decimals_padded
    );

    debug!(
        "Creating fraudulent withdrawal (submit): srcChain=0x{}, token={}, recipient={}, amount={}, nonce={}",
        hex::encode(src_chain_bytes4),
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
                "data": submit_data,
                "gas": "0x200000"
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("withdrawSubmit failed: {}", error));
    }

    let submit_tx_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let submit_tx = B256::from_slice(&hex::decode(submit_tx_hex.trim_start_matches("0x"))?);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Extract xchainHashId from WithdrawSubmit event log (indexed topics[1])
    let receipt_response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [format!("0x{}", hex::encode(submit_tx))],
            "id": 1
        }))
        .send()
        .await?;

    let receipt_body: serde_json::Value = receipt_response.json().await?;
    let receipt = receipt_body
        .get("result")
        .ok_or_else(|| eyre::eyre!("No receipt for withdrawSubmit tx"))?;

    // Check receipt status
    let status = receipt
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("0x0");
    if status != "0x1" {
        return Err(eyre::eyre!(
            "withdrawSubmit reverted on-chain. Ensure srcChain (0x{}) is registered in ChainRegistry \
             and token ({}) is registered in TokenRegistry.",
            hex::encode(src_chain_bytes4),
            token
        ));
    }

    // Parse WithdrawSubmit event: topics[0] = event sig, topics[1] = xchainHashId (indexed)
    let logs = receipt
        .get("logs")
        .and_then(|l| l.as_array())
        .ok_or_else(|| eyre::eyre!("No logs in withdrawSubmit receipt"))?;

    let xchain_hash_id = logs
        .iter()
        .find_map(|log| {
            let topics = log.get("topics")?.as_array()?;
            if topics.len() >= 2 {
                let topic_hex = topics[1].as_str()?;
                let bytes = hex::decode(topic_hex.trim_start_matches("0x")).ok()?;
                if bytes.len() == 32 {
                    return Some(B256::from_slice(&bytes));
                }
            }
            None
        })
        .ok_or_else(|| eyre::eyre!("Could not extract xchainHashId from WithdrawSubmit event"))?;

    // --- Step 2: withdrawApprove ---
    let approve_sel = selector("withdrawApprove(bytes32)");
    let xchain_hash_id_hex = hex::encode(xchain_hash_id.as_slice());
    let approve_data = format!("0x{}{}", approve_sel, xchain_hash_id_hex);

    let response2 = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", config.evm.contracts.bridge),
                "data": approve_data,
                "gas": "0x100000"
            }],
            "id": 1
        }))
        .send()
        .await?;

    let body2: serde_json::Value = response2.json().await?;

    if let Some(error) = body2.get("error") {
        return Err(eyre::eyre!("withdrawApprove failed: {}", error));
    }

    let approve_tx_hex = body2["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let approve_tx = B256::from_slice(&hex::decode(approve_tx_hex.trim_start_matches("0x"))?);
    info!(
        "Fraudulent approval transaction: 0x{}, xchainHashId: 0x{}",
        hex::encode(approve_tx),
        hex::encode(xchain_hash_id)
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(FraudulentApprovalResult {
        tx_hash: approve_tx,
        xchain_hash_id,
    })
}

/// Check if a withdrawal was cancelled by querying pendingWithdraws(bytes32)
///
/// PendingWithdraw has 13 fields, cancelled is at slot 11 (offset 352).
pub async fn is_approval_cancelled(config: &E2eConfig, xchain_hash_id: B256) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    let sel = selector("pendingWithdraws(bytes32)");
    let xchain_hash_id_hex = hex::encode(xchain_hash_id.as_slice());
    let call_data = format!("0x{}{}", sel, xchain_hash_id_hex);

    debug!(
        "Checking approval status for xchainHashId=0x{}",
        hex::encode(&xchain_hash_id.as_slice()[..8])
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
        debug!("Query failed (withdrawal may not exist): {}", error);
        return Ok(false);
    }

    let result_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let bytes = hex::decode(result_hex.trim_start_matches("0x")).unwrap_or_default();

    if bytes.len() < 13 * 32 {
        debug!(
            "Response too short ({}), withdrawal may not exist",
            bytes.len()
        );
        return Ok(false);
    }

    // Check submittedAt (slot 8) to verify withdrawal exists
    let submitted_nonzero = bytes[8 * 32..9 * 32].iter().any(|&b| b != 0);
    if !submitted_nonzero {
        debug!("Withdrawal does not exist (submittedAt=0)");
        return Ok(false);
    }

    let approved = bytes.get(10 * 32 + 31).copied().unwrap_or(0) != 0;
    let cancelled = bytes.get(11 * 32 + 31).copied().unwrap_or(0) != 0;
    let executed = bytes.get(12 * 32 + 31).copied().unwrap_or(0) != 0;

    // Extract nonce (slot 6, uint64 at end of 32-byte word)
    let nonce_bytes = &bytes[6 * 32..7 * 32];
    let nonce = u64::from_be_bytes(nonce_bytes[24..32].try_into().unwrap_or([0; 8]));

    // Extract srcChain (slot 0, bytes4 at start of 32-byte word)
    let src_chain = &bytes[0..4];

    debug!(
        "Withdrawal status: approved={}, cancelled={}, executed={}, nonce={}, srcChain=0x{}",
        approved,
        cancelled,
        executed,
        nonce,
        hex::encode(src_chain)
    );

    Ok(cancelled)
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
    xchain_hash_id: B256,
) -> eyre::Result<B256> {
    let client = reqwest::Client::new();

    let sel = selector("withdrawCancel(bytes32)");
    let xchain_hash_id_hex = hex::encode(xchain_hash_id.as_slice());
    let call_data = format!("0x{}{}", sel, xchain_hash_id_hex);

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
pub async fn try_execute_withdrawal(
    config: &E2eConfig,
    xchain_hash_id: B256,
) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    let sel = selector("withdrawExecuteUnlock(bytes32)");
    let hash_hex = hex::encode(xchain_hash_id);
    let call_data = format!("0x{}{}", sel, hash_hex);

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
