//! Integration tests for CL8Y Bridge Contract V2 using cw-multi-test.
//!
//! These tests verify the V2 withdrawal flow, rate limiting, deposit flows, and RecoverAsset.

use cosmwasm_std::{coins, to_json_binary, Addr, Binary, Uint128};
use cw20::{Cw20Coin, Cw20ExecuteMsg};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::msg::ReceiveMsg;
use bridge::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, LockedBalanceResponse, PendingWithdrawResponse,
    QueryMsg, ThisChainIdResponse, WithdrawDelayResponse,
};

// ============================================================================
// Test Setup
// ============================================================================

fn contract_bridge() -> Box<dyn cw_multi_test::Contract<cosmwasm_std::Empty>> {
    let contract = ContractWrapper::new(
        bridge::contract::execute,
        bridge::contract::instantiate,
        bridge::contract::query,
    );
    Box::new(contract)
}

fn contract_cw20() -> Box<dyn cw_multi_test::Contract<cosmwasm_std::Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn setup() -> (App, Addr, Addr, Addr) {
    let mut app = App::default();

    // Create test accounts
    let admin = Addr::unchecked("terra1admin");
    let operator = Addr::unchecked("terra1operator");
    let user = Addr::unchecked("terra1user");
    let canceler = Addr::unchecked("terra1canceler");

    // Fund accounts
    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(10_000_000_000, "uluna"))
            .unwrap();
        router
            .bank
            .init_balance(storage, &operator, coins(10_000_000_000, "uluna"))
            .unwrap();
        router
            .bank
            .init_balance(storage, &user, coins(10_000_000_000, "uluna"))
            .unwrap();
    });

    // Store contract code
    let code_id = app.store_code(contract_bridge());

    // Instantiate contract
    let contract_addr = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                admin: admin.to_string(),
                operators: vec![operator.to_string()],
                min_signatures: 1,
                min_bridge_amount: Uint128::from(1000u128),
                max_bridge_amount: Uint128::from(1_000_000_000_000u128),
                fee_bps: 30,
                fee_collector: admin.to_string(),
                this_chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
            &[],
            "cl8y-bridge",
            Some(admin.to_string()),
        )
        .unwrap();

    // Add canceler
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddCanceler {
            address: canceler.to_string(),
        },
        &[],
    )
    .unwrap();

    // Register supported chain (BSC)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "bsc_56".to_string(),
            chain_id: Binary::from(vec![0, 0, 0, 2]),
        },
        &[],
    )
    .unwrap();

    // Add supported token (uluna) — LockUnlock mode (default)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddToken {
            token: "uluna".to_string(),
            is_native: true,
            token_type: None,
            evm_token_address: "0x000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    // Set withdraw delay to 60 seconds
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetWithdrawDelay { delay_seconds: 60 },
        &[],
    )
    .unwrap();

    // Register incoming token mapping (BSC → uluna)
    let src_token_bytes = bridge::hash::keccak256(b"uluna");
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: Binary::from(vec![0, 0, 0, 2]),
            src_token: Binary::from(src_token_bytes.to_vec()),
            local_token: "uluna".to_string(),
            src_decimals: 18,
        },
        &[],
    )
    .unwrap();

    (app, contract_addr, operator, user)
}

/// Create a 4-byte source chain ID matching the registered BSC chain
fn create_test_src_chain() -> Binary {
    // Must match the chain_id registered in setup() for BSC
    Binary::from(vec![0, 0, 0, 2])
}

/// Create a test source account (EVM depositor, 32 bytes)
fn create_test_src_account() -> Binary {
    let mut account = [0u8; 32];
    // Simulated EVM address (20 bytes, right-aligned)
    account[12..32].copy_from_slice(&[0xAB; 20]);
    Binary::from(account.to_vec())
}

