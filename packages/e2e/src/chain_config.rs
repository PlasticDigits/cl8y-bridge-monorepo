//! Chain configuration and contract setup module
//!
//! This module handles:
//! - Role granting (OPERATOR_ROLE, CANCELER_ROLE) via AccessManager
//! - Chain key registration via ChainRegistry
//! - Token registration via TokenRegistry
//!
//! CW20 token deployment has been moved to `cw20_deploy` module to keep this file under 900 LOC.

use alloy::primitives::{Address, FixedBytes, B256};
use alloy::signers::local::PrivateKeySigner;
use eyre::{eyre, Result};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Type alias for 4-byte chain IDs used by ChainRegistry V2
pub type ChainId4 = FixedBytes<4>;

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

/// Register a COSMW chain on ChainRegistry via registerChain(string,bytes4)
///
/// The chain identifier format is "terraclassic_{chain_id}" (e.g., "terraclassic_localterra").
/// The caller specifies the predetermined 4-byte chain ID.
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The COSMW chain ID (e.g., "localterra")
/// * `predetermined_id` - The predetermined 4-byte chain ID to assign
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing
///
/// # Returns
/// The predetermined 4-byte chain ID (same as input)
pub async fn register_cosmw_chain_key(
    chain_registry: Address,
    chain_id: &str,
    predetermined_id: ChainId4,
    rpc_url: &str,
    private_key: &str,
) -> Result<ChainId4> {
    let identifier = format!("terraclassic_{}", chain_id);
    let chain_id_hex = format!("0x{}", hex::encode(predetermined_id));
    info!(
        "Registering COSMW chain with identifier: {}, chain_id: {}",
        identifier, chain_id_hex
    );

    // Check if already registered by computing hash and looking up chain ID
    let existing = get_chain_id_by_identifier(chain_registry, &identifier, rpc_url).await?;
    if existing != ChainId4::ZERO {
        info!(
            "Chain already registered with ID: 0x{}",
            hex::encode(existing)
        );
        return Ok(existing);
    }

    // Register chain: registerChain(string,bytes4)
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", chain_registry),
            "registerChain(string,bytes4)",
            &identifier,
            &chain_id_hex,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check if already registered
        if stderr.contains("already")
            || stderr.contains("ChainAlreadyRegistered")
            || stderr.contains("ChainIdAlreadyInUse")
        {
            let existing = get_chain_id_by_identifier(chain_registry, &identifier, rpc_url).await?;
            info!(
                "Chain already registered with ID: 0x{}",
                hex::encode(existing)
            );
            return Ok(existing);
        }
        return Err(eyre!("Failed to register chain: {}", stderr));
    }

    // Wait a moment for the transaction to be mined
    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("Chain registered with ID: {}", chain_id_hex);
    Ok(predetermined_id)
}

/// Get the chain ID for a COSMW chain from ChainRegistry
///
/// Uses computeIdentifierHash + getChainIdFromHash to look up the bytes4 chain ID.
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The COSMW chain ID (e.g., "localterra")
/// * `rpc_url` - EVM RPC URL
pub async fn get_cosmw_chain_key(
    chain_registry: Address,
    chain_id: &str,
    rpc_url: &str,
) -> Result<ChainId4> {
    let identifier = format!("terraclassic_{}", chain_id);
    get_chain_id_by_identifier(chain_registry, &identifier, rpc_url).await
}

/// Look up a chain's bytes4 ID by its string identifier
///
/// First computes the identifier hash via computeIdentifierHash(string),
/// then looks up the chain ID via getChainIdFromHash(bytes32).
/// Returns ChainId4::ZERO if the chain is not registered.
pub async fn get_chain_id_by_identifier(
    chain_registry: Address,
    identifier: &str,
    rpc_url: &str,
) -> Result<ChainId4> {
    // Step 1: Compute identifier hash
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", chain_registry),
            "computeIdentifierHash(string)(bytes32)",
            identifier,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to compute identifier hash: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash_hex = stdout.trim();

    // Step 2: Look up chain ID from hash
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", chain_registry),
            "getChainIdFromHash(bytes32)(bytes4)",
            hash_hex,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to get chain ID from hash: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let chain_id_hex = stdout.trim();
    debug!("Chain ID for '{}': {}", identifier, chain_id_hex);

    // Parse bytes4 value - cast returns e.g. "0x00000001"
    parse_bytes4(chain_id_hex)
}

