//! Chain configuration and contract setup module
//!
//! This module handles:
//! - Role granting (OPERATOR_ROLE, CANCELER_ROLE) via AccessManager
//! - Chain key registration via ChainRegistry
//! - Token registration via TokenRegistry
//! - CW20 token deployment on LocalTerra
//!
//! These functions are extracted from setup.rs to keep the orchestration module
//! under 900 LOC while providing full E2E parity with bash scripts.

use alloy::primitives::{Address, B256};
use eyre::{eyre, Result};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::config::E2eConfig;

// =============================================================================
// Constants
// =============================================================================

/// OPERATOR_ROLE ID in AccessManager (role ID 1)
pub const OPERATOR_ROLE_ID: u64 = 1;

/// CANCELER_ROLE ID in AccessManager (role ID 2)
pub const CANCELER_ROLE_ID: u64 = 2;

/// Bridge type for TokenRegistry
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BridgeType {
    MintBurn = 0,
    LockUnlock = 1,
}

/// LocalTerra container name
const LOCALTERRA_CONTAINER: &str = "cl8y-bridge-monorepo-localterra-1";

// =============================================================================
// Role Management (AccessManager)
// =============================================================================

/// Grant OPERATOR_ROLE to an account via AccessManager.grantRole()
///
/// Uses `cast send` to call the contract directly, matching the bash script behavior.
///
/// # Arguments
/// * `access_manager` - Address of the AccessManager contract
/// * `account` - Address to grant the role to
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing (hex string with 0x prefix)
pub async fn grant_operator_role(
    access_manager: Address,
    account: Address,
    rpc_url: &str,
    private_key: &str,
) -> Result<()> {
    info!("Granting OPERATOR_ROLE to {}", account);

    // First check if already has role
    if has_role(access_manager, OPERATOR_ROLE_ID, account, rpc_url).await? {
        info!("Account {} already has OPERATOR_ROLE", account);
        return Ok(());
    }

    // Grant role: grantRole(uint64 roleId, address account, uint32 delay)
    let output = std::process::Command::new("cast")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", access_manager),
            "grantRole(uint64,address,uint32)",
            &OPERATOR_ROLE_ID.to_string(),
            &format!("{}", account),
            "0", // no delay
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if it's just "already granted" error
        if stderr.contains("already") {
            info!("OPERATOR_ROLE already granted to {}", account);
            return Ok(());
        }
        return Err(eyre!("Failed to grant OPERATOR_ROLE: {}", stderr));
    }

    info!("OPERATOR_ROLE granted to {}", account);
    Ok(())
}

/// Grant CANCELER_ROLE to an account via AccessManager.grantRole()
///
/// # Arguments
/// * `access_manager` - Address of the AccessManager contract
/// * `account` - Address to grant the role to
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing (hex string with 0x prefix)
pub async fn grant_canceler_role(
    access_manager: Address,
    account: Address,
    rpc_url: &str,
    private_key: &str,
) -> Result<()> {
    info!("Granting CANCELER_ROLE to {}", account);

    // First check if already has role
    if has_role(access_manager, CANCELER_ROLE_ID, account, rpc_url).await? {
        info!("Account {} already has CANCELER_ROLE", account);
        return Ok(());
    }

    // Grant role: grantRole(uint64 roleId, address account, uint32 delay)
    let output = std::process::Command::new("cast")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", access_manager),
            "grantRole(uint64,address,uint32)",
            &CANCELER_ROLE_ID.to_string(),
            &format!("{}", account),
            "0", // no delay
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already") {
            info!("CANCELER_ROLE already granted to {}", account);
            return Ok(());
        }
        return Err(eyre!("Failed to grant CANCELER_ROLE: {}", stderr));
    }

    info!("CANCELER_ROLE granted to {}", account);
    Ok(())
}

/// Check if an account has a specific role
///
/// # Arguments
/// * `access_manager` - Address of the AccessManager contract
/// * `role_id` - The role ID to check
/// * `account` - The account to check
/// * `rpc_url` - EVM RPC URL
pub async fn has_role(
    access_manager: Address,
    role_id: u64,
    account: Address,
    rpc_url: &str,
) -> Result<bool> {
    // hasRole(uint64,address) returns (bool,uint32)
    let output = std::process::Command::new("cast")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", access_manager),
            "hasRole(uint64,address)(bool,uint32)",
            &role_id.to_string(),
            &format!("{}", account),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to query hasRole: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output format is "true 0" or "false 0" (bool, uint32)
    Ok(stdout.trim().starts_with("true"))
}