/// Helper: user submits a withdrawal, returns the withdraw hash as Binary.
fn submit_withdraw(
    app: &mut App,
    user: &Addr,
    contract_addr: &Addr,
    token: &str,
    amount: u128,
    nonce: u64,
    operator_gas: u128,
) -> Binary {
    let src_chain = create_test_src_chain();
    let src_account = create_test_src_account();

    let funds = if operator_gas > 0 {
        coins(operator_gas, "uluna")
    } else {
        vec![]
    };

    let res = app
        .execute_contract(
            user.clone(),
            contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain,
                src_account,
                token: token.to_string(),
                recipient: user.to_string(),
                amount: Uint128::from(amount),
                nonce,
            },
            &funds,
        )
        .unwrap();

    // Extract withdraw hash from attributes
    let withdraw_hash_hex = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "withdraw_hash")
        .map(|a| a.value.clone())
        .expect("withdraw_hash attribute not found");

    let hash_bytes = hex::decode(&withdraw_hash_hex[2..]).unwrap(); // Remove 0x prefix
    Binary::from(hash_bytes)
}

// ============================================================================
// Contract Instantiation Tests
// ============================================================================

#[test]
fn test_instantiate() {
    let (app, contract_addr, _operator, _user) = setup();

    let config: ConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::Config {})
        .unwrap();

    assert!(!config.paused);
    assert_eq!(config.min_signatures, 1);
    assert_eq!(config.fee_bps, 30);
}

// ============================================================================
// WithdrawSubmit Tests
// ============================================================================

#[test]
fn test_withdraw_submit_creates_pending() {
    let (mut app, contract_addr, _operator, user) = setup();

    // Amount in source chain (EVM) decimals: 1 token = 1e18
    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        100,
    );

    // Query the pending withdrawal
    let pending: PendingWithdrawResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::PendingWithdraw { withdraw_hash })
        .unwrap();

    assert!(pending.exists);
    assert!(!pending.approved);
    assert!(!pending.cancelled);
    assert!(!pending.executed);
    assert_eq!(pending.amount, Uint128::from(1_000_000_000_000_000_000u128));
    assert_eq!(pending.operator_funds.len(), 1);
    assert_eq!(pending.operator_funds[0].denom, "uluna");
    assert_eq!(pending.operator_funds[0].amount, Uint128::from(100u128));
}

#[test]
fn test_withdraw_submit_rejects_duplicate() {
    let (mut app, contract_addr, _operator, user) = setup();

    // First submission succeeds
    submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Second submission with same params should fail
    let src_chain = create_test_src_chain();
    let src_account = create_test_src_account();

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain,
            src_account,
            token: "uluna".to_string(),
            recipient: user.to_string(),
            amount: Uint128::from(1_000_000_000_000_000_000u128),
            nonce: 0,
        },
        &[],
    );

    assert!(res.is_err());
}

// ============================================================================
// WithdrawApprove Tests
// ============================================================================

#[test]
fn test_withdraw_approve_requires_operator() {
    let (mut app, contract_addr, _operator, user) = setup();

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Non-operator tries to approve — should fail
    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_withdraw_approve_sets_approved() {
    let (mut app, contract_addr, operator, user) = setup();

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Operator approves
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Query — should be approved
    let pending: PendingWithdrawResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();

    assert!(pending.approved);
    assert!(!pending.cancelled);
    assert!(!pending.executed);
}

// ============================================================================
// WithdrawCancel Tests
// ============================================================================

#[test]
fn test_withdraw_cancel_within_window() {
    let (mut app, contract_addr, operator, user) = setup();
    let canceler = Addr::unchecked("terra1canceler");

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Operator approves (starts cancel window)
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Canceler cancels within window (immediately after approval)
    let res = app.execute_contract(
        canceler.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_ok());

    // Query — should be cancelled (stored for auditing)
    let pending: PendingWithdrawResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::PendingWithdraw { withdraw_hash })
        .unwrap();

    assert!(pending.cancelled);
}

#[test]
fn test_withdraw_cancel_after_window_fails() {
    let (mut app, contract_addr, operator, user) = setup();
    let canceler = Addr::unchecked("terra1canceler");

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Operator approves
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Advance past cancel window (5 min + 1 sec)
    app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Cancel should fail — window expired
    let res = app.execute_contract(
        canceler.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("expired"),
        "Expected cancel window expired error, got: {}",
        err_str
    );
}

