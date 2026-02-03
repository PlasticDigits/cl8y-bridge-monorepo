//! Integration tests for CL8Y Canceler
//!
//! These tests require real infrastructure:
//! - Anvil running on localhost:8545
//! - LocalTerra running on localhost:1317
//! - Contracts deployed with bridge addresses set via environment
//!
//! Run with: cargo test --test integration_test -- --ignored --nocapture
//! Or set INTEGRATION_TEST=1 and run: cargo test --test integration_test
//!
//! Required environment variables:
//! - EVM_RPC_URL (default: http://localhost:8545)
//! - TERRA_LCD_URL (default: http://localhost:1317)
//! - EVM_BRIDGE_ADDRESS (required for contract tests)
//! - TERRA_BRIDGE_ADDRESS (required for contract tests)
//! - EVM_PRIVATE_KEY (required for transaction tests)
//! - TERRA_MNEMONIC (required for transaction tests)

use std::env;
use std::time::Duration;

/// Check if integration tests should run
fn should_run_integration() -> bool {
    env::var("INTEGRATION_TEST").is_ok() || env::var("CI").is_ok()
}

/// Test EVM RPC URL
fn evm_rpc_url() -> String {
    env::var("EVM_RPC_URL").unwrap_or_else(|_| "http://localhost:8545".to_string())
}

/// Test Terra LCD URL
fn terra_lcd_url() -> String {
    env::var("TERRA_LCD_URL").unwrap_or_else(|_| "http://localhost:1317".to_string())
}

// ============================================================================
// Infrastructure Connectivity Tests
// ============================================================================

mod infrastructure {
    use super::*;

    /// Test Anvil connectivity
    #[tokio::test]
    #[ignore = "requires Anvil running"]
    async fn test_anvil_connectivity() {
        let client = reqwest::Client::new();
        let url = evm_rpc_url();

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(r#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}"#)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => {
                assert!(resp.status().is_success(), "Anvil returned error status");
                let json: serde_json::Value = resp.json().await.unwrap();
                assert!(json["result"].is_string(), "Expected block number result");
                println!("Anvil block number: {}", json["result"]);
            }
            Err(e) => {
                panic!("Failed to connect to Anvil at {}: {}", url, e);
            }
        }
    }

    /// Test Anvil chain ID
    #[tokio::test]
    #[ignore = "requires Anvil running"]
    async fn test_anvil_chain_id() {
        let client = reqwest::Client::new();
        let url = evm_rpc_url();

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(r#"{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}"#)
            .send()
            .await
            .expect("Failed to connect to Anvil");

        let json: serde_json::Value = response.json().await.unwrap();
        let chain_id = json["result"].as_str().expect("Expected chain ID");

        // Anvil default chain ID is 31337 (0x7a69)
        assert_eq!(chain_id, "0x7a69", "Expected Anvil chain ID 31337");
        println!("Anvil chain ID: {} (31337)", chain_id);
    }

    /// Test LocalTerra connectivity
    #[tokio::test]
    #[ignore = "requires LocalTerra running"]
    async fn test_localterra_connectivity() {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/cosmos/base/tendermint/v1beta1/node_info",
            terra_lcd_url()
        );

        let response = client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => {
                assert!(
                    resp.status().is_success(),
                    "LocalTerra returned error status"
                );
                let json: serde_json::Value = resp.json().await.unwrap();
                let network = json["default_node_info"]["network"]
                    .as_str()
                    .expect("Expected network field");
                println!("LocalTerra network: {}", network);
            }
            Err(e) => {
                panic!("Failed to connect to LocalTerra at {}: {}", url, e);
            }
        }
    }

    /// Test LocalTerra block production
    #[tokio::test]
    #[ignore = "requires LocalTerra running"]
    async fn test_localterra_blocks() {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/cosmos/base/tendermint/v1beta1/blocks/latest",
            terra_lcd_url()
        );

        let response = client
            .get(&url)
            .send()
            .await
            .expect("Failed to connect to LocalTerra");

        let json: serde_json::Value = response.json().await.unwrap();
        let height = json["block"]["header"]["height"]
            .as_str()
            .expect("Expected height");

        let height_num: u64 = height.parse().expect("Height should be numeric");
        assert!(height_num > 0, "Block height should be positive");
        println!("LocalTerra height: {}", height_num);
    }
}

