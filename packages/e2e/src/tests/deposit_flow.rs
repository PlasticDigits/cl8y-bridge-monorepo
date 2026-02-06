//! Deposit flow E2E tests

use crate::{E2eConfig, TestResult};
use alloy::primitives::{keccak256, Address, B256, U256};
use std::time::{Duration, Instant};

use super::helpers::{
    approve_erc20, check_evm_connection, check_terra_connection, encode_terra_address,
    execute_deposit, get_erc20_balance, get_terra_chain_key, query_deposit_nonce,
};

macro_rules! or_fail {
    ($expr:expr, $name:ident, $start:ident, $msg:expr) => {
        match $expr {
            Ok(v) => v,
            Err(e) => return TestResult::fail($name, format!("{}: {}", $msg, e), $start.elapsed()),
        }
    };
}

/// Test native deposit from EVM to Terra
pub async fn test_native_deposit_evm_to_terra(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "native_deposit_evm_to_terra";

    or_fail!(
        check_evm_connection(&config.evm.rpc_url).await,
        name,
        start,
        "EVM connectivity"
    );
    or_fail!(
        check_terra_connection(&config.terra.lcd_url).await,
        name,
        start,
        "Terra connectivity"
    );

    let nonce_before = or_fail!(query_deposit_nonce(config).await, name, start, "Get nonce");
    let terra_chain_key = or_fail!(
        get_terra_chain_key(config).await,
        name,
        start,
        "Get Terra chain key"
    );
    let dest_account = encode_terra_address(&config.test_accounts.terra_address);

    let deposit_amount = 1_000_000_000_000_000u128;
    let deposit_native_selector = hex::encode(&keccak256(b"depositNative(bytes4,bytes32)")[0..4]);
    // bytes4 is ABI-encoded left-aligned in 32 bytes (right-padded with zeros)
    let chain_id_padded = format!("{}{}", hex::encode(terra_chain_key), "0".repeat(56));
    let call_data = format!(
        "0x{}{}{}",
        deposit_native_selector,
        chain_id_padded,
        hex::encode(dest_account)
    );

    let client = reqwest::Client::new();
    let body: serde_json::Value = match client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", config.evm.contracts.bridge),
                "value": format!("0x{:x}", deposit_amount),
                "data": call_data,
                "gas": "0x200000"
            }],
            "id": 1
        }))
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(r) => match r.json().await {
            Ok(b) => b,
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to parse response: {}", e),
                    start.elapsed(),
                )
            }
        },
        Err(e) => {
            return TestResult::fail(name, format!("Transaction failed: {}", e), start.elapsed())
        }
    };

    if let Some(error) = body.get("error") {
        return TestResult::fail(name, format!("Deposit failed: {}", error), start.elapsed());
    }

    if body["result"].as_str().is_none() {
        return TestResult::fail(name, "No tx hash", start.elapsed());
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    let nonce_after = or_fail!(
        query_deposit_nonce(config).await,
        name,
        start,
        "Get nonce after"
    );

    if nonce_after <= nonce_before {
        return TestResult::fail(
            name,
            format!(
                "Nonce did not increment: {} -> {}",
                nonce_before, nonce_after
            ),
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test native deposit from Terra to EVM
pub async fn test_native_deposit_terra_to_evm(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "native_deposit_terra_to_evm";

    or_fail!(
        check_terra_connection(&config.terra.lcd_url).await,
        name,
        start,
        "Terra connectivity"
    );
    or_fail!(
        check_evm_connection(&config.evm.rpc_url).await,
        name,
        start,
        "EVM connectivity"
    );

    let terra_bridge = match config
        .terra
        .bridge_address
        .as_ref()
        .filter(|a| !a.is_empty())
    {
        Some(b) => b,
        None => return TestResult::skip(name, "Terra bridge not configured"),
    };

    use crate::terra::TerraClient;
    let terra_client = TerraClient::new(&config.terra);
    let denom = "uluna";
    let balance_before = or_fail!(
        terra_client
            .get_balance(&config.test_accounts.terra_address, denom)
            .await,
        name,
        start,
        "Get balance"
    );

    let lock_amount = 1_000_000u128;
    if balance_before < lock_amount {
        return TestResult::fail(
            name,
            format!("Insufficient balance: {} < {}", balance_before, lock_amount),
            start.elapsed(),
        );
    }

    let evm_addr_hex = hex::encode(config.test_accounts.evm_address.as_slice());
    let evm_recipient = format!("{:0>64}", evm_addr_hex);
    let tx_hash = or_fail!(
        terra_client
            .lock_tokens(
                terra_bridge,
                config.evm.chain_id,
                &evm_recipient,
                lock_amount,
                denom
            )
            .await,
        name,
        start,
        "Lock"
    );

    let result = or_fail!(
        terra_client
            .wait_for_tx(&tx_hash, Duration::from_secs(60))
            .await,
        name,
        start,
        "Tx confirmation"
    );
    if !result.success {
        return TestResult::fail(
            name,
            format!("Lock tx failed: {}", result.raw_log),
            start.elapsed(),
        );
    }

    let balance_after = or_fail!(
        terra_client
            .get_balance(&config.test_accounts.terra_address, denom)
            .await,
        name,
        start,
        "Get balance after"
    );
    if balance_before - balance_after < lock_amount {
        return TestResult::fail(
            name,
            format!(
                "Balance decrease insufficient: {} -> {}",
                balance_before, balance_after
            ),
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test ERC20 lock deposit
pub async fn test_erc20_lock_deposit(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "erc20_lock_deposit";

    or_fail!(
        check_evm_connection(&config.evm.rpc_url).await,
        name,
        start,
        "EVM connectivity"
    );
    or_fail!(
        check_terra_connection(&config.terra.lcd_url).await,
        name,
        start,
        "Terra connectivity"
    );

    let test_token = config.evm.contracts.test_token;
    if test_token == Address::ZERO {
        return TestResult::skip(name, "Test token not configured");
    }

    let balance_before = or_fail!(
        get_erc20_balance(config, test_token, config.test_accounts.evm_address).await,
        name,
        start,
        "Get balance"
    );
    let deposit_amount = 1_000_000u128;
    if balance_before < U256::from(deposit_amount) {
        return TestResult::fail(
            name,
            format!(
                "Insufficient balance: {} < {}",
                balance_before, deposit_amount
            ),
            start.elapsed(),
        );
    }

    let nonce_before = or_fail!(query_deposit_nonce(config).await, name, start, "Get nonce");
    let terra_chain_key = or_fail!(
        get_terra_chain_key(config).await,
        name,
        start,
        "Get Terra chain key"
    );
    let dest_account = encode_terra_address(&config.test_accounts.terra_address);

    or_fail!(
        approve_erc20(
            config,
            test_token,
            config.evm.contracts.lock_unlock,
            deposit_amount
        )
        .await,
        name,
        start,
        "Approval"
    );
    or_fail!(
        execute_deposit(
            config,
            test_token,
            deposit_amount,
            terra_chain_key,
            dest_account
        )
        .await,
        name,
        start,
        "Deposit"
    );

    tokio::time::sleep(Duration::from_secs(2)).await;
    let nonce_after = or_fail!(
        query_deposit_nonce(config).await,
        name,
        start,
        "Get nonce after"
    );
    if nonce_after <= nonce_before {
        return TestResult::fail(
            name,
            format!(
                "Nonce did not increment: {} -> {}",
                nonce_before, nonce_after
            ),
            start.elapsed(),
        );
    }

    let balance_after = or_fail!(
        get_erc20_balance(config, test_token, config.test_accounts.evm_address).await,
        name,
        start,
        "Get balance after"
    );
    if balance_before - balance_after < U256::from(deposit_amount) {
        return TestResult::fail(
            name,
            format!(
                "Balance decrease insufficient: {} -> {}",
                balance_before, balance_after
            ),
            start.elapsed(),
        );
    }

    TestResult::pass(name, start.elapsed())
}

/// Test deposit events correctness
pub async fn test_deposit_events_correctness(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "deposit_events_correctness";

    or_fail!(
        check_evm_connection(&config.evm.rpc_url).await,
        name,
        start,
        "EVM connectivity"
    );

    let test_token = config.evm.contracts.test_token;
    if test_token == Address::ZERO {
        return TestResult::skip(name, "Test token not configured");
    }

    let balance_before = or_fail!(
        get_erc20_balance(config, test_token, config.test_accounts.evm_address).await,
        name,
        start,
        "Get balance"
    );
    let deposit_amount = 1_000_000u128;
    if balance_before < U256::from(deposit_amount) {
        return TestResult::fail(
            name,
            format!(
                "Insufficient balance: {} < {}",
                balance_before, deposit_amount
            ),
            start.elapsed(),
        );
    }

    let nonce_before = or_fail!(query_deposit_nonce(config).await, name, start, "Get nonce");
    let terra_chain_key = or_fail!(
        get_terra_chain_key(config).await,
        name,
        start,
        "Get Terra chain key"
    );
    let dest_account = encode_terra_address(&config.test_accounts.terra_address);

    or_fail!(
        approve_erc20(
            config,
            test_token,
            config.evm.contracts.lock_unlock,
            deposit_amount
        )
        .await,
        name,
        start,
        "Approval"
    );
    let tx_hash = or_fail!(
        execute_deposit(
            config,
            test_token,
            deposit_amount,
            terra_chain_key,
            dest_account
        )
        .await,
        name,
        start,
        "Deposit"
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    let deposit_event_topic = format!(
        "0x{}",
        hex::encode(keccak256(
            b"Deposit(bytes4,bytes32,bytes32,address,uint256,uint64,uint256)"
        ))
    );
    let client = reqwest::Client::new();
    let event_body: serde_json::Value = match client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getLogs",
            "params": [{
                "fromBlock": "latest",
                "toBlock": "latest",
                "address": format!("{}", config.evm.contracts.bridge),
                "topics": [deposit_event_topic]
            }],
            "id": 1
        }))
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(r) => match r.json().await {
            Ok(b) => b,
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to parse events: {}", e),
                    start.elapsed(),
                )
            }
        },
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Failed to query events: {}", e),
                start.elapsed(),
            )
        }
    };

    let logs = match event_body["result"].as_array() {
        Some(l) if !l.is_empty() => l,
        _ => return TestResult::fail(name, "No deposit events found", start.elapsed()),
    };

    for log in logs.iter().rev() {
        let log_tx_hash = log["transactionHash"]
            .as_str()
            .and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
            .filter(|b| b.len() == 32)
            .map(|b| B256::from_slice(&b));

        if log_tx_hash != Some(tx_hash) {
            continue;
        }

        let topics = match log["topics"].as_array() {
            Some(t) if t.len() >= 3 => t,
            _ => return TestResult::fail(name, "Invalid topics", start.elapsed()),
        };

        let dest_account_from_topic =
            match hex::decode(topics[2].as_str().unwrap_or("").trim_start_matches("0x")) {
                Ok(b) => b,
                Err(e) => {
                    return TestResult::fail(
                        name,
                        format!("Failed to decode destAccount: {}", e),
                        start.elapsed(),
                    )
                }
            };
        if dest_account_from_topic != dest_account {
            return TestResult::fail(name, "destAccount mismatch", start.elapsed());
        }

        let data = log["data"].as_str().unwrap_or("").trim_start_matches("0x");
        let data_bytes = match hex::decode(data) {
            Ok(b) if b.len() >= 160 => b,
            Ok(_) => return TestResult::fail(name, "Data too short", start.elapsed()),
            Err(e) => {
                return TestResult::fail(
                    name,
                    format!("Failed to decode data: {}", e),
                    start.elapsed(),
                )
            }
        };

        let token = Address::from_slice(&data_bytes[44..64]);
        let amount = U256::from_be_slice(&data_bytes[64..96]);
        let nonce = u64::from_be_bytes(data_bytes[88..96].try_into().unwrap_or([0u8; 8]));
        let src_account = B256::from_slice(&data_bytes[0..32]);

        if nonce != nonce_before + 1 {
            return TestResult::fail(
                name,
                format!(
                    "Nonce mismatch: expected {}, got {}",
                    nonce_before + 1,
                    nonce
                ),
                start.elapsed(),
            );
        }
        if token != test_token {
            return TestResult::fail(
                name,
                format!("Token mismatch: {} != {}", token, test_token),
                start.elapsed(),
            );
        }
        if amount > U256::from(deposit_amount) {
            return TestResult::fail(
                name,
                format!("Amount exceeds deposit: {} > {}", amount, deposit_amount),
                start.elapsed(),
            );
        }

        let mut expected_src_account = [0u8; 32];
        expected_src_account[12..32].copy_from_slice(config.test_accounts.evm_address.as_slice());
        if src_account.as_slice() != expected_src_account {
            return TestResult::fail(name, "srcAccount mismatch", start.elapsed());
        }

        return TestResult::pass(name, start.elapsed());
    }

    TestResult::fail(
        name,
        format!("Event not found for tx 0x{}", hex::encode(tx_hash)),
        start.elapsed(),
    )
}