#[test]
fn test_withdraw_cancel_requires_canceler_role() {
    let (mut app, contract_addr, operator, user) = setup();

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Operator approves
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Random user tries to cancel — should fail
    let random = Addr::unchecked("terra1random");
    let res = app.execute_contract(
        random,
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_withdraw_cancel_operator_rejected() {
    let (mut app, contract_addr, operator, user) = setup();

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Operator tries to cancel — should fail (only canceler can cancel)
    let res = app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("canceler") || err_str.contains("Canceler"),
        "Expected canceler-only error, got: {}",
        err_str
    );
}

#[test]
fn test_withdraw_cancel_admin_rejected() {
    let (mut app, contract_addr, operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Admin tries to cancel — should fail (only canceler can cancel)
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
}

// ============================================================================
// WithdrawUncancel Tests
// ============================================================================

#[test]
fn test_withdraw_uncancel_restores_and_resets_window() {
    let (mut app, contract_addr, operator, user) = setup();
    let canceler = Addr::unchecked("terra1canceler");

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Approve → Cancel → Uncancel
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        canceler.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Advance time by 30 seconds
    app.update_block(|block| {
        block.time = block.time.plus_seconds(30);
    });

    // Operator uncancels
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawUncancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Query — should be un-cancelled with new cancel window
    let pending: PendingWithdrawResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();

    assert!(!pending.cancelled);
    assert!(pending.approved);
    // Cancel window should have been reset — remaining should be close to 60
    // (the withdraw delay configured in setup() is 60 seconds)
    assert!(
        pending.cancel_window_remaining > 50,
        "Expected cancel window near 60s, got: {}",
        pending.cancel_window_remaining
    );
}

// ============================================================================
// WithdrawExecuteUnlock Tests
// ============================================================================

#[test]
fn test_execute_unlock_before_window_fails() {
    let (mut app, contract_addr, operator, user) = setup();

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Approve
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Try to execute immediately (within cancel window) — should fail
    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("Cancel window still active"),
        "Expected cancel window active error, got: {}",
        err_str
    );
}

#[test]
fn test_execute_unlock_after_window_insufficient_liquidity() {
    let (mut app, contract_addr, operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Approve
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Set rate limit to avoid BankQuery::Supply (not supported in cw-multi-test)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetRateLimit {
            token: "uluna".to_string(),
            max_per_transaction: Uint128::from(10_000_000_000u128),
            max_per_period: Uint128::from(100_000_000_000u128),
        },
        &[],
    )
    .unwrap();

    // Advance past cancel window
    app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Execute — should fail with insufficient liquidity (no locked balance)
    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("liquidity") || err_str.contains("Insufficient"),
        "Expected liquidity error, got: {}",
        err_str
    );
}

#[test]
fn test_cancelled_withdraw_cannot_execute() {
    let (mut app, contract_addr, operator, user) = setup();
    let canceler = Addr::unchecked("terra1canceler");

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        1_000_000_000_000_000_000,
        0,
        0,
    );

    // Approve → Cancel
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        canceler.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawCancel {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    // Advance past cancel window
    app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Try to execute — should fail because cancelled
    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("cancelled") || err_str.contains("Cancelled"),
        "Expected cancelled error, got: {}",
        err_str
    );
}

// ============================================================================
// Rate Limit Tests
// ============================================================================

#[test]
fn test_rate_limit_configuration() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Set rate limit
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetRateLimit {
            token: "uluna".to_string(),
            max_per_transaction: Uint128::from(5_000_000u128),
            max_per_period: Uint128::from(100_000_000u128),
        },
        &[],
    )
    .unwrap();

    // Query rate limit
    let rate_limit: Option<bridge::msg::RateLimitResponse> = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::RateLimit {
                token: "uluna".to_string(),
            },
        )
        .unwrap();

    assert!(rate_limit.is_some());
    let rl = rate_limit.unwrap();
    assert_eq!(rl.max_per_transaction, Uint128::from(5_000_000u128));
    assert_eq!(rl.max_per_period, Uint128::from(100_000_000u128));
}