/// Register an EVM chain on ChainRegistry via registerChain(string,bytes4)
///
/// The chain identifier format is "evm_{chain_id}" (e.g., "evm_31337").
/// The caller specifies the predetermined 4-byte chain ID.
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The EVM chain ID (e.g., 31337 for Anvil)
/// * `predetermined_id` - The predetermined 4-byte chain ID to assign
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing
///
/// # Returns
/// The predetermined 4-byte chain ID (same as input)
pub async fn register_evm_chain_key(
    chain_registry: Address,
    chain_id: u64,
    predetermined_id: ChainId4,
    rpc_url: &str,
    private_key: &str,
) -> Result<ChainId4> {
    let identifier = format!("evm_{}", chain_id);
    let chain_id_hex = format!("0x{}", hex::encode(predetermined_id));
    info!(
        "Registering EVM chain with identifier: {}, chain_id: {}",
        identifier, chain_id_hex
    );

    // Check if already registered
    let existing = get_chain_id_by_identifier(chain_registry, &identifier, rpc_url).await?;
    if existing != ChainId4::ZERO {
        info!(
            "EVM chain already registered with ID: 0x{}",
            hex::encode(existing)
        );
        return Ok(existing);
    }

    // Register chain: registerChain(string,bytes4)
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", chain_registry),
            "registerChain(string,bytes4)",
            &identifier,
            &chain_id_hex,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already")
            || stderr.contains("ChainAlreadyRegistered")
            || stderr.contains("ChainIdAlreadyInUse")
        {
            let existing = get_chain_id_by_identifier(chain_registry, &identifier, rpc_url).await?;
            info!(
                "EVM chain already registered with ID: 0x{}",
                hex::encode(existing)
            );
            return Ok(existing);
        }
        return Err(eyre!("Failed to register EVM chain: {}", stderr));
    }

    // Wait for the transaction to be mined
    tokio::time::sleep(Duration::from_secs(1)).await;

    info!("EVM chain registered with ID: {}", chain_id_hex);
    Ok(predetermined_id)
}

/// Get the chain ID for an EVM chain from ChainRegistry
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The EVM chain ID (e.g., 31337)
/// * `rpc_url` - EVM RPC URL
pub async fn get_evm_chain_key(
    chain_registry: Address,
    chain_id: u64,
    rpc_url: &str,
) -> Result<ChainId4> {
    let identifier = format!("evm_{}", chain_id);
    get_chain_id_by_identifier(chain_registry, &identifier, rpc_url).await
}

/// Check if a chain is registered in ChainRegistry by its bytes4 chain ID
///
/// # Arguments
/// * `chain_registry` - Address of the ChainRegistry contract
/// * `chain_id` - The 4-byte chain ID to check
/// * `rpc_url` - EVM RPC URL
pub async fn is_chain_registered(
    chain_registry: Address,
    chain_id: ChainId4,
    rpc_url: &str,
) -> Result<bool> {
    let chain_id_hex = format!("0x{}", hex::encode(chain_id));
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", chain_registry),
            "isChainRegistered(bytes4)(bool)",
            &chain_id_hex,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Failed to check chain registration: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout.trim();

    Ok(result == "true")
}

/// Parse a hex string into a FixedBytes<4> (bytes4)
///
/// Handles various formats: "0x00000001", "0x0000000100000000..." (ABI-padded)
fn parse_bytes4(hex_str: &str) -> Result<ChainId4> {
    let clean = hex_str.trim().trim_start_matches("0x");

    if clean.len() < 8 {
        return Err(eyre!("Hex string too short for bytes4: '{}'", hex_str));
    }

    // Take the first 8 hex chars (4 bytes)
    let bytes = hex::decode(&clean[..8])
        .map_err(|e| eyre!("Failed to decode bytes4 hex '{}': {}", hex_str, e))?;

    Ok(ChainId4::from_slice(&bytes))
}

