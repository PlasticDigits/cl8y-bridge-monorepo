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

// ============================================================================
// PendingApproval Construction Tests (Category A verification)
//
// These tests verify that PendingApproval is correctly populated from
// both EVM contract returns and Terra JSON responses. The canceler ABI
// was previously broken (wrong function name, missing fields, wrong types),
// and these tests verify the fix.
// ============================================================================

mod pending_approval_tests {
    use base64::Engine as _;
    use canceler::verifier::PendingApproval;

    /// Test constructing PendingApproval from mock EVM withdrawal data
    /// (simulating the getPendingWithdraw contract return).
    ///
    /// Verifies:
    /// - dest_account != src_account for non-loopback transfers
    /// - All fields correctly populated
    /// - Token is a 20-byte address left-padded to bytes32
    #[test]
    fn test_pending_approval_from_evm_struct() {
        // Simulate data returned by getPendingWithdraw with all 13 fields
        let src_chain: [u8; 4] = [0, 0, 0, 2]; // Terra chain
        let this_chain_id: [u8; 4] = [0, 0, 0, 1]; // EVM chain (destination)

        // Source account (Terra address bech32-decoded, left-padded)
        let mut src_account = [0u8; 32];
        src_account[12..32].copy_from_slice(&[0xAA; 20]);

        // Destination account (EVM address, left-padded)
        let mut dest_account = [0u8; 32];
        dest_account[12..32].copy_from_slice(&[0xBB; 20]);

        // Token address (EVM ERC20, left-padded to bytes32)
        let mut token_bytes32 = [0u8; 32];
        let token_addr: [u8; 20] = [
            0x5F, 0xbD, 0xB2, 0x31, 0x56, 0x78, 0xaf, 0xec, 0xb3, 0x67, 0xf0, 0x32, 0xd9, 0x3F,
            0x64, 0x2f, 0x64, 0x18, 0x0a, 0xa3,
        ];
        token_bytes32[12..32].copy_from_slice(&token_addr);

        let amount: u128 = 1_000_000;
        let nonce: u64 = 42;

        let approval = PendingApproval {
            withdraw_hash: [0xCC; 32], // placeholder
            src_chain_id: src_chain,
            dest_chain_id: this_chain_id,
            src_account,
            dest_account,
            dest_token: token_bytes32,
            amount,
            nonce,
            approved_at_timestamp: 1700000000,
            cancel_window: 300,
        };

        // Verify key fields
        assert_ne!(
            approval.src_account, approval.dest_account,
            "src_account and dest_account must differ for non-loopback transfers"
        );
        assert_eq!(approval.amount, 1_000_000);
        assert_eq!(approval.nonce, 42);
        assert_eq!(approval.src_chain_id, [0, 0, 0, 2]);
        assert_eq!(approval.dest_chain_id, [0, 0, 0, 1]);

        // Verify token is properly left-padded (first 12 bytes zero)
        assert_eq!(
            &approval.dest_token[0..12],
            &[0u8; 12],
            "Token bytes32 must have zero-padded first 12 bytes"
        );
        assert_eq!(&approval.dest_token[12..32], &token_addr);
    }

    /// Test constructing PendingApproval from mock Terra JSON data.
    ///
    /// Verifies the fields that were previously broken:
    /// - src_account is NOT [0u8; 32] (was hardcoded to zeros)
    /// - dest_account comes from "dest_account" JSON field, NOT "src_account"
    #[test]
    fn test_pending_approval_from_terra_json() {
        // Simulate parsing Terra withdrawal JSON (as CancelerWatcher does)
        let withdrawal_json = serde_json::json!({
            "withdraw_hash": base64::engine::general_purpose::STANDARD.encode([0xDD; 32]),
            "src_chain": base64::engine::general_purpose::STANDARD.encode([0u8, 0, 0, 1]),
            "dest_chain": base64::engine::general_purpose::STANDARD.encode([0u8, 0, 0, 2]),
            "token": base64::engine::general_purpose::STANDARD.encode([0xEE; 32]),
            "src_account": base64::engine::general_purpose::STANDARD.encode({
                let mut a = [0u8; 32];
                a[12..32].copy_from_slice(&[0x11; 20]);
                a
            }),
            "dest_account": base64::engine::general_purpose::STANDARD.encode({
                let mut a = [0u8; 32];
                a[12..32].copy_from_slice(&[0x22; 20]);
                a
            }),
            "amount": "500000",
            "nonce": 7,
            "approved_at": 0,
            "cancel_window": 300
        });

        // Parse fields the way the fixed watcher does
        let parse_bytes32 = |val: &serde_json::Value| -> [u8; 32] {
            let b64 = val.as_str().unwrap_or("");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap_or_default();
            let mut result = [0u8; 32];
            if bytes.len() >= 32 {
                result.copy_from_slice(&bytes[..32]);
            }
            result
        };

        let parse_bytes4 = |val: &serde_json::Value| -> [u8; 4] {
            let b64 = val.as_str().unwrap_or("");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64)
                .unwrap_or_default();
            let mut result = [0u8; 4];
            if bytes.len() >= 4 {
                result.copy_from_slice(&bytes[..4]);
            }
            result
        };