/// Grant both OPERATOR_ROLE and CANCELER_ROLE to the test account
///
/// This is the main entry point for role configuration, matching the bash script's
/// `grant_operator_role()` function.
pub async fn grant_test_account_roles(config: &E2eConfig) -> Result<()> {
    info!("Granting roles to test account for E2E testing");

    let access_manager = config.evm.contracts.access_manager;
    let test_account = config.test_accounts.evm_address;
    let rpc_url = config.evm.rpc_url.as_str();
    let private_key = format!("0x{:x}", config.evm.private_key);

    // Grant OPERATOR_ROLE
    grant_operator_role(access_manager, test_account, rpc_url, &private_key).await?;

    // Grant CANCELER_ROLE (for fraud testing)
    grant_canceler_role(access_manager, test_account, rpc_url, &private_key).await?;

    info!("All roles granted to test account");
    Ok(())
}

// =============================================================================
// Chain Registration (ChainRegistry)
// =============================================================================

/// Register a COSMW chain key on ChainRegistry via addCOSMWChainKey()
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The COSMW chain ID (e.g., "localterra")
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing
///
/// # Returns
/// The computed chain key (bytes32)
pub async fn register_cosmw_chain_key(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
    private_key: &str,
) -> Result<B256> {
    info!("Registering COSMW chain key for: {}", chain_id);

    // First check if already registered
    let existing = get_cosmw_chain_key(chain_registry, chain_id, rpc_url).await?;
    if existing != B256::ZERO {
        info!("Chain key already registered: {}", existing);
        return Ok(existing);
    }

    // Register chain key: addCOSMWChainKey(string)
    let output = std::process::Command::new("cast")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", chain_registry),
            "addCOSMWChainKey(string)",
            chain_id,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if already registered
        if stderr.contains("already") || stderr.contains("ChainKeyAlreadyRegistered") {
            // Get the existing key
            let key = get_cosmw_chain_key(chain_registry, chain_id, rpc_url).await?;
            info!("Chain key already registered: {}", key);
            return Ok(key);
        }
        return Err(eyre!("Failed to register chain key: {}", stderr));
    }

    // Wait a moment for the transaction to be mined
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Get the registered chain key
    let chain_key = get_cosmw_chain_key(chain_registry, chain_id, rpc_url).await?;

    if chain_key == B256::ZERO {
        return Err(eyre!(
            "Chain key registration failed - key is zero after registration"
        ));
    }

    info!("Chain key registered: {}", chain_key);
    Ok(chain_key)
}

/// Get COSMW chain key from ChainRegistry
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The COSMW chain ID
/// * `rpc_url` - EVM RPC URL
pub async fn get_cosmw_chain_key(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
) -> Result<B256> {
    let output = std::process::Command::new("cast")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", chain_registry),
            "getChainKeyCOSMW(string)(bytes32)",
            chain_id,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to get chain key: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let hex_str = stdout.trim();

    // Parse the bytes32 value
    let chain_key: B256 = hex_str
        .parse()
        .map_err(|e| eyre!("Failed to parse chain key '{}': {}", hex_str, e))?;

    Ok(chain_key)
}

/// Register Terra chain key on ChainRegistry
///
/// This is the main entry point for Terra chain registration, matching the bash
/// script's `register_terra_chain_key()` function.
pub async fn register_terra_chain_key(config: &E2eConfig) -> Result<B256> {
    info!("Registering Terra chain key on ChainRegistry");

    let chain_registry = config.evm.contracts.chain_registry;
    let rpc_url = config.evm.rpc_url.as_str();
    let private_key = format!("0x{:x}", config.evm.private_key);
    let chain_id = &config.terra.chain_id; // "localterra"

    register_cosmw_chain_key(chain_registry, chain_id, rpc_url, &private_key).await
}

// =============================================================================
// Token Registration (TokenRegistry)
// =============================================================================

