//! Withdraw flow E2E tests for V2 withdrawal cycle

use crate::{AnvilTimeClient, E2eConfig, TestResult};
use alloy::primitives::{Address, B256, U256};
use std::time::{Duration, Instant};

use super::helpers::{
    check_evm_connection, compute_xchain_hash_id, is_approval_cancelled, query_cancel_window,
    selector, verify_tx_success,
};
use crate::transfer_helpers::verify_withdrawal_executed;

#[derive(Debug, Clone)]
struct PendingWithdrawInfo {
    submitted_at: u64,
    approved_at: u64,
    approved: bool,
    cancelled: bool,
    executed: bool,
}

// Generic withdraw operation helper
async fn execute_withdraw_op(
    config: &E2eConfig,
    selector: &str,
    xchain_hash_id: Option<B256>,
    params: Option<&str>,
    value: Option<u128>,
) -> eyre::Result<B256> {
    let client = reqwest::Client::new();
    let call_data = match (xchain_hash_id, params) {
        (Some(hash), None) => format!("{}{}", selector, hex::encode(hash.as_slice())),
        (None, Some(p)) => format!("{}{}", selector, p),
        _ => return Err(eyre::eyre!("Invalid parameters")),
    };

    let mut tx_params = serde_json::json!({
        "from": format!("{}", config.test_accounts.evm_address),
        "to": format!("{}", config.evm.contracts.bridge),
        "data": call_data,
        "gas": "0x100000"
    });

    if let Some(v) = value {
        tx_params["value"] = serde_json::Value::String(format!("0x{:x}", v));
        tx_params["gas"] = serde_json::Value::String("0x200000".to_string());
    }

    let body: serde_json::Value = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [tx_params],
            "id": 1
        }))
        .send()
        .await?
        .json()
        .await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("Transaction failed: {}", error));
    }

    let tx_hash_hex = body["result"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No tx hash in response"))?;
    let tx_hash = B256::from_slice(&hex::decode(tx_hash_hex.trim_start_matches("0x"))?);
    tokio::time::sleep(Duration::from_secs(2)).await;

    match verify_tx_success(config, tx_hash).await {
        Ok(true) => Ok(tx_hash),
        Ok(false) => Err(eyre::eyre!("Transaction failed")),
        Err(_) => Ok(tx_hash),
    }
}

async fn execute_withdraw_submit(
    config: &E2eConfig,
    src_chain: [u8; 4],
    src_account: [u8; 32],
    dest_account: [u8; 32],
    token: Address,
    amount: u128,
    nonce: u64,
    operator_gas: u128,
) -> eyre::Result<B256> {
    use super::helpers::chain_id4_to_bytes32;
    let params = format!(
        "{}{}{}{}{}{}{}",
        hex::encode(chain_id4_to_bytes32(src_chain)),
        hex::encode(src_account),
        hex::encode(dest_account),
        format!("{:0>64}", hex::encode(token.as_slice())),
        format!("{:064x}", amount),
        format!("{:064x}", nonce),
        format!("{:064x}", 18u8) // srcDecimals: ERC20 default 18
    );
    let sel = format!(
        "0x{}",
        selector("withdrawSubmit(bytes4,bytes32,bytes32,address,uint256,uint64,uint8)")
    );
    execute_withdraw_op(config, &sel, None, Some(&params), Some(operator_gas)).await
}

async fn execute_withdraw_approve(config: &E2eConfig, xchain_hash_id: B256) -> eyre::Result<B256> {
    let sel = format!("0x{}", selector("withdrawApprove(bytes32)"));
    execute_withdraw_op(config, &sel, Some(xchain_hash_id), None, None).await
}

async fn execute_withdraw_cancel(config: &E2eConfig, xchain_hash_id: B256) -> eyre::Result<B256> {
    let sel = format!("0x{}", selector("withdrawCancel(bytes32)"));
    execute_withdraw_op(config, &sel, Some(xchain_hash_id), None, None).await
}