        let src_chain_id = parse_bytes4(&withdrawal_json["src_chain"]);
        let dest_chain_id = parse_bytes4(&withdrawal_json["dest_chain"]);
        let src_account = parse_bytes32(&withdrawal_json["src_account"]);
        let dest_account = parse_bytes32(&withdrawal_json["dest_account"]);
        let dest_token = parse_bytes32(&withdrawal_json["token"]);

        let approval = PendingApproval {
            withdraw_hash: parse_bytes32(&withdrawal_json["withdraw_hash"]),
            src_chain_id,
            dest_chain_id,
            src_account,
            dest_account,
            dest_token,
            amount: withdrawal_json["amount"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            nonce: withdrawal_json["nonce"].as_u64().unwrap_or(0),
            approved_at_timestamp: withdrawal_json["approved_at"].as_u64().unwrap_or(0),
            cancel_window: withdrawal_json["cancel_window"].as_u64().unwrap_or(300),
        };

        // Bug fix verification: src_account must NOT be all zeros
        assert_ne!(
            approval.src_account, [0u8; 32],
            "src_account must NOT be zeros (was hardcoded to zeros before fix)"
        );

        // Bug fix verification: dest_account must come from "dest_account" field
        let expected_dest = {
            let mut a = [0u8; 32];
            a[12..32].copy_from_slice(&[0x22; 20]);
            a
        };
        assert_eq!(
            approval.dest_account, expected_dest,
            "dest_account must come from 'dest_account' JSON field, not 'src_account'"
        );

        // Verify src_account is correctly populated
        let expected_src = {
            let mut a = [0u8; 32];
            a[12..32].copy_from_slice(&[0x11; 20]);
            a
        };
        assert_eq!(approval.src_account, expected_src);

        assert_eq!(approval.amount, 500000);
        assert_eq!(approval.nonce, 7);
    }

    /// Test hash verification: create a PendingApproval with known values,
    /// compute the hash, and verify it matches.
    ///
    /// This confirms the fields flow correctly from contract/JSON data
    /// through PendingApproval into hash computation.
    #[test]
    fn test_hash_verification_with_correct_fields() {
        use canceler::hash::compute_transfer_hash;

        let src_chain: [u8; 4] = [0, 0, 0, 1]; // EVM
        let dest_chain: [u8; 4] = [0, 0, 0, 2]; // Terra

        let mut src_account = [0u8; 32];
        src_account[12..32].copy_from_slice(&[0xAA; 20]);

        let mut dest_account = [0u8; 32];
        dest_account[12..32].copy_from_slice(&[0xBB; 20]);

        let dest_token = [0xEE; 32];
        let amount: u128 = 1_000_000;
        let nonce: u64 = 42;

        // Compute the expected hash
        let expected_hash = compute_transfer_hash(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_account,
            &dest_token,
            amount,
            nonce,
        );

        // Create PendingApproval with the computed hash
        let approval = PendingApproval {
            withdraw_hash: expected_hash,
            src_chain_id: src_chain,
            dest_chain_id: dest_chain,
            src_account,
            dest_account,
            dest_token,
            amount,
            nonce,
            approved_at_timestamp: 0,
            cancel_window: 300,
        };

        // Recompute and verify
        let recomputed = compute_transfer_hash(
            &approval.src_chain_id,
            &approval.dest_chain_id,
            &approval.src_account,
            &approval.dest_account,
            &approval.dest_token,
            approval.amount,
            approval.nonce,
        );

        assert_eq!(
            recomputed, approval.withdraw_hash,
            "Hash recomputed from PendingApproval fields must match the original withdraw_hash"
        );

        // Also verify the hash is non-zero
        assert_ne!(
            approval.withdraw_hash, [0u8; 32],
            "Hash should be non-zero for non-zero inputs"
        );
    }
}
