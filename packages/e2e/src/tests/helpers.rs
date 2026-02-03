//! Test helper functions for E2E tests
//!
//! This module contains helper functions used by test cases.

use crate::E2eConfig;
use alloy::primitives::{keccak256, Address, B256, U256};
use eyre::Result;
use std::time::Duration;
use tracing::{debug, info};
use url::Url;

/// Query withdraw delay from bridge contract
pub(crate) async fn query_withdraw_delay(config: &E2eConfig) -> eyre::Result<u64> {
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

/// Query deposit nonce from bridge contract
pub(crate) async fn query_deposit_nonce(config: &E2eConfig) -> eyre::Result<u64> {
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

/// Query EVM chain key from ChainRegistry
pub(crate) async fn query_evm_chain_key(
    config: &E2eConfig,
    chain_id: u64,
) -> eyre::Result<[u8; 32]> {
    let client = reqwest::Client::new();

    // Encode getChainKeyEVM(uint256) function call
    // Verified with: cast sig "getChainKeyEVM(uint256)" = 0x5411d37f
    let chain_id_hex = format!("{:064x}", chain_id);
    let call_data = format!("0x5411d37f{}", chain_id_hex);

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

    // Parse hex to bytes32
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    let mut chain_key = [0u8; 32];
    chain_key.copy_from_slice(&bytes[..32]);
    Ok(chain_key)
}

/// Check if a chain key is registered in ChainRegistry
pub(crate) async fn is_chain_key_registered(
    config: &E2eConfig,
    chain_key: [u8; 32],
) -> eyre::Result<bool> {
    let client = reqwest::Client::new();

    // Encode isChainKeyRegistered(bytes32) function call
    // Verified with: cast sig "isChainKeyRegistered(bytes32)" = 0x3a3099d1
    let chain_key_hex = hex::encode(chain_key);
    let call_data = format!("0x3a3099d1{}", chain_key_hex);

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
    // Verified with: cast sig "hasRole(uint64,address)" = 0xd1f856ee
    // ABI encode: padded role_id (32 bytes) + padded address (32 bytes)
    let role_padded = format!("{:064x}", role_id);
    let addr_padded = format!("{:0>64}", hex::encode(account.as_slice()));
    let call_data = format!("0xd1f856ee{}{}", role_padded, addr_padded);

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

/// Get Terra chain key from ChainRegistry
pub(crate) async fn get_terra_chain_key(config: &E2eConfig) -> Result<[u8; 32]> {
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
pub(crate) fn encode_terra_address(address: &str) -> [u8; 32] {
    let mut result = [0u8; 32];
    let addr_bytes = address.as_bytes();
    let len = std::cmp::min(addr_bytes.len(), 32);
    result[..len].copy_from_slice(&addr_bytes[..len]);
    result
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
pub(crate) async fn execute_deposit(
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

/// Create a fraudulent approval on the bridge
///
/// Function signature: approveWithdraw(bytes32,address,address,bytes32,uint256,uint256,uint256,address,bool)
/// This creates an approval that has no matching deposit (fraud scenario)
pub(crate) async fn create_fraudulent_approval(
    config: &E2eConfig,
    src_chain_key: B256,
    token: Address,
    recipient: Address,
    amount: &str,
    nonce: u64,
) -> Result<B256> {
    let client = reqwest::Client::new();

    // Function selector for approveWithdraw(bytes32,address,address,bytes32,uint256,uint256,uint256,address,bool)
    // keccak256 first 4 bytes
    let selector = "c6a1d878"; // Computed selector

    // Create a fake destAccount (the recipient's address as bytes32)
    let dest_account = format!("{:0>64}", hex::encode(recipient.as_slice()));

    // Parse amount to u256
    let amount_u256: u128 = amount.parse().unwrap_or(1234567890123456789);

    // ABI encode all parameters
    let src_chain_key_hex = hex::encode(src_chain_key.as_slice());
    let token_padded = format!("{:0>64}", hex::encode(token.as_slice()));
    let to_padded = format!("{:0>64}", hex::encode(recipient.as_slice()));
    let amount_padded = format!("{:064x}", amount_u256);
    let nonce_padded = format!("{:064x}", nonce);
    let fee_padded = format!("{:064x}", 0u128); // No fee
    let fee_recipient_padded = format!("{:0>64}", "00"); // Zero address
    let deduct_from_amount = format!("{:064x}", 0u8); // false

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
        "Fraudulent approval transaction: 0x{}",
        hex::encode(tx_hash)
    );

    // Wait for confirmation
    tokio::time::sleep(Duration::from_secs(2)).await;

    Ok(tx_hash)
}

/// Check if an approval was cancelled by querying getWithdrawApproval
///
/// Function signature: getWithdrawApproval(bytes32) returns (WithdrawApproval)
/// WithdrawApproval struct has `cancelled` field at offset 160 (5th field after fee, feeRecipient, approvedAt, isApproved, deductFromAmount)
pub(crate) async fn is_approval_cancelled(config: &E2eConfig, nonce: u64) -> Result<bool> {
    let client = reqwest::Client::new();

    // Function selector for getWithdrawApproval(bytes32)
    let selector = "8f601f66"; // Computed selector

    // Create a pseudo-hash from the nonce for querying
    let pseudo_hash = format!("{:0>64}", format!("{:x}", nonce));

    let call_data = format!("0x{}{}", selector, pseudo_hash);

    debug!("Checking approval status for nonce {}", nonce);

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

    // Parse the WithdrawApproval struct response
    let bytes = hex::decode(result_hex.trim_start_matches("0x")).unwrap_or_default();

    if bytes.len() < 192 {
        debug!("Response too short, approval may not exist");
        return Ok(false);
    }

    // Check the cancelled field (6th 32-byte slot, offset 160)
    let cancelled_byte = bytes.get(160 + 31).copied().unwrap_or(0);
    let is_cancelled = cancelled_byte != 0;

    debug!(
        "Approval status for nonce {}: cancelled={}",
        nonce, is_cancelled
    );

    Ok(is_cancelled)
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

/// Compute the withdrawHash for a given set of parameters
/// This matches the Solidity: keccak256(abi.encode(srcChainKey, token, to, destAccount, amount, nonce))
#[allow(dead_code)]
pub(crate) fn compute_withdraw_hash(
    src_chain_key: B256,
    token: Address,
    to: Address,
    dest_account: B256,
    amount: U256,
    nonce: u64,
) -> B256 {
    // ABI encode the parameters
    let mut data = Vec::with_capacity(192);
    data.extend_from_slice(src_chain_key.as_slice());
    data.extend_from_slice(&[0u8; 12]); // padding for address
    data.extend_from_slice(token.as_slice());
    data.extend_from_slice(&[0u8; 12]); // padding for address
    data.extend_from_slice(to.as_slice());
    data.extend_from_slice(dest_account.as_slice());
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