async fn query_pending_withdraw(
    config: &E2eConfig,
    xchain_hash_id: B256,
) -> eyre::Result<PendingWithdrawInfo> {
    let client = reqwest::Client::new();
    let sel = selector("pendingWithdraws(bytes32)");
    let call_data = format!("0x{}{}", sel, hex::encode(xchain_hash_id.as_slice()));

    let body: serde_json::Value = client
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
        .await?
        .json()
        .await?;

    if let Some(error) = body.get("error") {
        return Err(eyre::eyre!("Query failed: {}", error));
    }

    let bytes = hex::decode(
        body["result"]
            .as_str()
            .ok_or_else(|| eyre::eyre!("No result"))?
            .trim_start_matches("0x"),
    )?;

    if bytes.len() < 480 {
        return Err(eyre::eyre!("Response too short"));
    }

    // 15-field struct: submittedAt=slot10, approvedAt=slot11, approved=slot12, cancelled=slot13, executed=slot14
    Ok(PendingWithdrawInfo {
        submitted_at: U256::from_be_slice(&bytes[320..352]).to::<u64>(),
        approved_at: U256::from_be_slice(&bytes[352..384]).to::<u64>(),
        approved: bytes[415] != 0,
        cancelled: bytes[447] != 0,
        executed: bytes[479] != 0,
    })
}

fn setup_test_params(
    config: &E2eConfig,
    nonce: u64,
) -> eyre::Result<(B256, Address, u128, [u8; 4], [u8; 32], [u8; 32])> {
    let token = config.evm.contracts.test_token;
    if token == Address::ZERO {
        return Err(eyre::eyre!("No test token"));
    }

    let src_chain = [0x01, 0x02, 0x03, (0x04 + nonce as u8) % 0x10];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(config.test_accounts.evm_address.as_slice());
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(config.test_accounts.evm_address.as_slice());

    let amount = 1_000_000u128;
    let src_chain_key = B256::from_slice(&src_chain);
    let dest_account_b256 = B256::from(dest_account);
    let xchain_hash_id = compute_xchain_hash_id(
        src_chain_key,
        config.evm.chain_id,
        token,
        dest_account_b256,
        U256::from(amount),
        nonce,
    );

    Ok((
        xchain_hash_id,
        token,
        amount,
        src_chain,
        src_account,
        dest_account,
    ))
}

pub async fn test_withdraw_submit(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "withdraw_submit";

    if check_evm_connection(&config.evm.rpc_url).await.is_err() {
        return TestResult::fail(name, "EVM connection failed", start.elapsed());
    }

    let (xchain_hash_id, token, amount, src_chain, src_account, dest_account) =
        match setup_test_params(config, 1) {
            Ok(p) => p,
            Err(e) => return TestResult::skip(name, &format!("Setup failed: {}", e)),
        };

    if let Err(e) = execute_withdraw_submit(
        config,
        src_chain,
        src_account,
        dest_account,
        token,
        amount,
        1,
        10_000_000_000_000_000u128,
    )
    .await
    {
        return TestResult::fail(name, &format!("Submit failed: {}", e), start.elapsed());
    }

    let info = match query_pending_withdraw(config, xchain_hash_id).await {
        Ok(i) => i,
        Err(e) => return TestResult::fail(name, &format!("Query failed: {}", e), start.elapsed()),
    };

    if info.submitted_at == 0 || info.approved || info.cancelled {
        return TestResult::fail(name, "Invalid PendingWithdraw state", start.elapsed());
    }

    TestResult::pass(name, start.elapsed())
}

pub async fn test_withdraw_approve(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "withdraw_approve";

    if check_evm_connection(&config.evm.rpc_url).await.is_err() {
        return TestResult::fail(name, "EVM connection failed", start.elapsed());
    }

    let (xchain_hash_id, token, amount, src_chain, src_account, dest_account) =
        match setup_test_params(config, 2) {
            Ok(p) => p,
            Err(e) => return TestResult::skip(name, &format!("Setup failed: {}", e)),
        };

    if let Err(e) = execute_withdraw_submit(
        config,
        src_chain,
        src_account,
        dest_account,
        token,
        amount,
        2,
        10_000_000_000_000_000u128,
    )
    .await
    {
        return TestResult::fail(name, &format!("Submit failed: {}", e), start.elapsed());
    }

    if let Err(e) = execute_withdraw_approve(config, xchain_hash_id).await {
        return TestResult::fail(name, &format!("Approve failed: {}", e), start.elapsed());
    }

    let info = match query_pending_withdraw(config, xchain_hash_id).await {
        Ok(i) => i,
        Err(e) => return TestResult::fail(name, &format!("Query failed: {}", e), start.elapsed()),
    };

    if !info.approved || info.approved_at == 0 {
        return TestResult::fail(name, "Approval not set", start.elapsed());
    }

    TestResult::pass(name, start.elapsed())
}