// ============================================================================
// Lock (Deposit) Tests
// ============================================================================

#[test]
fn test_lock_stores_deposit_hash() {
    let (mut app, contract_addr, _operator, user) = setup();

    // 32-byte dest_account (EVM address, left-padded)
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ]);

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    );

    assert!(res.is_ok(), "DepositNative failed: {:?}", res.err());

    // Check that deposit_hash attribute is emitted
    let res = res.unwrap();
    let deposit_hash = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "deposit_hash");

    assert!(deposit_hash.is_some(), "deposit_hash attribute not found");

    // Query deposit by nonce
    let deposit: Option<bridge::msg::DepositInfoResponse> = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::DepositByNonce { nonce: 0 })
        .unwrap();

    assert!(deposit.is_some());
    let d = deposit.unwrap();
    assert_eq!(d.nonce, 0);
}

// ============================================================================
// Configuration Query Tests
// ============================================================================

#[test]
fn test_this_chain_id_query() {
    let (app, contract_addr, _operator, _user) = setup();

    let resp: ThisChainIdResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::ThisChainId {})
        .unwrap();

    assert_eq!(resp.chain_id.as_slice(), &[0, 0, 0, 1]);
}

// ============================================================================
// Withdraw Delay Query Tests
// ============================================================================

#[test]
fn test_withdraw_delay_query() {
    let (app, contract_addr, _operator, _user) = setup();

    let delay: WithdrawDelayResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::WithdrawDelay {})
        .unwrap();

    assert_eq!(delay.delay_seconds, 60);
}

// ============================================================================
// Deposit Flow Tests
// ============================================================================

#[test]
fn test_deposit_native_min_limit_enforced() {
    let (mut app, contract_addr, _operator, user) = setup();

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(500, "uluna"), // Below min_bridge_amount of 1000
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("minimum") || err_str.contains("Minimum"),
        "Expected min amount error, got: {}",
        err_str
    );
}

#[test]
fn test_deposit_native_max_limit_enforced() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Lower max to test enforcement
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateLimits {
            min_bridge_amount: Some(Uint128::from(1000u128)),
            max_bridge_amount: Some(Uint128::from(1_000_000u128)),
        },
        &[],
    )
    .unwrap();

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(2_000_000, "uluna"), // Above max_bridge_amount of 1_000_000
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("maximum") || err_str.contains("Maximum"),
        "Expected max amount error, got: {}",
        err_str
    );
}

// ============================================================================
// Deposit Flow – Fee Edge Cases (Full Flow)
// ============================================================================

#[test]
fn test_deposit_native_full_flow_fee_zero_bps() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);

    // Set custom fee 0 bps
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 0,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    )
    .unwrap();

    let locked: LockedBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::LockedBalance {
                token: "uluna".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        locked.amount,
        Uint128::from(1_000_000u128),
        "0 bps: full amount locked"
    );
}

#[test]
fn test_deposit_native_full_flow_fee_max_bps() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 100,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    )
    .unwrap();

    let locked: LockedBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::LockedBalance {
                token: "uluna".to_string(),
            },
        )
        .unwrap();
    assert_eq!(
        locked.amount,
        Uint128::from(990_000u128),
        "100 bps: 1% fee, 99% locked"
    );
}