/// Register Terra chain on ChainRegistry
///
/// This is the main entry point for Terra chain registration.
/// Uses identifier format "terraclassic_{chain_id}".
///
/// # Arguments
/// * `config` - E2E configuration
/// * `predetermined_id` - The predetermined 4-byte chain ID to assign to Terra
pub async fn register_terra_chain_key(
    config: &E2eConfig,
    predetermined_id: ChainId4,
) -> Result<ChainId4> {
    info!("Registering Terra chain on ChainRegistry");

    let chain_registry = config.evm.contracts.chain_registry;
    let rpc_url = config.evm.rpc_url.as_str();
    let private_key = format!("0x{:x}", config.evm.private_key);
    let chain_id = &config.terra.chain_id; // "localterra"

    register_cosmw_chain_key(
        chain_registry,
        chain_id,
        predetermined_id,
        rpc_url,
        &private_key,
    )
    .await
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

    // registerToken(address token, uint8 tokenType)
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", token_registry),
            "registerToken(address,uint8)",
            &format!("{}", token),
            &(bridge_type as u8).to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already")
            || stderr.contains("TokenAlreadyRegistered")
            || stderr.contains("AlreadyRegistered")
        {
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
    // Use the explicit isTokenRegistered function instead of getTokenBridgeType
    // getTokenBridgeType returns 0 for both unregistered tokens and MintBurn type
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", token_registry),
            "isTokenRegistered(address)(bool)",
            &format!("{}", token),
        ])
        .output()?;

    if !output.status.success() {
        // If the call reverts, token is not registered
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let is_registered = stdout.trim().eq_ignore_ascii_case("true");
    Ok(is_registered)
}

/// Set a destination chain mapping for a token on TokenRegistry
///
/// # Arguments
/// * `token_registry` - Address of the TokenRegistry contract
/// * `token` - Address of the source token
/// * `dest_chain_id` - The destination chain ID (bytes4)
/// * `dest_token_address` - The destination token address (bytes32-encoded)
/// * `decimals` - The token decimals on the destination chain
/// * `rpc_url` - EVM RPC URL
/// * `private_key` - Private key for signing
pub async fn add_token_dest_chain_key(
    token_registry: Address,
    token: Address,
    dest_chain_id: ChainId4,
    dest_token_address: B256,
    decimals: u8,
    rpc_url: &str,
    private_key: &str,
) -> Result<()> {
    let dest_chain_hex = format!("0x{}", hex::encode(dest_chain_id));
    info!(
        "Setting token destination for {}: dest_chain={}, dest_token={}, decimals={}",
        token, dest_chain_hex, dest_token_address, decimals
    );

    // Check if already registered
    if is_token_dest_chain_registered(token_registry, token, dest_chain_id, rpc_url).await? {
        info!(
            "Token {} destination chain {} already registered",
            token, dest_chain_hex
        );
        return Ok(());
    }

    // setTokenDestinationWithDecimals(address token, bytes4 destChain, bytes32 destToken, uint8 destDecimals)
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "send",
            "--rpc-url",
            rpc_url,
            "--private-key",
            private_key,
            &format!("{}", token_registry),
            "setTokenDestinationWithDecimals(address,bytes4,bytes32,uint8)",
            &format!("{}", token),
            &dest_chain_hex,
            &format!("{}", dest_token_address),
            &decimals.to_string(),
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("already") {
            info!("Token destination already set");
            return Ok(());
        }
        return Err(eyre!("Failed to set token destination: {}", stderr));
    }

    info!("Token destination set successfully");
    Ok(())
}

/// Check if a token's destination chain is registered
///
/// Uses getDestToken(address,bytes4) and checks if the result is non-zero.
async fn is_token_dest_chain_registered(
    token_registry: Address,
    token: Address,
    dest_chain_id: ChainId4,
    rpc_url: &str,
) -> Result<bool> {
    let dest_chain_hex = format!("0x{}", hex::encode(dest_chain_id));
    let output = std::process::Command::new("cast")
        .env("FOUNDRY_DISABLE_NIGHTLY_WARNING", "1")
        .args([
            "call",
            "--rpc-url",
            rpc_url,
            &format!("{}", token_registry),
            "getDestToken(address,bytes4)(bytes32)",
            &format!("{}", token),
            &dest_chain_hex,
        ])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout.trim();

    // If getDestToken returns zero bytes32, the destination is not set
    let dest_token: B256 = result.parse().unwrap_or(B256::ZERO);
    Ok(dest_token != B256::ZERO)
}