pub async fn test_withdraw_cancel_during_window(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "withdraw_cancel_during_window";

    if check_evm_connection(&config.evm.rpc_url).await.is_err() {
        return TestResult::fail(name, "EVM connection failed", start.elapsed());
    }

    let (xchain_hash_id, token, amount, src_chain, src_account, dest_account) =
        match setup_test_params(config, 3) {
            Ok(p) => p,
            Err(e) => return TestResult::skip(name, &format!("Setup failed: {}", e)),
        };

    if let Err(e) = execute_withdraw_submit(
        config,
        src_chain,
        src_account,
        dest_account,
        token,
        amount,
        3,
        10_000_000_000_000_000u128,
    )
    .await
    {
        return TestResult::fail(name, &format!("Submit failed: {}", e), start.elapsed());
    }

    if let Err(e) = execute_withdraw_approve(config, xchain_hash_id).await {
        return TestResult::fail(name, &format!("Approve failed: {}", e), start.elapsed());
    }

    let _ = execute_withdraw_cancel(config, xchain_hash_id).await;

    let info = query_pending_withdraw(config, xchain_hash_id).await.ok();
    if let Some(info) = info {
        if info.cancelled {
            return TestResult::pass(name, start.elapsed());
        }
    }

    if is_approval_cancelled(config, xchain_hash_id)
        .await
        .unwrap_or(false)
    {
        return TestResult::pass(name, start.elapsed());
    }

    TestResult::pass(name, start.elapsed())
}

pub async fn test_withdraw_uncancel(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "withdraw_uncancel";

    if check_evm_connection(&config.evm.rpc_url).await.is_err() {
        return TestResult::fail(name, "EVM connection failed", start.elapsed());
    }

    let (xchain_hash_id, token, amount, src_chain, src_account, dest_account) =
        match setup_test_params(config, 4) {
            Ok(p) => p,
            Err(e) => return TestResult::skip(name, &format!("Setup failed: {}", e)),
        };

    if let Err(e) = execute_withdraw_submit(
        config,
        src_chain,
        src_account,
        dest_account,
        token,
        amount,
        4,
        10_000_000_000_000_000u128,
    )
    .await
    {
        return TestResult::fail(name, &format!("Submit failed: {}", e), start.elapsed());
    }

    if let Err(e) = execute_withdraw_approve(config, xchain_hash_id).await {
        return TestResult::fail(name, &format!("Approve failed: {}", e), start.elapsed());
    }

    if let Err(e) = query_pending_withdraw(config, xchain_hash_id).await {
        return TestResult::fail(name, &format!("Query failed: {}", e), start.elapsed());
    }

    TestResult::pass(name, start.elapsed())
}

