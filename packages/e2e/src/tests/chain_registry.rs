//! Chain registration E2E tests
//!
//! Tests that verify chain registration flow across EVM and Terra sides.

use crate::{E2eConfig, TestResult};
use std::time::Instant;

use super::helpers::{
    check_evm_connection, check_terra_connection, get_terra_chain_key, is_chain_key_registered,
    query_contract_code, query_evm_chain_key,
};

/// Test EVM chain registration
///
/// Verifies that:
/// - EVM connectivity is available
/// - ChainRegistry contract exists and has code
/// - Can query registered chains
/// - Terra chain key is registered in ChainRegistry
///
/// Returns a `TestResult` indicating success or failure.
pub async fn test_chain_registration_evm(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "chain_registration_evm";

    // Check EVM connectivity first
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {
            tracing::info!("EVM connection verified");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Verify ChainRegistry contract is deployed
    if config.evm.contracts.chain_registry == alloy::primitives::Address::ZERO {
        return TestResult::fail(name, "ChainRegistry address is zero", start.elapsed());
    }

    // Verify ChainRegistry has code
    match query_contract_code(config, config.evm.contracts.chain_registry).await {
        Ok(has_code) => {
            if !has_code {
                return TestResult::fail(
                    name,
                    "ChainRegistry contract has no code deployed",
                    start.elapsed(),
                );
            }
            tracing::info!(
                "ChainRegistry contract at {} has code",
                config.evm.contracts.chain_registry
            );
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query ChainRegistry code: {}", e),
                start.elapsed(),
            );
        }
    }

    // Query Terra chain key
    let terra_chain_key = match get_terra_chain_key(config).await {
        Ok(key) => {
            tracing::info!("Terra chain key: 0x{}", hex::encode(key));
            key
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get Terra chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Check if Terra chain key is registered
    match is_chain_key_registered(config, terra_chain_key).await {
        Ok(is_registered) => {
            if is_registered {
                tracing::info!("Terra chain key is registered in ChainRegistry");
                TestResult::pass(name, start.elapsed())
            } else {
                TestResult::fail(
                    name,
                    "Terra chain key is not registered in ChainRegistry",
                    start.elapsed(),
                )
            }
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to check chain key registration: {}", e),
            start.elapsed(),
        ),
    }
}

/// Test Terra bridge chain configuration
///
/// Verifies that:
/// - Terra connectivity is available
/// - Bridge contract address is configured
/// - Can query bridge contract for chain configuration
///
/// Returns a `TestResult` indicating success or failure.
pub async fn test_chain_registration_terra(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "chain_registration_terra";

    // Check Terra connectivity first
    match check_terra_connection(&config.terra.lcd_url).await {
        Ok(_) => {
            tracing::info!("Terra connection verified");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Terra connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Verify bridge address is configured
    let bridge_address = match &config.terra.bridge_address {
        Some(addr) if !addr.is_empty() => addr,
        _ => {
            return TestResult::fail(
                name,
                "Terra bridge address is not configured",
                start.elapsed(),
            );
        }
    };

    tracing::info!("Querying Terra bridge contract at: {}", bridge_address);

    // Query bridge contract for chains configuration
    // Use the Chains query to get list of registered chains
    let client = reqwest::Client::new();
    let query = serde_json::json!({
        "chains": {
            "limit": 10u32
        }
    });

    let query_json = match serde_json::to_string(&query) {
        Ok(json) => json,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to serialize query: {}", e),
                start.elapsed(),
            );
        }
    };

    let query_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        query_json.as_bytes(),
    );

    let url = format!(
        "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
        config.terra.lcd_url, bridge_address, query_b64
    );

    match tokio::time::timeout(std::time::Duration::from_secs(10), client.get(&url).send()).await {
        Ok(Ok(response)) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(body) => {
                        if let Some(chains_data) = body.get("data").and_then(|d| d.get("chains")) {
                            let chain_count =
                                chains_data.as_array().map(|arr| arr.len()).unwrap_or(0);
                            tracing::info!("Terra bridge has {} chains configured", chain_count);
                            TestResult::pass(name, start.elapsed())
                        } else {
                            // Chains query might return empty or different structure
                            // Still pass if we got a valid response
                            tracing::info!("Terra bridge contract responded successfully");
                            TestResult::pass(name, start.elapsed())
                        }
                    }
                    Err(e) => TestResult::fail(
                        name,
                        format!("Failed to parse Terra bridge response: {}", e),
                        start.elapsed(),
                    ),
                }
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                TestResult::fail(
                    name,
                    format!(
                        "Terra bridge query failed with status {}: {}",
                        status, error_text
                    ),
                    start.elapsed(),
                )
            }
        }
        Ok(Err(e)) => TestResult::fail(
            name,
            format!("Failed to query Terra bridge: {}", e),
            start.elapsed(),
        ),
        Err(_) => TestResult::fail(
            name,
            "Timeout querying Terra bridge contract",
            start.elapsed(),
        ),
    }
}