// ============================================================================
// EVM Client Tests
// ============================================================================

mod evm_client {
    use super::*;

    /// Test EVM account balance query
    #[tokio::test]
    #[ignore = "requires Anvil running"]
    async fn test_evm_balance_query() {
        let client = reqwest::Client::new();
        let url = evm_rpc_url();

        // Default Anvil test account
        let test_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [test_address, "latest"],
            "id": 1
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .expect("Failed to query balance");

        let json: serde_json::Value = response.json().await.unwrap();
        let balance = json["result"].as_str().expect("Expected balance");

        // Parse balance (hex string)
        let balance_wei = u128::from_str_radix(&balance[2..], 16).expect("Invalid balance hex");

        // Should have significant balance (10000 ETH default)
        assert!(balance_wei > 0, "Balance should be positive");
        println!("Account {} balance: {} wei", test_address, balance_wei);
    }

    /// Test EVM gas price query
    #[tokio::test]
    #[ignore = "requires Anvil running"]
    async fn test_evm_gas_price() {
        let client = reqwest::Client::new();
        let url = evm_rpc_url();

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(r#"{"jsonrpc":"2.0","method":"eth_gasPrice","params":[],"id":1}"#)
            .send()
            .await
            .expect("Failed to query gas price");

        let json: serde_json::Value = response.json().await.unwrap();
        let gas_price = json["result"].as_str().expect("Expected gas price");

        let gas_price_wei =
            u128::from_str_radix(&gas_price[2..], 16).expect("Invalid gas price hex");
        assert!(gas_price_wei > 0, "Gas price should be positive");
        println!("Gas price: {} wei", gas_price_wei);
    }

    /// Test EVM contract call (if bridge deployed)
    #[tokio::test]
    #[ignore = "requires Anvil + deployed bridge"]
    async fn test_evm_bridge_withdraw_delay() {
        let bridge_address = match env::var("EVM_BRIDGE_ADDRESS") {
            Ok(addr) => addr,
            Err(_) => {
                println!("Skipping: EVM_BRIDGE_ADDRESS not set");
                return;
            }
        };

        let client = reqwest::Client::new();
        let url = evm_rpc_url();

        // withdrawDelay() selector: 0x0ebb172a
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": bridge_address,
                "data": "0x0ebb172a"
            }, "latest"],
            "id": 1
        });

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await
            .expect("Failed to call contract");

        let json: serde_json::Value = response.json().await.unwrap();

        if let Some(result) = json["result"].as_str() {
            if result != "0x" {
                let delay = u64::from_str_radix(&result[2..], 16).expect("Invalid delay hex");
                println!("Bridge withdraw delay: {} seconds", delay);
                assert!(delay > 0, "Delay should be positive");
            }
        } else if let Some(error) = json["error"].as_object() {
            println!("Contract call error: {:?}", error);
        }
    }
}

// ============================================================================
// Terra Client Tests
// ============================================================================

mod terra_client {
    use super::*;

    /// Test Terra account balance query
    #[tokio::test]
    #[ignore = "requires LocalTerra running"]
    async fn test_terra_balance_query() {
        let client = reqwest::Client::new();

        // LocalTerra test1 account
        let test_address = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
        let url = format!(
            "{}/cosmos/bank/v1beta1/balances/{}",
            terra_lcd_url(),
            test_address
        );

        let response = client
            .get(&url)
            .send()
            .await
            .expect("Failed to query balance");

        let json: serde_json::Value = response.json().await.unwrap();
        let balances = json["balances"]
            .as_array()
            .expect("Expected balances array");

        println!("Account {} has {} coin types", test_address, balances.len());

        for balance in balances {
            let denom = balance["denom"].as_str().unwrap_or("unknown");
            let amount = balance["amount"].as_str().unwrap_or("0");
            println!("  {}: {}", denom, amount);
        }
    }

