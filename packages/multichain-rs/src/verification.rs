//! Shared deposit verification logic for cross-chain transfers
//!
//! Both the operator and canceler need to verify that deposits exist on source
//! chains before approving (operator) or validating (canceler) withdrawals.
//! This module provides the shared verification functions.
//!
//! # Verification Flow
//!
//! 1. Given a withdraw hash and source chain ID, determine the source chain type
//! 2. Route to the correct verification method:
//!    - EVM source → query `getDeposit(hash)` on the source chain's bridge
//!    - Terra source → query `xchain_hash_id` on Terra LCD
//! 3. Return whether the deposit exists

#![allow(dead_code)]

use alloy::primitives::{Address, FixedBytes};
use alloy::providers::ProviderBuilder;
use alloy::sol;
use eyre::{eyre, Result, WrapErr};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::hash::bytes32_to_hex;
use crate::types::ChainId;

// Bridge contract ABI for deposit verification
sol! {
    #[sol(rpc)]
    contract Bridge {
        /// Get deposit record by hash (V2 DepositRecord struct)
        function getDeposit(bytes32 xchainHashId) external view returns (
            bytes4 destChain,
            bytes32 srcAccount,
            bytes32 destAccount,
            address token,
            uint256 amount,
            uint64 nonce,
            uint256 fee,
            uint256 timestamp
        );
    }
}

// ============================================================================
// Source Chain Endpoints
// ============================================================================

/// Configuration for verifying deposits on a known chain.
///
/// Used to build a routing table from V2 chain ID → verification parameters.
#[derive(Debug, Clone)]
pub struct SourceChainEndpoint {
    /// RPC URL for the source chain
    pub rpc_url: String,
    /// Bridge contract address on the source chain
    pub bridge_address: String,
}

/// Build source chain endpoints map from configured chain peers.
///
/// The resulting map can be passed to `verify_deposit_on_source()` for routing.
pub fn build_source_endpoints(
    chains: &[(ChainId, String, String)], // (v2_id, rpc_url, bridge_address)
) -> HashMap<[u8; 4], SourceChainEndpoint> {
    let mut endpoints = HashMap::new();
    for (chain_id, rpc_url, bridge_address) in chains {
        endpoints.insert(
            chain_id.0,
            SourceChainEndpoint {
                rpc_url: rpc_url.clone(),
                bridge_address: bridge_address.clone(),
            },
        );
    }
    endpoints
}

// ============================================================================
// EVM Deposit Verification
// ============================================================================

/// Verify a deposit exists on an EVM chain by querying `getDeposit(hash)`.
///
/// Returns `true` if the deposit record has a non-zero timestamp.
/// Returns `false` if the deposit doesn't exist (timestamp == 0).
/// Returns `Err` if the RPC call fails.
pub async fn verify_evm_deposit(
    rpc_url: &str,
    bridge_address: &str,
    xchain_hash_id: &[u8; 32],
) -> Result<bool> {
    let provider = ProviderBuilder::new().on_http(rpc_url.parse().wrap_err("Invalid RPC URL")?);

    let address =
        Address::from_str(bridge_address).wrap_err("Invalid bridge address for verification")?;

    let contract = Bridge::new(address, &provider);
    let hash_fixed = FixedBytes::from(*xchain_hash_id);

    match contract.getDeposit(hash_fixed).call().await {
        Ok(deposit) => {
            if deposit.timestamp.is_zero() {
                debug!(
                    hash = %bytes32_to_hex(xchain_hash_id),
                    rpc = rpc_url,
                    "No deposit found on source EVM chain (timestamp=0)"
                );
                return Ok(false);
            }

            info!(
                hash = %bytes32_to_hex(xchain_hash_id),
                nonce = deposit.nonce,
                amount = %deposit.amount,
                dest_chain = %format!("0x{}", hex::encode(deposit.destChain.0)),
                rpc = rpc_url,
                "Deposit verified on source EVM chain"
            );
            Ok(true)
        }
        Err(e) => {
            warn!(
                error = %e,
                hash = %bytes32_to_hex(xchain_hash_id),
                rpc = rpc_url,
                "Failed to query getDeposit on source EVM chain"
            );
            Err(eyre!("Failed to verify EVM deposit: {}", e))
        }
    }
}

// ============================================================================
// Terra Deposit Verification
// ============================================================================

/// Verify a deposit exists on Terra by querying the bridge's `xchain_hash_id` query.
///
/// Returns `true` if the query returns a non-null `data` field.
/// Returns `false` if the deposit doesn't exist.
/// Returns `Err` if the LCD query fails.
pub async fn verify_terra_deposit(
    lcd_url: &str,
    bridge_address: &str,
    xchain_hash_id: &[u8; 32],
) -> Result<bool> {
    use base64::Engine;

    let query = serde_json::json!({
        "xchain_hash_id": {
            "xchain_hash_id": base64::engine::general_purpose::STANDARD.encode(xchain_hash_id)
        }
    });
    let query_b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_vec(&query)?);
    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
        lcd_url.trim_end_matches('/'),
        bridge_address,
        query_b64
    );

    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| eyre!("Terra deposit verification request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(eyre!(
            "Terra deposit verification failed with status {}",
            response.status()
        ));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| eyre!("Failed to parse Terra deposit verification response: {}", e))?;

    let exists = terra_deposit_exists_in_response(&body);

    if exists {
        info!(
            hash = %bytes32_to_hex(xchain_hash_id),
            nonce = body["data"]["nonce"].as_u64().unwrap_or_default(),
            amount = body["data"]["amount"].as_str().unwrap_or("?"),
            "Terra deposit verified on source chain"
        );
    } else {
        debug!(
            hash = %bytes32_to_hex(xchain_hash_id),
            "Terra deposit not found on source chain"
        );
    }

    Ok(exists)
}