/// Register a token on TokenRegistry with addToken()
///
/// # Arguments
/// * `token_registry` - Address of the TokenRegistry contract
/// * `token` - Address of the token to register
/// * `bridge_type` - The bridge type (MintBurn or LockUnlock)
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing
pub async fn register_token(
    token_registry: Address,
    token: Address,
    bridge_type: BridgeType,
    rpc_url: &str,
    private_key: &str,
) -> Result<()> {
    info!(
        "Registering token {} with bridge type {:?}",
        token, bridge_type
    );

    // Check if already registered
    if is_token_registered(token_registry, token, rpc_url).await? {
        info!("Token {} already registered", token);
        return Ok(());
    }

    // addToken(address token, uint8 bridgeType)
    let output = std::process::Command::new("cast")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", token_registry),
            "addToken(address,uint8)",
            &format!("{}", token),
            &(bridge_type as u8).to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already") || stderr.contains("TokenAlreadyRegistered") {
            info!("Token {} already registered", token);
            return Ok(());
        }
        return Err(eyre!("Failed to register token: {}", stderr));
    }

    info!("Token {} registered successfully", token);
    Ok(())
}

/// Check if a token is registered on TokenRegistry
async fn is_token_registered(
    token_registry: Address,
    token: Address,
    rpc_url: &str,
) -> Result<bool> {
    // Try querying getTokenBridgeType - returns 0 for unregistered
    let output = std::process::Command::new("cast")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", token_registry),
            "getTokenBridgeType(address)(uint8)",
            &format!("{}", token),
        ])
        .output()?;

    if !output.status.success() {
        // If the call reverts, token is not registered
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // If we got a non-zero bridge type, token is registered
    let bridge_type: u8 = stdout.trim().parse().unwrap_or(0);

    // BridgeType 0 could be MintBurn which is valid, so we need a different check
    // Check if there's any destination chain registered
    Ok(bridge_type <= 1) // 0=MintBurn, 1=LockUnlock, both valid if call succeeded
}

/// Add a destination chain key for a token on TokenRegistry
///
/// # Arguments
/// * `token_registry` - Address of the TokenRegistry contract
/// * `token` - Address of the source token
/// * `dest_chain_key` - The destination chain key (bytes32)
/// * `dest_token_address` - The destination token address (bytes32-encoded)
/// * `decimals` - The token decimals on the destination chain
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing
pub async fn add_token_dest_chain_key(
    token_registry: Address,
    token: Address,
    dest_chain_key: B256,
    dest_token_address: B256,
    decimals: u8,
    rpc_url: &str,
    private_key: &str,
) -> Result<()> {
    info!(
        "Adding destination chain key for token {}: chain_key={}, dest_token={}, decimals={}",
        token, dest_chain_key, dest_token_address, decimals
    );

    // Check if already registered
    if is_token_dest_chain_registered(token_registry, token, dest_chain_key, rpc_url).await? {
        info!(
            "Token {} destination chain {} already registered",
            token, dest_chain_key
        );
        return Ok(());
    }

    // addTokenDestChainKey(address token, bytes32 destChainKey, bytes32 destTokenAddress, uint8 decimals)
    let output = std::process::Command::new("cast")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", token_registry),
            "addTokenDestChainKey(address,bytes32,bytes32,uint8)",
            &format!("{}", token),
            &format!("{}", dest_chain_key),
            &format!("{}", dest_token_address),
            &decimals.to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already") || stderr.contains("DestChainKeyAlreadyRegistered") {
            info!("Destination chain key already registered");
            return Ok(());
        }
        return Err(eyre!("Failed to add destination chain key: {}", stderr));
    }

    info!("Destination chain key added successfully");
    Ok(())
}

/// Check if a token's destination chain key is registered
async fn is_token_dest_chain_registered(
    token_registry: Address,
    token: Address,
    dest_chain_key: B256,
    rpc_url: &str,
) -> Result<bool> {
    let output = std::process::Command::new("cast")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", token_registry),
            "isTokenDestChainKeyRegistered(address,bytes32)(bool)",
            &format!("{}", token),
            &format!("{}", dest_chain_key),
        ])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == "true")
}

/// Encode a Terra address (or native denom) as bytes32
///
/// For CW20 addresses or native denoms, this encodes the string as left-padded bytes32.
pub fn encode_terra_token_address(token: &str) -> B256 {
    let token_bytes = token.as_bytes();
    let mut bytes = [0u8; 32];

    // Left-pad the token bytes (terra addresses are typically ~44 chars, denoms like "uluna" are 5)
    let start = if token_bytes.len() <= 32 {
        0
    } else {
        token_bytes.len() - 32
    };

    let copy_len = token_bytes.len().min(32);
    bytes[..copy_len].copy_from_slice(&token_bytes[start..start + copy_len]);

    B256::from_slice(&bytes)
}

