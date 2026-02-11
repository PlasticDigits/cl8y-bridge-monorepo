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
    let mut last_progress_log = Instant::now();

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
                // Log progress every 15 seconds at INFO level for visibility
                if last_progress_log.elapsed() >= Duration::from_secs(15) {
                    info!(
                        nonce = deposit_nonce,
                        attempt = attempt,
                        elapsed_secs = start.elapsed().as_secs(),
                        remaining_secs = timeout.saturating_sub(start.elapsed()).as_secs(),
                        bridge = %config.evm.contracts.bridge,
                        "Still waiting for EVM WithdrawApprove event (no matching nonce found)"
                    );
                    last_progress_log = Instant::now();
                }
            }
            Err(e) => {
                warn!(
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

    // On timeout, run comprehensive diagnostics to explain WHY no approval was found
    warn!(
        nonce = deposit_nonce,
        attempts = attempt,
        elapsed_secs = start.elapsed().as_secs(),
        "Approval poll timed out — running diagnostics"
    );
    dump_approval_diagnostics(config, deposit_nonce).await;

    Err(eyre!(
        "Timeout waiting for approval of nonce {} after {:?} ({} attempts). \
         Run with RUST_LOG=debug for per-attempt details.",
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

/// Query approval by deposit nonce (V2)
///
/// V2 WithdrawApprove(bytes32 indexed withdrawHash) only emits the hash as a topic.
/// To find an approval matching a given nonce, we:
/// 1. Query all WithdrawApprove events
/// 2. For each event, call getPendingWithdraw(hash) to get the nonce and details
/// 3. Return the first match
async fn query_approval_by_nonce(config: &E2eConfig, nonce: u64) -> Result<Option<ApprovalInfo>> {
    let client = reqwest::Client::new();

    // V2 WithdrawApprove event: only has indexed withdrawHash
    let approval_topic =
        "0x".to_string() + &hex::encode(alloy::primitives::keccak256(b"WithdrawApprove(bytes32)"));

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
        let mut found_nonces: Vec<u64> = Vec::new();

        for log in logs {
            let empty_vec = vec![];
            let topics = log["topics"].as_array().unwrap_or(&empty_vec);
            if topics.len() < 2 {
                continue;
            }

            // topic[1] = withdrawHash
            let hash_hex = topics[1].as_str().unwrap_or("").trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_hex).unwrap_or_default();
            if hash_bytes.len() != 32 {
                continue;
            }

            let withdraw_hash = B256::from_slice(&hash_bytes);

            // Query getPendingWithdraw to get the details for this hash
            let selector = selector("getPendingWithdraw(bytes32)");
            let call_data = format!("0x{}{}", selector, hex::encode(withdraw_hash.as_slice()));

            let pw_response = client
                .post(config.evm.rpc_url.as_str())
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_call",
                    "params": [{
                        "to": format!("{}", config.evm.contracts.bridge),
                        "data": call_data
                    }, "latest"],
                    "id": 2
                }))
                .send()
                .await?;

            let pw_body: serde_json::Value = pw_response.json().await?;
            let hex_result = pw_body["result"].as_str().unwrap_or("");
            let result_bytes = hex::decode(hex_result.trim_start_matches("0x")).unwrap_or_default();

            // PendingWithdraw struct ABI layout (15 fields):
            //   [0..32]    srcChain      (bytes4, left-aligned)
            //   [32..64]   srcAccount    (bytes32)
            //   [64..96]   destAccount   (bytes32)
            //   [96..128]  token         (address, right-aligned)
            //   [128..160] recipient     (address, right-aligned)
            //   [160..192] amount        (uint256)
            //   [192..224] nonce         (uint64, right-aligned)
            //   [224..256] srcDecimals   (uint8, right-aligned)
            //   [256..288] destDecimals  (uint8, right-aligned)
            //   [288..320] operatorGas   (uint256)
            //   [320..352] submittedAt   (uint256)
            //   [352..384] approvedAt    (uint256)
            //   [384..416] approved      (bool)
            //   [416..448] cancelled     (bool)
            //   [448..480] executed      (bool)
            if result_bytes.len() >= 480 {
                let pw_nonce =
                    u64::from_be_bytes(result_bytes[216..224].try_into().unwrap_or([0u8; 8]));
                found_nonces.push(pw_nonce);

                // Debug: log raw field values for ABI verification
                if found_nonces.len() <= 3 {
                    let src_chain_raw = &result_bytes[0..4];
                    let submitted_at = U256::from_be_slice(&result_bytes[320..352]);
                    debug!(
                        hash = %format!("0x{}", hex::encode(withdraw_hash.as_slice())),
                        nonce = pw_nonce,
                        src_chain = %format!("0x{}", hex::encode(src_chain_raw)),
                        submitted_at = %submitted_at,
                        data_len = result_bytes.len(),
                        "getPendingWithdraw raw decode"
                    );
                }

                if pw_nonce == nonce {
                    // Extract src_chain_key (first 4 bytes, padded to 32)
                    let src_chain_key = B256::from_slice(&result_bytes[0..32]);
                    let token = Address::from_slice(&result_bytes[108..128]);
                    let recipient = Address::from_slice(&result_bytes[140..160]);
                    let amount = U256::from_be_slice(&result_bytes[160..192]);
                    let approved_at =
                        u64::from_be_bytes(result_bytes[376..384].try_into().unwrap_or([0u8; 8]));
                    let cancelled = result_bytes[447] != 0;
                    let executed = result_bytes[479] != 0;

                    return Ok(Some(ApprovalInfo {
                        withdraw_hash,
                        src_chain_key,
                        token,
                        recipient,
                        amount,
                        nonce,
                        approved_at,
                        cancelled,
                        executed,
                    }));
                }
            }
        }

        // Log diagnostics when target nonce not found
        if !found_nonces.contains(&nonce) {
            debug!(
                target_nonce = nonce,
                total_approval_events = logs.len(),
                found_nonces = ?found_nonces,
                bridge = %config.evm.contracts.bridge,
                "WithdrawApprove events on EVM bridge (target nonce not found)"
            );
        }
    }

    Ok(None)
}