#[test]
fn test_deposit_cw20_lock_full_flow() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let cw20_code_id = app.store_code(contract_cw20());
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            admin.clone(),
            &cw20_base::msg::InstantiateMsg {
                name: "Test Token".to_string(),
                symbol: "TST".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: user.to_string(),
                    amount: Uint128::from(10_000_000u128),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "cw20-test",
            None,
        )
        .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetAllowedCw20CodeIds {
            code_ids: vec![cw20_code_id],
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddToken {
            token: cw20_addr.to_string(),
            is_native: false,
            token_type: Some("lock_unlock".to_string()),
            evm_token_address: "0x0000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetTokenDestination {
            token: cw20_addr.to_string(),
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_token: "0000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            dest_decimals: 18,
        },
        &[],
    )
    .unwrap();

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);
    let deposit_msg = ReceiveMsg::DepositCw20Lock {
        dest_chain: Binary::from(vec![0, 0, 0, 2]),
        dest_account: Binary::from(dest_account_bytes.to_vec()),
    };

    app.execute_contract(
        user.clone(),
        cw20_addr.clone(),
        &Cw20ExecuteMsg::Send {
            contract: contract_addr.to_string(),
            amount: Uint128::from(1_000_000u128),
            msg: to_json_binary(&deposit_msg).unwrap(),
        },
        &[],
    )
    .unwrap();

    let locked: LockedBalanceResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::LockedBalance {
                token: cw20_addr.to_string(),
            },
        )
        .unwrap();
    assert!(locked.amount > Uint128::zero(), "CW20 lock: tokens locked");
}

#[test]
fn test_deposit_cw20_burn_full_flow() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let cw20_code_id = app.store_code(contract_cw20());
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            admin.clone(),
            &cw20_base::msg::InstantiateMsg {
                name: "Mint Token".to_string(),
                symbol: "MNT".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: user.to_string(),
                    amount: Uint128::from(10_000_000u128),
                }],
                mint: Some(cw20::MinterResponse {
                    minter: admin.to_string(),
                    cap: None,
                }),
                marketing: None,
            },
            &[],
            "cw20-mint",
            None,
        )
        .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetAllowedCw20CodeIds {
            code_ids: vec![cw20_code_id],
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddToken {
            token: cw20_addr.to_string(),
            is_native: false,
            token_type: Some("mint_burn".to_string()),
            evm_token_address: "0x0000000000000000000000000000000000000000000000000000000000000002"
                .to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetTokenDestination {
            token: cw20_addr.to_string(),
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_token: "0000000000000000000000000000000000000000000000000000000000000002"
                .to_string(),
            dest_decimals: 18,
        },
        &[],
    )
    .unwrap();

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);
    let deposit_msg = ReceiveMsg::DepositCw20MintableBurn {
        dest_chain: Binary::from(vec![0, 0, 0, 2]),
        dest_account: Binary::from(dest_account_bytes.to_vec()),
    };

    app.execute_contract(
        user.clone(),
        cw20_addr.clone(),
        &Cw20ExecuteMsg::Send {
            contract: contract_addr.to_string(),
            amount: Uint128::from(1_000_000u128),
            msg: to_json_binary(&deposit_msg).unwrap(),
        },
        &[],
    )
    .unwrap();

    let token_info: cw20::TokenInfoResponse = app
        .wrap()
        .query_wasm_smart(&cw20_addr, &cw20::Cw20QueryMsg::TokenInfo {})
        .unwrap();
    assert!(
        token_info.total_supply < Uint128::from(10_000_000u128),
        "CW20 burn: supply decreased"
    );
}

// ============================================================================
// RecoverAsset Tests
// ============================================================================

#[test]
fn test_recover_asset_requires_pause() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Recover without pause should fail
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RecoverAsset {
            asset: common::AssetInfo::native("uluna"),
            amount: Uint128::from(1000u128),
            recipient: admin.to_string(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("paused") || err_str.contains("Recovery"),
        "Expected recovery requires pause, got: {}",
        err_str
    );
}

#[test]
fn test_recover_asset_admin_only() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Pause {},
        &[],
    )
    .unwrap();

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RecoverAsset {
            asset: common::AssetInfo::native("uluna"),
            amount: Uint128::from(1000u128),
            recipient: user.to_string(),
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_recover_asset_native_success() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Deposit native tokens (contract receives them)
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);

    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(5_000_000, "uluna"),
    )
    .unwrap();

    // Pause and recover
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Pause {},
        &[],
    )
    .unwrap();

    let recover_amount = Uint128::from(1_000_000u128);
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RecoverAsset {
            asset: common::AssetInfo::native("uluna"),
            amount: recover_amount,
            recipient: admin.to_string(),
        },
        &[],
    );

    assert!(res.is_ok(), "RecoverAsset failed: {:?}", res.err());

    // Verify admin received the recovered tokens
    let admin_balance = app.wrap().query_balance(&admin, "uluna").unwrap();
    assert!(
        admin_balance.amount >= recover_amount,
        "Admin should have received recovered amount"
    );
}