    /// Test Terra contract query (if bridge deployed)
    #[tokio::test]
    #[ignore = "requires LocalTerra + deployed bridge"]
    async fn test_terra_bridge_config() {
        let bridge_address = match env::var("TERRA_BRIDGE_ADDRESS") {
            Ok(addr) => addr,
            Err(_) => {
                println!("Skipping: TERRA_BRIDGE_ADDRESS not set");
                return;
            }
        };

        let client = reqwest::Client::new();

        // Query config
        let query = serde_json::json!({"config": {}});
        let query_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            serde_json::to_string(&query).unwrap().as_bytes(),
        );

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            terra_lcd_url(),
            bridge_address,
            query_b64
        );

        match client.get(&url).send().await {
            Ok(resp) => {
                let json: serde_json::Value = resp.json().await.unwrap();
                if let Some(data) = json.get("data") {
                    println!("Bridge config: {:?}", data);
                    assert!(data.get("owner").is_some(), "Config should have owner");
                } else if let Some(error) = json.get("message") {
                    println!("Query error: {}", error);
                }
            }
            Err(e) => {
                println!("Request failed: {}", e);
            }
        }
    }

    /// Test Terra pending approvals query (if bridge deployed)
    #[tokio::test]
    #[ignore = "requires LocalTerra + deployed bridge"]
    async fn test_terra_pending_approvals() {
        let bridge_address = match env::var("TERRA_BRIDGE_ADDRESS") {
            Ok(addr) => addr,
            Err(_) => {
                println!("Skipping: TERRA_BRIDGE_ADDRESS not set");
                return;
            }
        };

        let client = reqwest::Client::new();

        // Query pending approvals
        let query = serde_json::json!({"pending_approvals": {"limit": 10}});
        let query_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            serde_json::to_string(&query).unwrap().as_bytes(),
        );

        let url = format!(
            "{}/cosmwasm/wasm/v1/contract/{}/smart/{}",
            terra_lcd_url(),
            bridge_address,
            query_b64
        );

        match client.get(&url).send().await {
            Ok(resp) => {
                let json: serde_json::Value = resp.json().await.unwrap();
                if let Some(data) = json.get("data") {
                    if let Some(approvals) = data.get("approvals") {
                        let count = approvals.as_array().map(|a| a.len()).unwrap_or(0);
                        println!("Found {} pending approvals", count);
                    }
                } else if let Some(error) = json.get("message") {
                    println!("Query error: {}", error);
                }
            }
            Err(e) => {
                println!("Request failed: {}", e);
            }
        }
    }
}

// ============================================================================
// Hash Computation Tests (unit tests, no infrastructure needed)
// ============================================================================

mod hash {
    /// Test withdraw hash computation format
    #[test]
    fn test_bytes32_hex_format() {
        let bytes: [u8; 32] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20,
        ];

        let hex = format!("0x{}", hex::encode(bytes));
        assert_eq!(
            hex.len(),
            66,
            "bytes32 hex should be 66 chars with 0x prefix"
        );
        assert!(hex.starts_with("0x"), "Should have 0x prefix");
    }

    /// Test keccak256 hash computation
    #[test]
    fn test_keccak256_computation() {
        use tiny_keccak::{Hasher, Keccak};

        let mut hasher = Keccak::v256();
        let mut output = [0u8; 32];

        hasher.update(b"test");
        hasher.finalize(&mut output);

        // Known keccak256("test") result
        let expected = "9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
        let result = hex::encode(output);

        assert_eq!(result, expected, "keccak256('test') mismatch");
    }
}

use base64::Engine;
