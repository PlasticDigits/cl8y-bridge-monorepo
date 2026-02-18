//! Test helper functions for E2E tests
//!
//! This module contains helper functions used by test cases.

use crate::E2eConfig;
use alloy::primitives::{keccak256, Address, B256, U256};
use eyre::Result;
use std::time::Duration;
use tracing::{debug, info};
use url::Url;

/// Compute the 4-byte function selector from a Solidity function signature.
///
/// Example: `selector("getCancelWindow()")` returns the first 4 bytes of keccak256.
pub(crate) fn selector(sig: &str) -> String {
    hex::encode(&keccak256(sig.as_bytes())[..4])
}

/// Query cancel window (formerly "withdraw delay") from bridge contract
pub(crate) async fn query_cancel_window(config: &E2eConfig) -> eyre::Result<u64> {
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

/// Query deposit nonce from bridge contract
pub(crate) async fn query_deposit_nonce(config: &E2eConfig) -> eyre::Result<u64> {
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

/// Query if a contract has code deployed
pub(crate) async fn query_contract_code(
    config: &E2eConfig,
    address: Address,
) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getCode",
            "params": [format!("{}", address), "latest"],
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

    let code = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    // Contract has code if result is not "0x" or "0x0"
    Ok(code != "0x" && code != "0x0" && code.len() > 4)
}

/// Query EVM chain ID (bytes4) from ChainRegistry using identifier "evm_{chain_id}"
///
/// Uses computeIdentifierHash + getChainIdFromHash to look up the bytes4 chain ID.
pub(crate) async fn query_evm_chain_key(
    config: &E2eConfig,
    chain_id: u64,
) -> eyre::Result<[u8; 4]> {
    let identifier = format!("evm_{}", chain_id);
    query_chain_id_by_identifier(config, &identifier).await
}

/// Look up a chain's bytes4 ID by its string identifier
///
/// Step 1: computeIdentifierHash(string) -> bytes32
/// Step 2: getChainIdFromHash(bytes32) -> bytes4
async fn query_chain_id_by_identifier(
    config: &E2eConfig,
    identifier: &str,
) -> eyre::Result<[u8; 4]> {
    let client = reqwest::Client::new();

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

    // Parse bytes4 from the ABI-encoded result (left-aligned in 32 bytes)
    let bytes = hex::decode(chain_id_hex.trim_start_matches("0x"))?;
    if bytes.len() < 4 {
        return Err(eyre::eyre!("Invalid chain ID response: too short"));
    }
    let mut chain_id = [0u8; 4];
    chain_id.copy_from_slice(&bytes[..4]);
    Ok(chain_id)
}

/// Check if a chain is registered in ChainRegistry by its bytes4 chain ID
pub(crate) async fn is_chain_key_registered(
    config: &E2eConfig,
    chain_id: [u8; 4],
) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    // Encode isChainRegistered(bytes4) function call
    // bytes4 is ABI-encoded as left-aligned in 32 bytes (right-padded with zeros)
    let sel = selector("isChainRegistered(bytes4)");
    let chain_id_padded = hex::encode(chain_id4_to_bytes32(chain_id));
    let call_data = format!("0x{}{}", sel, chain_id_padded);

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

    // Parse result - boolean is returned as 32 bytes with 1 or 0
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    Ok(bytes.last().copied().unwrap_or(0) != 0)
}

/// Query if account has role from AccessManager
pub(crate) async fn query_has_role(
    config: &E2eConfig,
    role_id: u64,
    account: Address,
) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    // Encode hasRole(uint64,address) function call
    let sel = selector("hasRole(uint64,address)");
    let role_padded = format!("{:064x}", role_id);
    let addr_padded = format!("{:0>64}", hex::encode(account.as_slice()));
    let call_data = format!("0x{}{}{}", sel, role_padded, addr_padded);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.access_manager),
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

    // The result is (bool, uint32), we just check the first 32 bytes for the bool
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    if bytes.len() >= 32 {
        // First 32 bytes is the bool (padded)
        Ok(bytes[31] != 0)
    } else {
        Ok(false)
    }
}