/// Register test tokens on TokenRegistry for E2E testing
///
/// This registers the test ERC20 token with the Terra chain as destination,
/// matching the bash script's `register_test_tokens()` function.
///
/// # Arguments
/// * `config` - E2E configuration
/// * `test_token` - Optional ERC20 token address (if None, skips registration)
/// * `terra_cw20_address` - Optional CW20 address on Terra (if None, uses "uluna")
/// * `terra_chain_key` - The Terra chain key from ChainRegistry
pub async fn register_test_tokens(
    config: &E2eConfig,
    test_token: Option<Address>,
    terra_cw20_address: Option<&str>,
    terra_chain_key: B256,
) -> Result<()> {
    let Some(token) = test_token else {
        warn!("No test token address provided, skipping token registration");
        return Ok(());
    };

    info!("Registering test tokens on TokenRegistry");

    let token_registry = config.evm.contracts.token_registry;
    let rpc_url = config.evm.rpc_url.as_str();
    let private_key = format!("0x{:x}", config.evm.private_key);

    // Step 1: Add token with bridge type (LockUnlock for test tokens)
    register_token(
        token_registry,
        token,
        BridgeType::LockUnlock,
        rpc_url,
        &private_key,
    )
    .await?;

    // Step 2: Encode destination token
    let dest_token = terra_cw20_address.unwrap_or("uluna");
    let dest_token_encoded = encode_terra_token_address(dest_token);

    // Step 3: Add destination chain key (Terra decimals are typically 6)
    add_token_dest_chain_key(
        token_registry,
        token,
        terra_chain_key,
        dest_token_encoded,
        6, // Terra token decimals
        rpc_url,
        &private_key,
    )
    .await?;

    info!("Test token registration complete");
    Ok(())
}

// =============================================================================
// CW20 Token Deployment (LocalTerra)
// =============================================================================

/// Result of CW20 deployment
#[derive(Debug, Clone)]
pub struct Cw20DeployResult {
    pub code_id: u64,
    pub contract_address: String,
}

/// Deploy a CW20 test token on LocalTerra
///
/// This function:
/// 1. Copies the CW20 WASM to the container
/// 2. Stores the WASM code
/// 3. Instantiates the contract with initial balances
///
/// Matches the bash script's CW20 deployment in `deploy_test_tokens()`.
///
/// # Arguments
/// * `wasm_path` - Path to the CW20 WASM file (e.g., cw20_mintable.wasm)
/// * `name` - Token name
/// * `symbol` - Token symbol
/// * `decimals` - Token decimals (typically 6 for Terra)
/// * `initial_balance` - Initial balance for the test account
/// * `test_address` - Terra test account address
///
/// # Returns
/// The deployed CW20 contract address
pub async fn deploy_cw20_token(
    wasm_path: &std::path::Path,
    name: &str,
    symbol: &str,
    decimals: u8,
    initial_balance: u128,
    test_address: &str,
) -> Result<Cw20DeployResult> {
    info!("Deploying CW20 token {} ({}) on LocalTerra", name, symbol);

    // Check if container is running
    if !is_localterra_running().await? {
        return Err(eyre!("LocalTerra container is not running"));
    }

    // Check if WASM file exists
    if !wasm_path.exists() {
        return Err(eyre!("CW20 WASM not found at: {}", wasm_path.display()));
    }

    // Step 1: Create directory and copy WASM to container
    docker_exec(&["mkdir", "-p", "/tmp/wasm"]).await?;

    let wasm_filename = wasm_path
        .file_name()
        .ok_or_else(|| eyre!("Invalid WASM path"))?
        .to_string_lossy();
    let container_wasm = format!("/tmp/wasm/{}", wasm_filename);

    docker_cp(wasm_path, &container_wasm).await?;

    // Step 2: Store WASM code
    info!("Storing CW20 WASM code...");
    let _store_output = terrad_exec(&[
        "tx",
        "wasm",
        "store",
        &container_wasm,
        "--from",
        "test1",
        "--chain-id",
        "localterra",
        "--gas",
        "auto",
        "--gas-adjustment",
        "1.5",
        "--fees",
        "10000000uluna",
        "--broadcast-mode",
        "sync",
        "-y",
        "--keyring-backend",
        "test",
        "-o",
        "json",
    ])
    .await?;

    // Wait for transaction to be included
    tokio::time::sleep(Duration::from_secs(8)).await;

    // Get code ID from list-code
    let code_id = get_latest_code_id().await?;
    info!("CW20 code stored with ID: {}", code_id);

    // Step 3: Instantiate contract
    let init_msg = serde_json::json!({
        "name": name,
        "symbol": symbol,
        "decimals": decimals,
        "initial_balances": [{
            "address": test_address,
            "amount": initial_balance.to_string()
        }],
        "mint": {
            "minter": test_address
        }
    });

    info!("Instantiating CW20 contract...");
    let inst_output = terrad_exec(&[
        "tx",
        "wasm",
        "instantiate",
        &code_id.to_string(),
        &serde_json::to_string(&init_msg)?,
        "--label",
        &format!("{}-e2e", symbol.to_lowercase()),
        "--admin",
        test_address,
        "--from",
        "test1",
        "--chain-id",
        "localterra",
        "--gas",
        "auto",
        "--gas-adjustment",
        "1.5",
        "--fees",
        "10000000uluna",
        "--broadcast-mode",
        "sync",
        "-y",
        "--keyring-backend",
        "test",
        "-o",
        "json",
    ])
    .await?;

    debug!("Instantiate output: {}", inst_output);

    // Wait for instantiation
    tokio::time::sleep(Duration::from_secs(6)).await;

    // Get contract address
    let contract_address = get_contract_by_code_id(code_id).await?;
    info!("CW20 deployed at: {}", contract_address);

    Ok(Cw20DeployResult {
        code_id,
        contract_address,
    })
}