/// Encode a Terra token (native denom or CW20 address) as bytes32
///
/// Uses the unified encoding from multichain-rs to match the Terra contract's
/// `encode_token_address(deps, token)` function:
/// - CW20 addresses (start with "terra1"): bech32 decode → 20 raw bytes → left-pad to 32 bytes
///   (matches Terra's `addr_canonicalize → encode_terra_address`)
/// - Native denoms (like "uluna"): `keccak256(denom_bytes)`
///   (matches Terra's `keccak256(token.as_bytes())`)
///
/// This ensures the EVM TokenRegistry's `destToken` matches the hash computed
/// during Terra's `WithdrawSubmit`, which is critical for cross-chain verification.
pub fn encode_terra_token_address(token: &str) -> B256 {
    if token.starts_with("terra1") {
        // CW20 contract address - bech32 decode to raw 20 bytes, left-pad to 32
        // Matches Terra contract's encode_token_address when addr_validate succeeds
        let bytes32 = multichain_rs::hash::encode_terra_address_to_bytes32(token)
            .unwrap_or_else(|e| panic!("Failed to encode CW20 address '{}': {}", token, e));
        B256::from_slice(&bytes32)
    } else {
        // Native denom - keccak256 of the denom string
        // Matches Terra contract's encode_token_address for native tokens
        let hash = multichain_rs::hash::keccak256(token.as_bytes());
        B256::from_slice(&hash)
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
/// * `terra_chain_key` - The Terra chain ID (bytes4) from ChainRegistry
pub async fn register_test_tokens(
    config: &E2eConfig,
    test_token: Option<Address>,
    terra_cw20_address: Option<&str>,
    terra_chain_key: ChainId4,
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
    /// Terra chain ID (bytes4)
    pub terra_chain_key: ChainId4,
    /// CW20 token address (if deployed)
    pub cw20_address: Option<String>,
    /// Whether roles were granted successfully
    pub roles_granted: bool,
    /// Whether token was registered
    pub token_registered: bool,
}

/// Perform complete chain configuration for E2E testing
///
/// This function performs all 4 setup steps:
/// 1. Grant OPERATOR_ROLE and CANCELER_ROLE to test accounts
/// 2. Register Terra chain via ChainRegistry.registerChain()
/// 3. Register test tokens on TokenRegistry with destination chain mappings
/// 4. Deploy CW20 test token on LocalTerra (optional)
///
/// # Arguments
/// * `config` - E2E configuration
/// * `project_root` - Path to project root (for finding WASM files)
/// * `test_token` - Optional ERC20 token address to register
/// * `terra_predetermined_id` - The predetermined 4-byte chain ID for Terra
pub async fn configure_chains(
    config: &E2eConfig,
    project_root: &std::path::Path,
    test_token: Option<Address>,
    terra_predetermined_id: ChainId4,
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
    let terra_chain_key = register_terra_chain_key(config, terra_predetermined_id).await?;
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
    fn test_encode_terra_token_address_native_denom() {
        let uluna = encode_terra_token_address("uluna");
        // Should be keccak256("uluna"), matching Terra contract's encode_token_address
        let expected = multichain_rs::hash::keccak256(b"uluna");
        assert_eq!(uluna.as_slice(), &expected);
    }

    #[test]
    fn test_encode_terra_token_address_native_denoms() {
        // Verify encoding matches multichain-rs keccak256 for native denoms
        for denom in &[
            "uluna",
            "uusd",
            "ibc/0EF15DF2F02480ADE0BB6E85D9EBB5DAEA2836D3860E9F97F9AADE4F57A31AA0",
        ] {
            let encoded = encode_terra_token_address(denom);
            let expected = multichain_rs::hash::keccak256(denom.as_bytes());
            assert_eq!(
                encoded.as_slice(),
                &expected,
                "Token encoding mismatch for '{}'",
                denom
            );
        }
    }

    #[test]
    fn test_encode_terra_token_address_cw20() {
        // CW20 addresses should be bech32-decoded and left-padded, matching
        // Terra contract's encode_terra_address (canonicalize + left-pad)
        let cw20 = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let encoded = encode_terra_token_address(cw20);
        let expected = multichain_rs::hash::encode_terra_address_to_bytes32(cw20).unwrap();
        assert_eq!(encoded.as_slice(), &expected);
        // First 12 bytes should be zero-padding
        assert_eq!(&encoded.as_slice()[..12], &[0u8; 12]);
    }

    #[test]
    fn test_bridge_type_values() {
        assert_eq!(BridgeType::MintBurn as u8, 0);
        assert_eq!(BridgeType::LockUnlock as u8, 1);
    }
}