#[test]
fn test_recover_asset_exceeds_balance_fails() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);

    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Pause {},
        &[],
    )
    .unwrap();

    // Recover more than contract holds (2M > 1M) — Bank::Send submessage fails
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RecoverAsset {
            asset: common::AssetInfo::native("uluna"),
            amount: Uint128::from(2_000_000u128),
            recipient: admin.to_string(),
        },
        &[],
    );

    assert!(
        res.is_err(),
        "RecoverAsset should fail when amount exceeds balance"
    );
}

#[test]
fn test_recover_asset_cw20_success() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let cw20_code_id = app.store_code(contract_cw20());
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            admin.clone(),
            &cw20_base::msg::InstantiateMsg {
                name: "Recover Token".to_string(),
                symbol: "RCV".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: user.to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "cw20-recover",
            None,
        )
        .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetAllowedCw20CodeIds {
            code_ids: vec![cw20_code_id],
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddToken {
            token: cw20_addr.to_string(),
            is_native: false,
            token_type: Some("lock_unlock".to_string()),
            evm_token_address: "0x0000000000000000000000000000000000000000000000000000000000000003"
                .to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetTokenDestination {
            token: cw20_addr.to_string(),
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_token: "0000000000000000000000000000000000000000000000000000000000000003"
                .to_string(),
            dest_decimals: 18,
        },
        &[],
    )
    .unwrap();

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);
    let deposit_msg = ReceiveMsg::DepositCw20Lock {
        dest_chain: Binary::from(vec![0, 0, 0, 2]),
        dest_account: Binary::from(dest_account_bytes.to_vec()),
    };

    app.execute_contract(
        user.clone(),
        cw20_addr.clone(),
        &Cw20ExecuteMsg::Send {
            contract: contract_addr.to_string(),
            amount: Uint128::from(100_000u128),
            msg: to_json_binary(&deposit_msg).unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Pause {},
        &[],
    )
    .unwrap();

    let recover_amount = Uint128::from(50_000u128);
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RecoverAsset {
            asset: common::AssetInfo::cw20(cw20_addr.clone()),
            amount: recover_amount,
            recipient: admin.to_string(),
        },
        &[],
    );

    assert!(res.is_ok(), "RecoverAsset CW20 failed: {:?}", res.err());

    let admin_cw20 = app
        .wrap()
        .query_wasm_smart::<cw20::BalanceResponse>(
            &cw20_addr,
            &cw20::Cw20QueryMsg::Balance {
                address: admin.to_string(),
            },
        )
        .unwrap();
    assert!(
        admin_cw20.balance >= recover_amount,
        "Admin should have received recovered CW20"
    );
}

#[test]
fn test_recover_asset_cw20_exceeds_balance_fails() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let cw20_code_id = app.store_code(contract_cw20());
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            admin.clone(),
            &cw20_base::msg::InstantiateMsg {
                name: "Exceed Token".to_string(),
                symbol: "EXC".to_string(),
                decimals: 6,
                initial_balances: vec![Cw20Coin {
                    address: user.to_string(),
                    amount: Uint128::from(1_000_000u128),
                }],
                mint: None,
                marketing: None,
            },
            &[],
            "cw20-exceed",
            None,
        )
        .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetAllowedCw20CodeIds {
            code_ids: vec![cw20_code_id],
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddToken {
            token: cw20_addr.to_string(),
            is_native: false,
            token_type: Some("lock_unlock".to_string()),
            evm_token_address: "0x0000000000000000000000000000000000000000000000000000000000000004"
                .to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetTokenDestination {
            token: cw20_addr.to_string(),
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_token: "0000000000000000000000000000000000000000000000000000000000000004"
                .to_string(),
            dest_decimals: 18,
        },
        &[],
    )
    .unwrap();

    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);
    let deposit_msg = ReceiveMsg::DepositCw20Lock {
        dest_chain: Binary::from(vec![0, 0, 0, 2]),
        dest_account: Binary::from(dest_account_bytes.to_vec()),
    };

    app.execute_contract(
        user.clone(),
        cw20_addr.clone(),
        &Cw20ExecuteMsg::Send {
            contract: contract_addr.to_string(),
            amount: Uint128::from(100_000u128),
            msg: to_json_binary(&deposit_msg).unwrap(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::Pause {},
        &[],
    )
    .unwrap();

    // Recover more than bridge holds (200k > 100k) — Cw20 Transfer submessage fails
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RecoverAsset {
            asset: common::AssetInfo::cw20(cw20_addr.clone()),
            amount: Uint128::from(200_000u128),
            recipient: admin.to_string(),
        },
        &[],
    );

    assert!(
        res.is_err(),
        "RecoverAsset should fail when CW20 amount exceeds balance"
    );
}