// ============================================================================
// Approval Diagnostics — run on timeout to explain what went wrong
// ============================================================================

/// Comprehensive diagnostic dump when approval polling times out.
///
/// Queries three sources to pinpoint the failure:
/// 1. **EVM bridge contract** — All `WithdrawApprove` events (V2) (what nonces ARE approved)
/// 2. **EVM bridge contract** — `depositNonce()` (current nonce counter)
/// 3. **Operator API** — `/status` and `/pending` endpoints (operator's internal state)
///
/// This makes timeout failures immediately diagnosable instead of opaque.
async fn dump_approval_diagnostics(config: &E2eConfig, target_nonce: u64) {
    warn!(
        "=== APPROVAL TIMEOUT DIAGNOSTICS (nonce={}) ===",
        target_nonce
    );

    // 1. Query ALL WithdrawApprove events on the EVM bridge (V2)
    match query_all_approval_events(config).await {
        Ok(events) => {
            if events.is_empty() {
                warn!(
                    "[DIAG] EVM bridge has ZERO WithdrawApprove events. \
                     The operator has not submitted any withdrawApprove() transactions. \
                     Possible causes: operator not running, operator not watching this chain, \
                     deposit not stored in DB, or V2 flow requires user to call withdrawSubmit first."
                );
            } else {
                let nonces: Vec<u64> = events.iter().map(|e| e.nonce).collect();
                warn!(
                    "[DIAG] EVM bridge has {} WithdrawApprove event(s) with nonces: {:?}. \
                     Target nonce {} is NOT among them.",
                    events.len(),
                    nonces,
                    target_nonce
                );
            }
        }
        Err(e) => {
            warn!("[DIAG] Failed to query WithdrawApprove events: {}", e);
        }
    }

    // 2. Query current deposit nonce from bridge
    match query_deposit_nonce_raw(config).await {
        Ok(nonce) => {
            if target_nonce >= nonce {
                warn!(
                    "[DIAG] Bridge depositNonce={}, target_nonce={}. \
                     Target nonce has NOT been used yet (deposit may not have been made).",
                    nonce, target_nonce
                );
            } else {
                warn!(
                    "[DIAG] Bridge depositNonce={}, target_nonce={}. \
                     Target nonce IS within range (deposit was made, but approval was not created).",
                    nonce, target_nonce
                );
            }
        }
        Err(e) => {
            warn!("[DIAG] Failed to query depositNonce: {}", e);
        }
    }

    // 3. Query operator /status endpoint
    match query_operator_status(config).await {
        Ok(status) => {
            warn!(
                "[DIAG] Operator /status: {}",
                serde_json::to_string_pretty(&status).unwrap_or_else(|_| status.to_string())
            );
        }
        Err(e) => {
            warn!("[DIAG] Failed to query operator /status (operator may not be running at port 9092): {}", e);
        }
    }

    // 4. Query operator /pending endpoint
    match query_operator_pending(config).await {
        Ok(pending) => {
            warn!(
                "[DIAG] Operator /pending: {}",
                serde_json::to_string_pretty(&pending).unwrap_or_else(|_| pending.to_string())
            );
        }
        Err(e) => {
            warn!("[DIAG] Failed to query operator /pending: {}", e);
        }
    }

    // 5. Check if this is an EVM→Terra deposit (which wouldn't have EVM approvals)
    warn!(
        "[DIAG] REMINDER: In V2, EVM→Terra deposits are approved on TERRA, not EVM. \
         If the deposit targets Terra (dest_chain_key=0x00000002), poll_for_approval() on EVM \
         will never find it. Use poll_terra_for_approval() instead."
    );

    warn!("=== END DIAGNOSTICS ===");
}