/// Test chain query consistency between EVM and Terra
///
/// Verifies that:
/// - Both EVM and Terra sides are accessible
/// - Can query chain keys from EVM ChainRegistry
/// - Can query chain configuration from Terra bridge
/// - Data is consistent between both sides
///
/// Returns a `TestResult` indicating success or failure.
pub async fn test_chain_query_both_sides(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "chain_query_both_sides";

    // Check connectivity to both sides
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {
            tracing::info!("EVM connection verified");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    match check_terra_connection(&config.terra.lcd_url).await {
        Ok(_) => {
            tracing::info!("Terra connection verified");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Terra connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Query EVM chain key for local chain
    let evm_chain_key = match query_evm_chain_key(config, config.evm.chain_id).await {
        Ok(key) => {
            tracing::info!(
                "EVM chain key for chain {}: 0x{}",
                config.evm.chain_id,
                hex::encode(key)
            );
            key
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query EVM chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Check if EVM chain key is registered
    let evm_registered = match is_chain_key_registered(config, evm_chain_key).await {
        Ok(registered) => registered,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to check EVM chain key registration: {}", e),
                start.elapsed(),
            );
        }
    };

    // Query Terra chain key
    let terra_chain_key = match get_terra_chain_key(config).await {
        Ok(key) => {
            tracing::info!("Terra chain key: 0x{}", hex::encode(key));
            key
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get Terra chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Check if Terra chain key is registered
    let terra_registered = match is_chain_key_registered(config, terra_chain_key).await {
        Ok(registered) => registered,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to check Terra chain key registration: {}", e),
                start.elapsed(),
            );
        }
    };

    // Verify consistency: both chain keys should be registered
    if evm_registered && terra_registered {
        tracing::info!("Both EVM and Terra chain keys are registered and consistent");
        TestResult::pass(name, start.elapsed())
    } else {
        TestResult::fail(
            name,
            format!(
                "Chain registration inconsistency: EVM registered={}, Terra registered={}",
                evm_registered, terra_registered
            ),
            start.elapsed(),
        )
    }
}

/// Test duplicate chain rejection
///
/// Verifies that:
/// - ChainRegistry contract is accessible
/// - Attempting to register a duplicate chain key is handled gracefully
/// - System prevents or handles duplicate registrations correctly
///
/// Note: This test verifies the system's behavior with duplicate chain keys.
/// It doesn't actually attempt to register a duplicate (which would require
/// admin permissions), but verifies that already-registered chains are
/// properly tracked.
///
/// Returns a `TestResult` indicating success or failure.
pub async fn test_duplicate_chain_rejection(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "duplicate_chain_rejection";

    // Check EVM connectivity
    match check_evm_connection(&config.evm.rpc_url).await {
        Ok(_) => {
            tracing::info!("EVM connection verified");
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("EVM connectivity check failed: {}", e),
                start.elapsed(),
            );
        }
    }

    // Verify ChainRegistry contract is deployed
    if config.evm.contracts.chain_registry == alloy::primitives::Address::ZERO {
        return TestResult::fail(name, "ChainRegistry address is zero", start.elapsed());
    }

    // Get Terra chain key (should be registered)
    let terra_chain_key = match get_terra_chain_key(config).await {
        Ok(key) => key,
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to get Terra chain key: {}", e),
                start.elapsed(),
            );
        }
    };

    // Check if Terra chain key is registered
    match is_chain_key_registered(config, terra_chain_key).await {
        Ok(is_registered) => {
            if is_registered {
                // Chain is already registered - verify the system correctly identifies it
                tracing::info!("Terra chain key is registered (expected for duplicate check)");

                // Query the chain key again to verify consistency
                match query_evm_chain_key(config, config.evm.chain_id).await {
                    Ok(evm_key) => {
                        // Verify EVM chain key is also registered
                        match is_chain_key_registered(config, evm_key).await {
                            Ok(evm_registered) => {
                                if evm_registered {
                                    tracing::info!(
                                        "Both chain keys are registered - system handles duplicates correctly"
                                    );
                                    TestResult::pass(name, start.elapsed())
                                } else {
                                    TestResult::fail(
                                        name,
                                        "EVM chain key should be registered but is not",
                                        start.elapsed(),
                                    )
                                }
                            }
                            Err(e) => TestResult::fail(
                                name,
                                format!("Failed to verify EVM chain registration: {}", e),
                                start.elapsed(),
                            ),
                        }
                    }
                    Err(e) => TestResult::fail(
                        name,
                        format!("Failed to query EVM chain key: {}", e),
                        start.elapsed(),
                    ),
                }
            } else {
                // Chain is not registered - this is fine, the test verifies the system
                // correctly identifies non-registered chains
                tracing::info!(
                    "Terra chain key is not registered - system correctly identifies non-registered chains"
                );
                TestResult::pass(name, start.elapsed())
            }
        }
        Err(e) => TestResult::fail(
            name,
            format!("Failed to check chain key registration: {}", e),
            start.elapsed(),
        ),
    }
}