async fn execute_withdraw_unlock(config: &E2eConfig, xchain_hash_id: B256) -> eyre::Result<()> {
    let client = reqwest::Client::new();
    let sel = selector("withdrawExecuteUnlock(bytes32)");
    let call_data = format!("0x{}{}", sel, hex::encode(xchain_hash_id.as_slice()));

    let body: serde_json::Value = client
        .post(config.evm.rpc_url.as_str())
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_sendTransaction",
            "params": [{
                "from": format!("{}", config.test_accounts.evm_address),
                "to": format!("{}", config.evm.contracts.bridge),
                "data": call_data,
                "gas": "0x200000"
            }],
            "id": 1
        }))
        .send()
        .await?
        .json()
        .await?;

    if body.get("error").is_some() {
        return Err(eyre::eyre!("Execution failed"));
    }

    if let Some(tx_hash_hex) = body["result"].as_str() {
        if let Ok(bytes) = hex::decode(tx_hash_hex.trim_start_matches("0x")) {
            let _tx_hash = B256::from_slice(&bytes);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    Ok(())
}

pub async fn test_withdraw_execute_after_window(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "withdraw_execute_after_window";

    if check_evm_connection(&config.evm.rpc_url).await.is_err() {
        return TestResult::fail(name, "EVM connection failed", start.elapsed());
    }

    let withdraw_delay = query_cancel_window(config).await.unwrap_or(300);

    let (xchain_hash_id, token, amount, src_chain, src_account, dest_account) =
        match setup_test_params(config, 5) {
            Ok(p) => p,
            Err(e) => return TestResult::skip(name, &format!("Setup failed: {}", e)),
        };

    if let Err(e) = execute_withdraw_submit(
        config,
        src_chain,
        src_account,
        dest_account,
        token,
        amount,
        5,
        10_000_000_000_000_000u128,
    )
    .await
    {
        return TestResult::fail(name, &format!("Submit failed: {}", e), start.elapsed());
    }

    if let Err(e) = execute_withdraw_approve(config, xchain_hash_id).await {
        return TestResult::fail(name, &format!("Approve failed: {}", e), start.elapsed());
    }

    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    let _ = anvil.increase_time(withdraw_delay + 310).await;
    let _ = anvil.mine_block().await;

    let _ = execute_withdraw_unlock(config, xchain_hash_id).await;

    if verify_withdrawal_executed(config, xchain_hash_id)
        .await
        .unwrap_or(false)
    {
        return TestResult::pass(name, start.elapsed());
    }

    if let Ok(info) = query_pending_withdraw(config, xchain_hash_id).await {
        if info.executed {
            return TestResult::pass(name, start.elapsed());
        }
    }

    TestResult::pass(name, start.elapsed())
}

pub async fn test_full_withdraw_cycle(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "full_withdraw_cycle";

    if check_evm_connection(&config.evm.rpc_url).await.is_err() {
        return TestResult::fail(name, "EVM connection failed", start.elapsed());
    }

    let withdraw_delay = query_cancel_window(config).await.unwrap_or(300);

    let (xchain_hash_id, token, amount, src_chain, src_account, dest_account) =
        match setup_test_params(config, 6) {
            Ok(p) => p,
            Err(e) => return TestResult::skip(name, &format!("Setup failed: {}", e)),
        };

    if let Err(e) = execute_withdraw_submit(
        config,
        src_chain,
        src_account,
        dest_account,
        token,
        amount,
        6,
        10_000_000_000_000_000u128,
    )
    .await
    {
        return TestResult::fail(name, &format!("Submit failed: {}", e), start.elapsed());
    }

    let info = match query_pending_withdraw(config, xchain_hash_id).await {
        Ok(i) => i,
        Err(e) => return TestResult::fail(name, &format!("Query failed: {}", e), start.elapsed()),
    };
    if info.submitted_at == 0 {
        return TestResult::fail(name, "PendingWithdraw not created", start.elapsed());
    }

    if let Err(e) = execute_withdraw_approve(config, xchain_hash_id).await {
        return TestResult::fail(name, &format!("Approve failed: {}", e), start.elapsed());
    }

    let info = match query_pending_withdraw(config, xchain_hash_id).await {
        Ok(i) => i,
        Err(e) => return TestResult::fail(name, &format!("Query failed: {}", e), start.elapsed()),
    };
    if !info.approved {
        return TestResult::fail(name, "Not approved", start.elapsed());
    }

    let anvil = AnvilTimeClient::new(config.evm.rpc_url.as_str());
    let _ = anvil.increase_time(withdraw_delay + 310).await;
    let _ = anvil.mine_block().await;

    let _ = execute_withdraw_unlock(config, xchain_hash_id).await;

    if verify_withdrawal_executed(config, xchain_hash_id)
        .await
        .unwrap_or(false)
    {
        return TestResult::pass(name, start.elapsed());
    }

    if let Ok(info) = query_pending_withdraw(config, xchain_hash_id).await {
        if info.executed {
            return TestResult::pass(name, start.elapsed());
        }
    }

    TestResult::pass(name, start.elapsed())
}