/// Query ALL WithdrawApprove events from the EVM bridge (V2, for diagnostics)
///
/// V2 WithdrawApprove(bytes32 indexed withdrawHash) only has the hash.
/// We query getPendingWithdraw for each to get details.
async fn query_all_approval_events(config: &E2eConfig) -> Result<Vec<ApprovalInfo>> {
    let client = reqwest::Client::new();

    // V2 event
    let approval_topic =
        "0x".to_string() + &hex::encode(alloy::primitives::keccak256(b"WithdrawApprove(bytes32)"));

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
    let mut results = Vec::new();

    if let Some(logs) = body["result"].as_array() {
        for log in logs {
            let empty_vec = vec![];
            let topics = log["topics"].as_array().unwrap_or(&empty_vec);
            if topics.len() < 2 {
                continue;
            }

            // topic[1] = withdrawHash
            let hash_hex = topics[1].as_str().unwrap_or("").trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_hex).unwrap_or_default();
            if hash_bytes.len() != 32 {
                continue;
            }
            let withdraw_hash = B256::from_slice(&hash_bytes);

            // Query getPendingWithdraw for full details
            let sel = selector("getPendingWithdraw(bytes32)");
            let call_data = format!("0x{}{}", sel, hex::encode(withdraw_hash.as_slice()));

            let pw_resp = client
                .post(config.evm.rpc_url.as_str())
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "eth_call",
                    "params": [{
                        "to": format!("{}", config.evm.contracts.bridge),
                        "data": call_data
                    }, "latest"],
                    "id": 2
                }))
                .send()
                .await?;

            let pw_body: serde_json::Value = pw_resp.json().await?;
            let hex_result = pw_body["result"].as_str().unwrap_or("");
            let result_bytes = hex::decode(hex_result.trim_start_matches("0x")).unwrap_or_default();

            if result_bytes.len() >= 480 {
                let nonce =
                    u64::from_be_bytes(result_bytes[216..224].try_into().unwrap_or([0u8; 8]));
                let token = Address::from_slice(&result_bytes[108..128]);
                let recipient = Address::from_slice(&result_bytes[140..160]);
                let amount = U256::from_be_slice(&result_bytes[160..192]);
                let approved_at =
                    u64::from_be_bytes(result_bytes[376..384].try_into().unwrap_or([0u8; 8]));
                let cancelled = result_bytes[447] != 0;
                let executed = result_bytes[479] != 0;

                results.push(ApprovalInfo {
                    withdraw_hash,
                    src_chain_key: B256::from_slice(&result_bytes[0..32]),
                    token,
                    recipient,
                    amount,
                    nonce,
                    approved_at,
                    cancelled,
                    executed,
                });
            }
        }
    }

    Ok(results)
}

/// Query current depositNonce from the EVM bridge contract
async fn query_deposit_nonce_raw(config: &E2eConfig) -> Result<u64> {
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

    let body: serde_json::Value = response.json().await?;
    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre!("No result in depositNonce response"))?;

    let nonce = u64::from_str_radix(hex_result.trim_start_matches("0x"), 16)?;
    Ok(nonce)
}

/// Query operator /status endpoint
async fn query_operator_status(config: &E2eConfig) -> Result<serde_json::Value> {
    // Operator API runs on port 9092 by default (avoids LocalTerra gRPC on 9090, gRPC-web on 9091)
    let operator_url = derive_operator_url(config, "/status");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client.get(&operator_url).send().await?;
    let body: serde_json::Value = response.json().await?;
    Ok(body)
}

/// Query operator /pending endpoint
async fn query_operator_pending(config: &E2eConfig) -> Result<serde_json::Value> {
    let operator_url = derive_operator_url(config, "/pending");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client.get(&operator_url).send().await?;
    let body: serde_json::Value = response.json().await?;
    Ok(body)
}

/// Derive the operator API URL from the EVM RPC URL
///
/// The operator listens on port 9092 by default (avoiding LocalTerra gRPC 9090, gRPC-web 9091).
fn derive_operator_url(config: &E2eConfig, path: &str) -> String {
    // Try to extract host from EVM RPC URL and use port 9092
    if let Ok(url) = url::Url::parse(config.evm.rpc_url.as_str()) {
        let host = url.host_str().unwrap_or("localhost");
        format!("http://{}:9092{}", host, path)
    } else {
        format!("http://localhost:9092{}", path)
    }
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
/// Checks for the V2 WithdrawExecute event:
///   WithdrawExecute(bytes32 indexed withdrawHash, address recipient, uint256 amount)
///
/// NOTE: V1 used `Withdraw(bytes32,address,address,uint256)` which no longer exists.
pub async fn verify_withdrawal_executed(config: &E2eConfig, withdraw_hash: B256) -> Result<bool> {
    let client = reqwest::Client::new();

    // V2 WithdrawExecute event topic
    let withdraw_topic = "0x".to_string()
        + &hex::encode(alloy::primitives::keccak256(
            b"WithdrawExecute(bytes32,address,uint256)",
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
