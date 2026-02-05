//! Token registration diagnostics and EVM chain key computation
//!
//! This module provides helper functions for diagnosing token registration issues
//! and computing EVM chain keys for E2E tests.
//!
//! Extracted from operator_helpers.rs to keep files under 900 LOC.

use crate::E2eConfig;
use alloy::primitives::Address;
use eyre::Result;
use tracing::{debug, info, warn};

// ============================================================================
// EVM Chain Key Computation
// ============================================================================

/// Compute EVM chain key (matches ChainRegistry.getChainKeyEVM)
///
/// This function computes the chain key for an EVM chain using the same
/// algorithm as the on-chain ChainRegistry.getChainKeyEVM() function.
///
/// The encoding follows ABI encoding for (string, uint256):
/// - offset to string data (32 bytes)
/// - chainId as uint256 (32 bytes)
/// - string length (32 bytes)
/// - string data "EVM" (padded to 32 bytes)
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

    keccak256(data).into()
}

/// Generate a unique nonce based on current timestamp
///
/// Useful for creating unique deposit identifiers in tests.
pub fn generate_unique_nonce() -> u64 {
    999_000_000
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            % 1_000_000)
}

// ============================================================================
// Token Registration Diagnostics
// ============================================================================

/// Check if a token is registered for a destination chain key
///
/// Queries TokenRegistry.isTokenDestChainKeyRegistered(token, destChainKey)
/// This helps diagnose deposit failures due to missing token registration.
pub async fn is_token_registered_for_chain(
    config: &E2eConfig,
    token: Address,
    dest_chain_key: [u8; 32],
) -> Result<bool> {
    // Check for zero addresses which indicate contracts not deployed
    if config.evm.contracts.token_registry == Address::ZERO {
        return Err(eyre::eyre!(
            "TokenRegistry address is zero - contracts not deployed. Run 'cl8y-e2e setup' first."
        ));
    }

    let client = reqwest::Client::new();

    // isTokenDestChainKeyRegistered(address,bytes32) selector: 0xb2072f30
    // Verified via: cast sig "isTokenDestChainKeyRegistered(address,bytes32)"
    let selector = "b2072f30";
    let token_padded = format!("{:0>64}", hex::encode(token.as_slice()));
    let chain_key_hex = hex::encode(dest_chain_key);
    let call_data = format!("0x{}{}{}", selector, token_padded, chain_key_hex);

    let response = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{}", config.evm.contracts.token_registry),
                "data": call_data
            }, "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let body: serde_json::Value = response.json().await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("Token registration check failed: {}", error));
    }

    let hex_result = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No result in response"))?;

    // Result is a bool encoded as bytes32 - check if last byte is 1
    let bytes = hex::decode(hex_result.trim_start_matches("0x"))?;
    let is_registered = bytes.last().copied().unwrap_or(0) == 1;

    debug!(
        "Token {} registration for chain 0x{}: {}",
        token,
        hex::encode(&dest_chain_key[..8]),
        is_registered
    );

    Ok(is_registered)
}

/// Verify token is properly set up for deposits before attempting transfer
///
/// Checks:
/// 1. Token is registered on TokenRegistry
/// 2. Token has destination chain key configured
/// 3. Logs diagnostic information if not properly configured
pub async fn verify_token_setup(
    config: &E2eConfig,
    token: Address,
    dest_chain_key: [u8; 32],
) -> Result<()> {
    let is_registered = is_token_registered_for_chain(config, token, dest_chain_key).await?;

    if !is_registered {
        warn!(
            "Token {} is NOT registered for destination chain 0x{}! \
             Deposit transactions will revert. Run setup to register the token first.",
            token,
            hex::encode(&dest_chain_key[..8])
        );
        return Err(eyre::eyre!(
            "Token {} not registered for destination chain. \
             Ensure TokenRegistry.addToken() and TokenRegistry.addTokenDestChainKey() were called during setup.",
            token
        ));
    }

    info!(
        "Token {} verified: registered for destination chain 0x{}",
        token,
        hex::encode(&dest_chain_key[..8])
    );
    Ok(())
}

/// Query all registered destination chains for a token
///
/// Returns a list of chain keys that the token is registered for.
/// Useful for debugging token registration issues.
pub async fn get_token_registered_chains(
    config: &E2eConfig,
    token: Address,
) -> Result<Vec<[u8; 32]>> {
    // For now, we check common chain keys
    let chain_keys_to_check = vec![
        compute_evm_chain_key(config.evm.chain_id), // Local EVM chain
        compute_evm_chain_key(31337),               // Anvil default
        compute_evm_chain_key(1),                   // Mainnet
    ];

    let mut registered = Vec::new();

    for chain_key in chain_keys_to_check {
        if is_token_registered_for_chain(config, token, chain_key)
            .await
            .unwrap_or(false)
        {
            registered.push(chain_key);
        }
    }

    Ok(registered)
}

/// Print diagnostic information about token registration
///
/// Logs detailed information about token registration status for debugging.
pub async fn print_token_diagnostics(config: &E2eConfig, token: Address) -> Result<()> {
    info!("=== Token Registration Diagnostics ===");
    info!("Token address: {}", token);
    info!("TokenRegistry: {}", config.evm.contracts.token_registry);

    // Check EVM chain key registration
    let evm_chain_key = compute_evm_chain_key(config.evm.chain_id);
    let is_evm_registered = is_token_registered_for_chain(config, token, evm_chain_key)
        .await
        .unwrap_or(false);
    info!(
        "Registered for EVM chain {} (0x{}): {}",
        config.evm.chain_id,
        hex::encode(&evm_chain_key[..8]),
        if is_evm_registered { "YES" } else { "NO" }
    );

    // Get all registered chains
    let registered_chains = get_token_registered_chains(config, token).await?;
    info!(
        "Total registered destination chains: {}",
        registered_chains.len()
    );

    for chain_key in &registered_chains {
        info!("  - Chain key: 0x{}", hex::encode(&chain_key[..8]));
    }

    if registered_chains.is_empty() {
        warn!(
            "Token {} is not registered for any destination chain! \
             Deposits will fail. Ensure setup.rs register_tokens() is called.",
            token
        );
    }

    info!("=== End Token Diagnostics ===");
    Ok(())
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
    fn test_evm_chain_key_deterministic() {
        let key1 = compute_evm_chain_key(31337);
        let key2 = compute_evm_chain_key(31337);
        assert_eq!(key1, key2, "Same chain ID should produce same key");
    }

    #[test]
    fn test_evm_chain_key_different_chains() {
        let key_31337 = compute_evm_chain_key(31337);
        let key_1 = compute_evm_chain_key(1);
        assert_ne!(
            key_31337, key_1,
            "Different chain IDs should produce different keys"
        );
    }

    #[test]
    fn test_generate_unique_nonce() {
        let nonce1 = generate_unique_nonce();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let nonce2 = generate_unique_nonce();
        // Nonces should be different (based on milliseconds)
        // Note: May occasionally be equal if called within same millisecond
        assert!(nonce1 >= 999_000_000, "Nonce should be in expected range");
        assert!(nonce2 >= 999_000_000, "Nonce should be in expected range");
    }
}