/// Query Terra bridge withdraw delay via LCD
pub(crate) async fn query_terra_bridge_delay(
    config: &E2eConfig,
    bridge_address: &str,
) -> eyre::Result<u64> {
    let client = reqwest::Client::new();

    // Build smart query
    let query = serde_json::json!({ "withdraw_delay": {} });
    let query_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        serde_json::to_vec(&query)?,
    );

    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
        config.terra.lcd_url, bridge_address, query_b64
    );

    let response =
        tokio::time::timeout(std::time::Duration::from_secs(10), client.get(&url).send())
            .await
            .map_err(|_| eyre::eyre!("Timeout querying Terra bridge"))??;

    if !response.status().is_success() {
        return Err(eyre::eyre!(
            "Terra LCD returned status: {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response.json().await?;

    body["data"]["delay_seconds"]
        .as_u64()
        .or_else(|| body["data"]["delay"].as_u64())
        .ok_or_else(|| eyre::eyre!("Could not parse delay from response"))
}

/// Get ERC20 token balance
pub(crate) async fn get_erc20_balance(
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

/// Get Terra chain ID (bytes4) from ChainRegistry using identifier "terraclassic_{chain_id}"
pub(crate) async fn get_terra_chain_key(config: &E2eConfig) -> Result<[u8; 4]> {
    let identifier = format!("terraclassic_{}", config.terra.chain_id);
    query_chain_id_by_identifier(config, &identifier).await
}

/// Encode Terra address as bytes32 using bech32 decode + left-pad
///
/// Uses the unified encoding from multichain-rs to ensure hash consistency
/// across EVM deposits, Terra WithdrawSubmit, and operator hash computation.
/// The bech32 address is decoded to raw 20 bytes, then left-padded to 32 bytes.
pub(crate) fn encode_terra_address(address: &str) -> [u8; 32] {
    multichain_rs::hash::encode_terra_address_to_bytes32(address).unwrap_or_else(|e| {
        panic!("Failed to encode Terra address '{}': {}", address, e);
    })
}

/// Approve ERC20 token spend
pub(crate) async fn approve_erc20(
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

/// Execute deposit on Bridge via depositERC20
///
/// Function signature: depositERC20(address,uint256,bytes4,bytes32)
///
/// Automatically approves both the Bridge contract (for fee transfer) and
/// the LockUnlock adapter (for token locking) before executing the deposit.
pub(crate) async fn execute_deposit(
    config: &E2eConfig,
    token: Address,
    amount: u128,
    dest_chain_id: [u8; 4],
    dest_account: [u8; 32],
) -> Result<B256> {
    // Single approval: Bridge does fee + net transfer to LockUnlock
    approve_erc20(config, token, config.evm.contracts.bridge, amount).await?;

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

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify transaction succeeded
    match verify_tx_success(config, tx_hash).await {
        Ok(true) => Ok(tx_hash),
        Ok(false) => Err(eyre::eyre!("Deposit transaction failed")),
        Err(e) => {
            tracing::warn!("Could not verify transaction: {}", e);
            Ok(tx_hash) // Return hash anyway, caller can check
        }
    }
}

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
/// Step 1: withdrawSubmit(bytes4,bytes32,bytes32,address,uint256,uint64,uint8) - creates pending withdrawal
/// Step 2: withdrawApprove(bytes32) - operator approves it
///
/// IMPORTANT: `src_chain_key` must have a registered bytes4 chain ID in the first 4 bytes,
/// and `token` must be a registered token. The fraud aspect comes from using a nonce
/// that has no matching deposit on the source chain.
///
/// The xchainHashId is extracted from the WithdrawSubmit event log (topics[1])
/// rather than computed locally, ensuring it always matches the contract.
pub(crate) async fn create_fraudulent_approval(
    config: &E2eConfig,
    src_chain_key: B256,
    token: Address,
    recipient: Address,
    amount: &str,
    nonce: u64,
) -> Result<FraudulentApprovalResult> {
    let client = reqwest::Client::new();

    // Create destAccount (the recipient's address as bytes32, left-padded with zeros)
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(recipient.as_slice());

    // Create a fake srcAccount (just a random bytes32)
    let mut src_account_bytes = [0u8; 32];
    src_account_bytes[0..8].copy_from_slice(&nonce.to_be_bytes());
    src_account_bytes[8] = 0xff; // marker

    // Parse amount to u128
    let amount_u256: u128 = amount.parse().unwrap_or(1234567890123456789);

    // --- Step 1: withdrawSubmit ---
    let submit_sel =
        selector("withdrawSubmit(bytes4,bytes32,bytes32,address,uint256,uint64,uint8)");

    // srcChain is the first 4 bytes of src_chain_key, ABI-encoded as bytes4 (left-aligned in 32 bytes)
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
        nonce,
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

    let tx_hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;

    let submit_tx_hash = B256::from_slice(&hex::decode(tx_hash_hex.trim_start_matches("0x"))?);

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Extract xchainHashId from WithdrawSubmit event log (indexed topics[1])
    let receipt_response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [format!("0x{}", hex::encode(submit_tx_hash))],
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

    let approve_tx_hash = B256::from_slice(&hex::decode(approve_tx_hex.trim_start_matches("0x"))?);
    info!(
        "Fraudulent approval transaction: 0x{}, xchainHashId: 0x{}",
        hex::encode(approve_tx_hash),
        hex::encode(xchain_hash_id)
    );

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(FraudulentApprovalResult {
        tx_hash: approve_tx_hash,
        xchain_hash_id,
    })
}

/// Check if a withdrawal was cancelled by querying pendingWithdraws(bytes32)
///
/// PendingWithdraw struct layout (each field is 32 bytes in ABI encoding):
/// - slot 0: srcChain (bytes4, left-aligned in 32 bytes)
/// - slot 1: srcAccount (bytes32)
/// - slot 2: destAccount (bytes32)
/// - slot 3: token (address, left-padded in 32 bytes)
/// - slot 4: recipient (address, left-padded in 32 bytes)
/// - slot 5: amount (uint256)
/// - slot 6: nonce (uint64, left-padded in 32 bytes)
/// - slot 7: srcDecimals (uint8, right-aligned in 32 bytes)
/// - slot 8: destDecimals (uint8, right-aligned in 32 bytes)
/// - slot 9: operatorGas (uint256)
/// - slot 10: submittedAt (uint256)
/// - slot 11: approvedAt (uint256)
/// - slot 12: approved (bool)
/// - slot 13: cancelled (bool)
/// - slot 14: executed (bool)
pub(crate) async fn is_approval_cancelled(
    config: &E2eConfig,
    xchain_hash_id: B256,
) -> Result<bool> {
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

    // PendingWithdraw has 15 fields * 32 bytes = 480 bytes
    if bytes.len() < 15 * 32 {
        debug!(
            "Response too short ({}), withdrawal may not exist",
            bytes.len()
        );
        return Ok(false);
    }

    // Check submittedAt (slot 10, offset 320) to verify it exists
    let submitted_at_offset = 10 * 32;
    let submitted_at_byte = bytes.get(submitted_at_offset + 31).copied().unwrap_or(0);
    let submitted_at_nonzero = bytes[submitted_at_offset..submitted_at_offset + 32]
        .iter()
        .any(|&b| b != 0);

    if !submitted_at_nonzero {
        debug!("Withdrawal does not exist (submittedAt=0)");
        return Ok(false);
    }

    // Check approved (slot 12, offset 384)
    let approved = bytes.get(12 * 32 + 31).copied().unwrap_or(0) != 0;

    // Check cancelled (slot 13, offset 416)
    let cancelled = bytes.get(13 * 32 + 31).copied().unwrap_or(0) != 0;

    debug!(
        "Withdrawal status for xchainHashId=0x{}: submitted={}, approved={}, cancelled={}",
        hex::encode(&xchain_hash_id.as_slice()[..8]),
        submitted_at_byte != 0 || submitted_at_nonzero,
        approved,
        cancelled
    );

    Ok(cancelled)
}

/// Verify a transaction succeeded by checking its receipt
pub(crate) async fn verify_tx_success(config: &E2eConfig, tx_hash: B256) -> Result<bool> {
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
        return Err(eyre::eyre!("Transaction receipt not found"));
    }

    let status = body["result"]["status"].as_str().unwrap_or("0x0");

    Ok(status == "0x1")
}

/// Convert a bytes4 chain ID to bytes32 (left-aligned, right-padded with zeros)
///
/// This matches Solidity's bytes32(bytes4(chainId)) cast behavior:
/// bytes4(0x00000001) -> 0x0000000100000000...000

pub(crate) fn chain_id4_to_bytes32(chain_id: [u8; 4]) -> [u8; 32] {
    let mut result = [0u8; 32];
    result[..4].copy_from_slice(&chain_id);
    result
}

/// Compute the xchainHashId for a given set of parameters
///
/// Matches HashLib.computeXchainHashId which uses bytes4 chain IDs
/// cast to bytes32 for hashing:
/// ```solidity
/// keccak256(abi.encode(
///     bytes32(srcChain), bytes32(destChain),
///     srcAccount, destAccount, token,
///     amount, uint256(nonce)
/// ))
/// ```
///
/// For the withdraw side, the hash is computed as:
///   srcChain = source chain bytes4 (left-aligned in 32 bytes)
///   destChain = this chain's bytes4 (left-aligned in 32 bytes)
///   srcAccount = depositor account on source chain
///   destAccount = recipient on this chain
///   token = token address on this chain (left-padded to bytes32)
///   amount = transfer amount
///   nonce = deposit nonce from source chain
pub(crate) fn compute_xchain_hash_id(
    src_chain_key: B256,
    dest_chain_id: u64,
    token: Address,
    dest_account: B256,
    amount: U256,
    nonce: u64,
) -> B256 {
    // src_chain_key is already bytes32 (bytes4 left-aligned, right-padded with zeros)

    // dest chain key: bytes4 left-aligned in 32 bytes
    let mut dest_chain_key = [0u8; 32];
    dest_chain_key[..4].copy_from_slice(&(dest_chain_id as u32).to_be_bytes());

    // For withdraw-side hash, we need srcAccount. In fraud tests this is a fake
    // value. We use a deterministic srcAccount derived from nonce.
    let mut src_account = [0u8; 32];
    src_account[0..8].copy_from_slice(&nonce.to_be_bytes());
    src_account[8] = 0xff; // marker

    // Convert token address to bytes32 (left-padded with zeros, matching addressToBytes32)
    let mut token_bytes32 = [0u8; 32];
    token_bytes32[12..32].copy_from_slice(token.as_slice());

    // ABI encode all 7 fields: keccak256(abi.encode(srcChain, destChain, srcAccount, destAccount, token, amount, nonce))
    let mut data = Vec::with_capacity(7 * 32);
    data.extend_from_slice(src_chain_key.as_slice()); // bytes32(srcChain)
    data.extend_from_slice(&dest_chain_key); // bytes32(destChain)
    data.extend_from_slice(&src_account); // srcAccount
    data.extend_from_slice(dest_account.as_slice()); // destAccount
    data.extend_from_slice(&token_bytes32); // token as bytes32
    data.extend_from_slice(&amount.to_be_bytes::<32>());
    data.extend_from_slice(&U256::from(nonce).to_be_bytes::<32>());

    keccak256(&data)
}

/// Check EVM RPC connection
///
/// Sends an eth_blockNumber request to the RPC endpoint.
/// Returns the current block number on success, an error otherwise.
pub(crate) async fn check_evm_connection(rpc_url: &Url) -> Result<u64> {
    let client = reqwest::Client::new();
    let response = client
        .post(rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        eyre::bail!("EVM RPC returned status: {}", response.status());
    }

    let body: serde_json::Value = response.json().await?;

    let hex_block = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    let block = u64::from_str_radix(hex_block.trim_start_matches("0x"), 16)?;
    Ok(block)
}

/// Check Terra LCD connection
///
/// Sends a request to the Terra LCD endpoint to check node status.
/// Returns Ok(()) on success, an error otherwise.
pub(crate) async fn check_terra_connection(lcd_url: &Url) -> Result<()> {
    let client = reqwest::Client::new();
    let status_url = format!("{}/cosmos/base/tendermint/v1beta1/syncing", lcd_url);

    let response = client.get(&status_url).send().await?;

    if response.status().is_success() {
        Ok(())
    } else {
        eyre::bail!("Terra LCD returned status: {}", response.status())
    }
}