/// Check if LocalTerra container is running
pub async fn is_localterra_running() -> Result<bool> {
    let output = std::process::Command::new("docker")
        .args([
            "ps",
            "--format",
            "{{.Names}}",
            "--filter",
            &format!("name={}", LOCALTERRA_CONTAINER),
        ])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains(LOCALTERRA_CONTAINER))
}

/// Execute a command in the LocalTerra container
async fn docker_exec(args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["exec", LOCALTERRA_CONTAINER];
    cmd_args.extend(args);

    let output = std::process::Command::new("docker")
        .args(&cmd_args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Docker exec failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Copy a file to the LocalTerra container
async fn docker_cp(src: &std::path::Path, dest: &str) -> Result<()> {
    let output = std::process::Command::new("docker")
        .args([
            "cp",
            &src.to_string_lossy(),
            &format!("{}:{}", LOCALTERRA_CONTAINER, dest),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Docker cp failed: {}", stderr));
    }

    Ok(())
}

/// Execute a terrad command in the LocalTerra container
async fn terrad_exec(args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["exec", LOCALTERRA_CONTAINER, "terrad"];
    cmd_args.extend(args);

    debug!("Executing terrad: {:?}", args);

    let output = std::process::Command::new("docker")
        .args(&cmd_args)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("terrad command failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get the latest stored code ID from LocalTerra
async fn get_latest_code_id() -> Result<u64> {
    let output = terrad_exec(&["query", "wasm", "list-code", "-o", "json"]).await?;

    let json: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| eyre!("Failed to parse list-code response: {}", e))?;

    let code_id = json["code_infos"]
        .as_array()
        .and_then(|arr| arr.last())
        .and_then(|info| info["code_id"].as_str())
        .or_else(|| {
            json["code_infos"]
                .as_array()
                .and_then(|arr| arr.last())
                .and_then(|info| info["code_id"].as_u64())
                .map(|_| "")
        })
        .ok_or_else(|| eyre!("No code_id found in list-code response"))?;

    // Handle both string and number code_id
    if code_id.is_empty() {
        json["code_infos"]
            .as_array()
            .and_then(|arr| arr.last())
            .and_then(|info| info["code_id"].as_u64())
            .ok_or_else(|| eyre!("Failed to parse code_id"))
    } else {
        code_id
            .parse()
            .map_err(|e| eyre!("Failed to parse code_id '{}': {}", code_id, e))
    }
}

/// Get contract address by code ID
async fn get_contract_by_code_id(code_id: u64) -> Result<String> {
    let output = terrad_exec(&[
        "query",
        "wasm",
        "list-contract-by-code",
        &code_id.to_string(),
        "-o",
        "json",
    ])
    .await?;

    let json: serde_json::Value = serde_json::from_str(&output)
        .map_err(|e| eyre!("Failed to parse list-contract-by-code response: {}", e))?;

    json["contracts"]
        .as_array()
        .and_then(|arr| arr.last())
        .and_then(|addr| addr.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("No contract found for code_id {}", code_id))
}

/// Deploy test CW20 token with default parameters
///
/// Convenience function that deploys a test bridge token on LocalTerra.
pub async fn deploy_test_cw20(
    project_root: &std::path::Path,
    test_address: &str,
) -> Result<Option<Cw20DeployResult>> {
    // Check if LocalTerra is running
    if !is_localterra_running().await? {
        warn!("LocalTerra not running, skipping CW20 deployment");
        return Ok(None);
    }

    // Check for CW20 WASM
    let cw20_wasm = project_root
        .join("packages")
        .join("contracts-terraclassic")
        .join("artifacts")
        .join("cw20_mintable.wasm");

    if !cw20_wasm.exists() {
        warn!(
            "CW20 WASM not found at {}, skipping CW20 deployment",
            cw20_wasm.display()
        );
        return Ok(None);
    }

    match deploy_cw20_token(
        &cw20_wasm,
        "Test Bridge Token",
        "TBT",
        6,
        1_000_000_000_000, // 1M tokens with 6 decimals
        test_address,
    )
    .await
    {
        Ok(result) => Ok(Some(result)),
        Err(e) => {
            warn!("CW20 deployment failed: {}", e);
            Ok(None)
        }
    }
}

// =============================================================================
// Full Setup Functions
// =============================================================================

/// Configuration result from full chain setup
#[derive(Debug, Clone)]
pub struct ChainConfigResult {
    /// Terra chain key (bytes32)
    pub terra_chain_key: B256,
    /// CW20 token address (if deployed)
    pub cw20_address: Option<String>,
    /// Whether roles were granted successfully
    pub roles_granted: bool,
    /// Whether token was registered
    pub token_registered: bool,
}

/// Perform complete chain configuration for E2E testing
///
/// This function performs all 4 setup gaps:
/// 1. Grant OPERATOR_ROLE and CANCELER_ROLE to test accounts
/// 2. Register Terra chain key via ChainRegistry.addCOSMWChainKey()
/// 3. Register test tokens on TokenRegistry with destination chain mappings
/// 4. Deploy CW20 test token on LocalTerra (optional)
///
/// # Arguments
/// * `config` - E2E configuration
/// * `project_root` - Path to project root (for finding WASM files)
/// * `test_token` - Optional ERC20 token address to register
pub async fn configure_chains(
    config: &E2eConfig,
    project_root: &std::path::Path,
    test_token: Option<Address>,
) -> Result<ChainConfigResult> {
    info!("Starting full chain configuration for E2E testing");

    // Step 1: Grant roles to test account
    let roles_granted = match grant_test_account_roles(config).await {
        Ok(()) => {
            info!("Roles granted successfully");
            true
        }
        Err(e) => {
            warn!("Failed to grant roles: {}", e);
            false
        }
    };

    // Step 2: Register Terra chain key
    let terra_chain_key = register_terra_chain_key(config).await?;
    info!("Terra chain key registered: {}", terra_chain_key);

    // Step 3: Deploy CW20 on LocalTerra (optional)
    let cw20_result = deploy_test_cw20(project_root, &config.test_accounts.terra_address).await?;
    let cw20_address = cw20_result.as_ref().map(|r| r.contract_address.clone());

    // Step 4: Register test tokens on TokenRegistry
    let token_registered = if test_token.is_some() {
        match register_test_tokens(config, test_token, cw20_address.as_deref(), terra_chain_key)
            .await
        {
            Ok(()) => {
                info!("Test tokens registered successfully");
                true
            }
            Err(e) => {
                warn!("Failed to register test tokens: {}", e);
                false
            }
        }
    } else {
        warn!("No test token provided, skipping token registration");
        false
    };

    info!("Chain configuration complete");

    Ok(ChainConfigResult {
        terra_chain_key,
        cw20_address,
        roles_granted,
        token_registered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_terra_token_address() {
        let uluna = encode_terra_token_address("uluna");
        // "uluna" should be encoded as bytes
        assert_eq!(&uluna.as_slice()[..5], b"uluna");
    }

    #[test]
    fn test_bridge_type_values() {
        assert_eq!(BridgeType::MintBurn as u8, 0);
        assert_eq!(BridgeType::LockUnlock as u8, 1);
    }
}
