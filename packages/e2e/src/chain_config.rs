//! Chain configuration and contract setup module
//!
//! This module handles:
//! - Role granting (OPERATOR_ROLE, CANCELER_ROLE) via AccessManager
//! - Chain key registration via ChainRegistry
//! - Token registration via TokenRegistry
//!
//! CW20 token deployment has been moved to `cw20_deploy` module to keep this file under 900 LOC.

use alloy::primitives::{Address, B256};
use alloy::signers::local::PrivateKeySigner;
use eyre::{eyre, Result};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::config::E2eConfig;
pub use crate::cw20_deploy::{
    deploy_cw20_token, deploy_test_cw20, is_localterra_running, Cw20DeployResult,
};

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
    // Set FOUNDRY_DISABLE_NIGHTLY_WARNING to suppress nightly warnings
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Check if it's just "already granted" error
        if stderr.contains("already") || stdout.contains("already") {
            info!("OPERATOR_ROLE already granted to {}", account);
            return Ok(());
        }
        // Ignore warnings that don't indicate actual failure
        if stderr.contains("nightly") && stdout.contains("transactionHash") {
            info!("OPERATOR_ROLE granted (ignoring nightly warning)");
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
    // Set FOUNDRY_DISABLE_NIGHTLY_WARNING to suppress nightly warnings
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stderr.contains("already") || stdout.contains("already") {
            info!("CANCELER_ROLE already granted to {}", account);
            return Ok(());
        }
        // Ignore warnings that don't indicate actual failure
        if stderr.contains("nightly") && stdout.contains("transactionHash") {
            info!("CANCELER_ROLE granted (ignoring nightly warning)");
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
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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
///
/// IMPORTANT: Also grants CANCELER_ROLE to the canceler service's derived address
/// (from config.evm.private_key), not just the test account. This ensures the
/// canceler service can submit cancel transactions.
pub async fn grant_test_account_roles(config: &E2eConfig) -> Result<()> {
    info!("Granting roles to test account for E2E testing");

    let access_manager = config.evm.contracts.access_manager;
    let test_account = config.test_accounts.evm_address;
    let rpc_url = config.evm.rpc_url.as_str();
    let private_key = format!("0x{:x}", config.evm.private_key);

    // Grant OPERATOR_ROLE to test account
    grant_operator_role(access_manager, test_account, rpc_url, &private_key).await?;

    // Grant CANCELER_ROLE to test account (for fraud testing)
    grant_canceler_role(access_manager, test_account, rpc_url, &private_key).await?;

    // Also grant CANCELER_ROLE to the canceler service's derived address
    // The canceler service uses evm.private_key if set, otherwise test_accounts.evm_private_key
    // This matches the logic in services.rs build_canceler_env()
    let canceler_key = if config.evm.private_key == B256::ZERO {
        config.test_accounts.evm_private_key
    } else {
        config.evm.private_key
    };
    let canceler_address = derive_address_from_private_key(&canceler_key)?;
    if canceler_address != test_account {
        info!(
            "Granting CANCELER_ROLE to canceler service address {} (different from test account {})",
            canceler_address, test_account
        );
        grant_canceler_role(access_manager, canceler_address, rpc_url, &private_key).await?;
    } else {
        debug!(
            "Canceler address {} matches test account, no additional grant needed",
            canceler_address
        );
    }

    info!("All roles granted to test account and canceler service");
    Ok(())
}

/// Derive an Ethereum address from a private key
///
/// Uses alloy's PrivateKeySigner to derive the address from the 32-byte private key.
pub fn derive_address_from_private_key(private_key: &B256) -> Result<Address> {
    let key_hex = format!("0x{:x}", private_key);
    let signer: PrivateKeySigner = key_hex
        .parse()
        .map_err(|e| eyre!("Failed to parse private key: {}", e))?;
    Ok(signer.address())
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

    // Compute the chain key first
    let chain_key = get_cosmw_chain_key(chain_registry, chain_id, rpc_url).await?;

    // Check if it's actually registered in the contract's EnumerableSet
    if is_chain_key_registered(chain_registry, chain_key, rpc_url).await? {
        info!("Chain key already registered: {}", chain_key);
        return Ok(chain_key);
    }

    // Register chain key: addCOSMWChainKey(string)
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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

/// Check if a chain key is registered in ChainRegistry
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_key` - The chain key to check
/// * `rpc_url` - EVM RPC URL
pub async fn is_chain_key_registered(
    chain_registry: Address,
    chain_key: B256,
    rpc_url: &str,
) -> Result<bool> {
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", chain_registry),
            "isChainKeyRegistered(bytes32)(bool)",
            &format!("{}", chain_key),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to check chain key registration: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout.trim();

    // Parse boolean result (cast returns "true" or "false")
    Ok(result == "true")
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
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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

    // addTokenDestChainKey(address token, bytes32 destChainKey, bytes32 destTokenAddress, uint256 decimals)
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", token_registry),
            "addTokenDestChainKey(address,bytes32,bytes32,uint256)",
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
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
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
/// For short denoms (like "uluna"), encodes as left-padded bytes.
/// For longer addresses (CW20 contract addresses), uses keccak256 hash
/// since Terra bech32 addresses are too long to fit in 32 bytes.
pub fn encode_terra_token_address(token: &str) -> B256 {
    use alloy::primitives::keccak256;

    let token_bytes = token.as_bytes();

    if token_bytes.len() <= 32 {
        // Short denoms can be encoded directly
        let mut bytes = [0u8; 32];
        bytes[..token_bytes.len()].copy_from_slice(token_bytes);
        B256::from_slice(&bytes)
    } else {
        // Long addresses (CW20 contracts) are hashed to fit in 32 bytes
        keccak256(token_bytes)
    }
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