// ============================================================================
// Rate Limit Enforcement Tests
// ============================================================================

#[test]
fn test_rate_limit_per_transaction_exceeded() {
    let (mut app, contract_addr, operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Deposit liquidity
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);
    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(10_000_000, "uluna"),
    )
    .unwrap();

    // Set strict per-transaction limit: 1_000_000 (6 decimals)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetRateLimit {
            token: "uluna".to_string(),
            max_per_transaction: Uint128::from(1_000_000u128),
            max_per_period: Uint128::from(100_000_000u128),
        },
        &[],
    )
    .unwrap();

    // Submit withdraw for 2e18 (18 decimals) → 2e6 (6 decimals) payout, exceeds 1e6 limit
    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        2_000_000_000_000_000_000u128, // 2e18
        0,
        0,
    );

    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("Rate") || err_str.contains("rate") || err_str.contains("limit"),
        "Expected rate limit error, got: {}",
        err_str
    );
}

#[test]
fn test_rate_limit_per_period_exceeded() {
    let (mut app, contract_addr, operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Deposit liquidity for two withdrawals
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[0xAB; 20]);
    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_account_bytes.to_vec()),
        },
        &coins(10_000_000, "uluna"),
    )
    .unwrap();

    // Set period limit: 3_000_000 total per 24h
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetRateLimit {
            token: "uluna".to_string(),
            max_per_transaction: Uint128::zero(), // No per-tx limit
            max_per_period: Uint128::from(3_000_000u128),
        },
        &[],
    )
    .unwrap();

    // First withdrawal: 2e18 → 2e6 payout
    let hash1 = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        2_000_000_000_000_000_000u128,
        0,
        0,
    );
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: hash1.clone(),
        },
        &[],
    )
    .unwrap();
    app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });
    app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: hash1.clone(),
        },
        &[],
    )
    .unwrap();

    // Second withdrawal: another 2e6, total 4e6 > 3e6 period limit
    let hash2 = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna",
        2_000_000_000_000_000_000u128,
        1,
        0,
    );
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: hash2.clone(),
        },
        &[],
    )
    .unwrap();
    app.update_block(|block| {
        block.time = block.time.plus_seconds(301); // Past cancel window, same rate limit period
    });

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: hash2.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("Rate") || err_str.contains("rate") || err_str.contains("limit"),
        "Expected rate limit error, got: {}",
        err_str
    );
}

// ============================================================================
// WithdrawExecuteMint Tests
// ============================================================================

#[test]
fn test_execute_mint_rejects_lock_unlock_token() {
    let (mut app, contract_addr, operator, user) = setup();

    let withdraw_hash = submit_withdraw(
        &mut app,
        &user,
        &contract_addr,
        "uluna", // LockUnlock token
        1_000_000_000_000_000_000,
        0,
        0,
    );

    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    )
    .unwrap();

    app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Call ExecuteMint with LockUnlock token — should fail (wrong token type)
    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteMint {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("mint_burn") || err_str.contains("token type"),
        "Expected wrong token type for ExecuteMint, got: {}",
        err_str
    );
}