/// Check if the Terra LCD query response contains a deposit record.
pub fn terra_deposit_exists_in_response(body: &serde_json::Value) -> bool {
    body.get("data").is_some_and(|data| !data.is_null())
}

// ============================================================================
// Routed Verification
// ============================================================================

/// Route deposit verification to the correct chain based on source chain ID.
///
/// # Arguments
/// - `src_chain_id` — 4-byte V2 chain ID of the source chain
/// - `xchain_hash_id` — the transfer hash to verify
/// - `evm_endpoints` — map of V2 chain ID → EVM verification parameters
/// - `terra_chain_id` — V2 chain ID of the Terra chain (if configured)
/// - `terra_lcd_url` — Terra LCD URL (if configured)
/// - `terra_bridge_address` — Terra bridge contract address (if configured)
///
/// Returns `Ok(true)` if deposit exists, `Ok(false)` if not found or unknown chain.
pub async fn route_verification(
    src_chain_id: &[u8; 4],
    xchain_hash_id: &[u8; 32],
    evm_endpoints: &HashMap<[u8; 4], SourceChainEndpoint>,
    terra_chain_id: Option<&ChainId>,
    terra_lcd_url: Option<&str>,
    terra_bridge_address: Option<&str>,
) -> Result<bool> {
    // Check if source is Terra
    if let Some(terra_id) = terra_chain_id {
        if src_chain_id == terra_id.as_bytes() {
            let lcd_url = terra_lcd_url.ok_or_else(|| {
                eyre!("Terra LCD URL not configured for Terra-source verification")
            })?;
            let bridge = terra_bridge_address.ok_or_else(|| {
                eyre!("Terra bridge address not configured for Terra-source verification")
            })?;
            return verify_terra_deposit(lcd_url, bridge, xchain_hash_id).await;
        }
    }

    // Check if source is a known EVM chain
    if let Some(endpoint) = evm_endpoints.get(src_chain_id) {
        return verify_evm_deposit(&endpoint.rpc_url, &endpoint.bridge_address, xchain_hash_id).await;
    }

    // Unknown source chain: fail closed
    warn!(
        hash = %bytes32_to_hex(xchain_hash_id),
        src_chain = %format!("0x{}", hex::encode(src_chain_id)),
        known_evm_chains = evm_endpoints.len(),
        "Unknown source chain ID — refusing to verify (fail closed). \
         Configure the source chain in EVM_CHAINS_COUNT or check Terra config."
    );
    Ok(false)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terra_deposit_exists_in_response() {
        // Deposit exists
        let body = serde_json::json!({
            "data": {
                "nonce": 1,
                "amount": "1000000",
                "xchain_hash_id": "abc123"
            }
        });
        assert!(terra_deposit_exists_in_response(&body));

        // No deposit (null data)
        let body = serde_json::json!({ "data": null });
        assert!(!terra_deposit_exists_in_response(&body));

        // No data field at all
        let body = serde_json::json!({ "error": "not found" });
        assert!(!terra_deposit_exists_in_response(&body));
    }

    #[test]
    fn test_build_source_endpoints() {
        let chains = vec![
            (
                ChainId::from_u32(1),
                "http://localhost:8545".to_string(),
                "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
            ),
            (
                ChainId::from_u32(3),
                "http://localhost:8546".to_string(),
                "0x5FbDB2315678afecb367f032d93F642f64180aa3".to_string(),
            ),
        ];

        let endpoints = build_source_endpoints(&chains);
        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.contains_key(&[0, 0, 0, 1]));
        assert!(endpoints.contains_key(&[0, 0, 0, 3]));

        let ep = endpoints.get(&[0, 0, 0, 1]).unwrap();
        assert_eq!(ep.rpc_url, "http://localhost:8545");
    }

    #[tokio::test]
    async fn test_route_verification_unknown_chain_returns_false() {
        let endpoints = HashMap::new();
        let unknown_chain = [0, 0, 0, 99];
        let hash = [1u8; 32];

        let result = route_verification(&unknown_chain, &hash, &endpoints, None, None, None).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_route_verification_terra_without_config_errors() {
        let endpoints = HashMap::new();
        let terra_chain = ChainId::from_u32(2);
        let hash = [1u8; 32];

        // Terra chain ID matches but LCD URL not configured
        let result = route_verification(
            terra_chain.as_bytes(),
            &hash,
            &endpoints,
            Some(&terra_chain),
            None, // No LCD URL
            Some("terra1bridge"),
        )
        .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("LCD URL not configured"));
    }
}
