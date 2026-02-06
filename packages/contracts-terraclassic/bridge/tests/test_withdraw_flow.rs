//! Comprehensive V2 Withdrawal Flow Integration Tests.
//!
//! Tests the complete user-initiated withdrawal lifecycle:
//! - Submit → Approve → Execute (unlock with funds)
//! - Cancel/Uncancel cycles
//! - Decimal normalization (18→6)
//! - Edge cases (double execute, unapproved execute, paused bridge)
//! - Operator gas tip mechanics

use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::msg::{
    ExecuteMsg, InstantiateMsg, LockedBalanceResponse, PendingWithdrawResponse,
    PendingWithdrawalsResponse, QueryMsg, StatsResponse,
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

struct TestEnv {
    app: App,
    contract_addr: Addr,
    admin: Addr,
    operator: Addr,
    user: Addr,
    canceler: Addr,
}

fn setup() -> TestEnv {
    let mut app = App::default();
    let admin = Addr::unchecked("terra1admin");
    let operator = Addr::unchecked("terra1operator");
    let user = Addr::unchecked("terra1user");
    let canceler = Addr::unchecked("terra1canceler");

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

    let code_id = app.store_code(contract_bridge());
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

    // Register chain (BSC)
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

    // Add uluna token (18 EVM decimals, 6 Terra decimals)
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

    // Set withdraw delay
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetWithdrawDelay { delay_seconds: 60 },
        &[],
    )
    .unwrap();

    TestEnv {
        app,
        contract_addr,
        admin,
        operator,
        user,
        canceler,
    }
}

fn create_src_chain() -> Binary {
    Binary::from(56u32.to_be_bytes().to_vec())
}

fn create_src_account() -> Binary {
    let mut account = [0u8; 32];
    account[12..32].copy_from_slice(&[0xAB; 20]);
    Binary::from(account.to_vec())
}

fn make_dest_account() -> Binary {
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(&[0xDE; 20]);
    Binary::from(bytes.to_vec())
}

/// Submit a withdraw and return the hash
fn submit_withdraw(
    env: &mut TestEnv,
    token: &str,
    amount: u128,
    nonce: u64,
    operator_gas: u128,
) -> Binary {
    let funds = if operator_gas > 0 {
        coins(operator_gas, "uluna")
    } else {
        vec![]
    };

    let res = env
        .app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: create_src_chain(),
                src_account: create_src_account(),
                token: token.to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(amount),
                nonce,
            },
            &funds,
        )
        .unwrap();

    let withdraw_hash_hex = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "withdraw_hash")
        .map(|a| a.value.clone())
        .expect("withdraw_hash attribute not found");

    let hash_bytes = hex::decode(&withdraw_hash_hex[2..]).unwrap();
    Binary::from(hash_bytes)
}

/// Deposit native tokens to build up locked liquidity
fn deposit_to_build_liquidity(env: &mut TestEnv, amount: u128) {
    let dest_account = make_dest_account();
    // Register an EVM chain if needed
    let _ = env.app.execute_contract(
        env.admin.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
            chain_id: Binary::from(vec![0, 0, 0, 3]),
        },
        &[],
    );

    env.app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::DepositNative {
                dest_chain: Binary::from(vec![0, 0, 0, 3]),
                dest_account,
            },
            &coins(amount, "uluna"),
        )
        .unwrap();
}

// ============================================================================
// Full Cycle Tests — Submit → Approve → Execute
// ============================================================================

#[test]
fn test_full_withdraw_cycle_with_liquidity() {
    let mut env = setup();

    // First: deposit to build liquidity (user deposits 5M uluna)
    deposit_to_build_liquidity(&mut env, 5_000_000);

    // Check locked balance increased (minus fee)
    let locked: LockedBalanceResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::LockedBalance {
                token: "uluna".to_string(),
            },
        )
        .unwrap();
    assert!(locked.amount > Uint128::zero());

    // Submit withdraw: 1e18 in source decimals (EVM), normalizes to 1e6 in Terra
    let withdraw_hash = submit_withdraw(
        &mut env,
        "uluna",
        1_000_000_000_000_000_000, // 1e18 (EVM decimals)
        0,
        0,
    );

    // Approve
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Advance past cancel window (5 min + 1s)
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Execute unlock
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_ok(), "Execute unlock failed: {:?}", res.err());

    // Verify withdrawal is marked as executed
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert!(pending.executed);

    // Verify stats updated
    let stats: StatsResponse = env
        .app
        .wrap()
        .query_wasm_smart(&env.contract_addr, &QueryMsg::Stats {})
        .unwrap();
    assert_eq!(stats.total_incoming_txs, 1);
}

#[test]
fn test_withdraw_decimal_normalization_18_to_6() {
    let mut env = setup();

    // Deposit 10M uluna for liquidity
    deposit_to_build_liquidity(&mut env, 10_000_000);

    // Submit 2e18 in EVM decimals → should normalize to 2e6 (2_000_000) uluna
    let withdraw_hash = submit_withdraw(
        &mut env,
        "uluna",
        2_000_000_000_000_000_000, // 2e18
        1,
        0,
    );

    // Check pending withdraw stored amount (in source decimals)
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(pending.amount, Uint128::from(2_000_000_000_000_000_000u128));
    assert_eq!(pending.src_decimals, 18);
    assert_eq!(pending.dest_decimals, 6);

    // Approve + wait + execute
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );
    assert!(res.is_ok(), "Execute failed: {:?}", res.err());

    // Check the payout amount attribute (should be normalized to 2_000_000)
    let res = res.unwrap();
    let amount_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "amount")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(amount_attr, "2000000"); // 2e18 / 1e12 = 2e6
}

// ============================================================================
// Cancel/Uncancel Cycle Tests
// ============================================================================

#[test]
fn test_cancel_uncancel_then_execute() {
    let mut env = setup();
    deposit_to_build_liquidity(&mut env, 5_000_000);

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 2, 0);

    // Approve
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Cancel
    env.app
        .execute_contract(
            env.canceler.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawCancel {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Verify cancelled
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert!(pending.cancelled);

    // Uncancel (resets window)
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawUncancel {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Wait past new cancel window
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Execute should succeed now
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );
    assert!(
        res.is_ok(),
        "Execute after uncancel failed: {:?}",
        res.err()
    );
}

#[test]
fn test_uncancel_resets_cancel_window() {
    let mut env = setup();

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 3, 0);

    // Approve
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Wait 200s (within window)
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(200);
    });

    // Cancel
    env.app
        .execute_contract(
            env.canceler.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawCancel {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Wait 50s more
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(50);
    });

    // Uncancel
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawUncancel {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Cancel window should be ~300s from now
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert!(
        pending.cancel_window_remaining >= 295,
        "Window should be near 300s, got {}",
        pending.cancel_window_remaining
    );
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_double_execute_rejected() {
    let mut env = setup();
    deposit_to_build_liquidity(&mut env, 10_000_000);

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 4, 0);

    // Approve + wait
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // First execute succeeds
    env.app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawExecuteUnlock {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Second execute should fail
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );
    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("already executed") || err_str.contains("Already"),
        "Expected already executed error, got: {}",
        err_str
    );
}

#[test]
fn test_execute_without_approval_rejected() {
    let mut env = setup();

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 5, 0);

    // Skip approval, wait, try to execute
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("not approved") || err_str.contains("Not approved"),
        "Expected not approved error, got: {}",
        err_str
    );
}

#[test]
fn test_execute_nonexistent_hash_rejected() {
    let mut env = setup();

    let fake_hash = Binary::from(vec![0xDE; 32]);

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: fake_hash,
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("not found") || err_str.contains("Not found"),
        "Expected not found error, got: {}",
        err_str
    );
}

#[test]
fn test_execute_while_paused_rejected() {
    let mut env = setup();
    deposit_to_build_liquidity(&mut env, 5_000_000);

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 6, 0);

    // Approve + wait
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Pause the bridge
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::Pause {},
            &[],
        )
        .unwrap();

    // Execute should fail because paused
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("paused") || err_str.contains("Paused"),
        "Expected paused error, got: {}",
        err_str
    );
}

#[test]
fn test_submit_while_paused_rejected() {
    let mut env = setup();

    // Pause the bridge
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::Pause {},
            &[],
        )
        .unwrap();

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: create_src_chain(),
            src_account: create_src_account(),
            token: "uluna".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 0,
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_submit_zero_amount_rejected() {
    let mut env = setup();

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: create_src_chain(),
            src_account: create_src_account(),
            token: "uluna".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::zero(),
            nonce: 0,
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_submit_unsupported_token_rejected() {
    let mut env = setup();

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: create_src_chain(),
            src_account: create_src_account(),
            token: "uatom".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 0,
        },
        &[],
    );

    assert!(res.is_err());
}

// ============================================================================
// Operator Gas Tip Tests
// ============================================================================

#[test]
fn test_submit_with_operator_gas_tip() {
    let mut env = setup();

    let withdraw_hash = submit_withdraw(
        &mut env,
        "uluna",
        1_000_000_000_000_000_000,
        7,
        500_000, // 0.5 LUNA tip
    );

    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();

    assert_eq!(pending.operator_gas, Uint128::from(500_000u128));
}

#[test]
fn test_operator_gas_transferred_on_approve() {
    let mut env = setup();

    let withdraw_hash = submit_withdraw(
        &mut env,
        "uluna",
        1_000_000_000_000_000_000,
        8,
        1_000_000, // 1 LUNA tip
    );

    // Get operator balance before
    let operator_balance_before = env
        .app
        .wrap()
        .query_balance(&env.operator, "uluna")
        .unwrap();

    // Approve (operator gets the gas tip)
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    let operator_balance_after = env
        .app
        .wrap()
        .query_balance(&env.operator, "uluna")
        .unwrap();

    // Operator should have received the 1M uluna tip
    assert_eq!(
        operator_balance_after.amount - operator_balance_before.amount,
        Uint128::from(1_000_000u128)
    );
}

// ============================================================================
// Anyone Can Execute After Window Tests
// ============================================================================

#[test]
fn test_anyone_can_execute_after_window() {
    let mut env = setup();
    deposit_to_build_liquidity(&mut env, 10_000_000);

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 9, 0);

    // Approve + wait
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // A random third party executes (not the user, not operator)
    let random = Addr::unchecked("terra1random");
    let res = env.app.execute_contract(
        random.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(
        res.is_ok(),
        "Anyone should be able to execute after window: {:?}",
        res.err()
    );
}

// ============================================================================
// Wrong Token Type Tests
// ============================================================================

#[test]
fn test_execute_unlock_wrong_token_type_rejected() {
    let mut env = setup();

    // Add a MintBurn token
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::AddToken {
                token: "mintable_token".to_string(),
                is_native: false,
                token_type: Some("mint_burn".to_string()),
                evm_token_address:
                    "0x0000000000000000000000001111111111111111111111111111111111111111"
                        .to_string(),
                terra_decimals: 6,
                evm_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Submit withdraw for the MintBurn token
    let res = env
        .app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: create_src_chain(),
                src_account: create_src_account(),
                token: "mintable_token".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(1_000_000u128),
                nonce: 10,
            },
            &[],
        )
        .unwrap();

    let withdraw_hash_hex = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "withdraw_hash")
        .map(|a| a.value.clone())
        .unwrap();
    let hash_bytes = hex::decode(&withdraw_hash_hex[2..]).unwrap();
    let withdraw_hash = Binary::from(hash_bytes);

    // Approve + wait
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(301);
    });

    // Try to ExecuteUnlock (should fail — token is MintBurn)
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawExecuteUnlock {
            withdraw_hash: withdraw_hash.clone(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("token type") || err_str.contains("lock_unlock"),
        "Expected wrong token type error, got: {}",
        err_str
    );
}

// ============================================================================
// Cancel Window Query Tests
// ============================================================================

#[test]
fn test_cancel_window_countdown() {
    let mut env = setup();

    let withdraw_hash = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 11, 0);

    // Before approval: cancel_window_remaining should be 0
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(pending.cancel_window_remaining, 0);

    // Approve
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Right after approval: should be ~300s
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(pending.cancel_window_remaining, 300);

    // Advance 100s
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(100);
    });

    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(pending.cancel_window_remaining, 200);

    // Advance past window
    env.app.update_block(|block| {
        block.time = block.time.plus_seconds(201);
    });

    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();
    assert_eq!(pending.cancel_window_remaining, 0);
}

// ============================================================================
// PendingWithdrawals Pagination Tests
// ============================================================================

#[test]
fn test_pending_withdrawals_empty() {
    let env = setup();

    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert!(result.withdrawals.is_empty());
}

#[test]
fn test_pending_withdrawals_returns_all() {
    let mut env = setup();

    // Submit 3 withdrawals with different nonces
    let hash1 = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 100, 0);
    let hash2 = submit_withdraw(&mut env, "uluna", 2_000_000_000_000_000_000, 101, 0);
    let hash3 = submit_withdraw(&mut env, "uluna", 3_000_000_000_000_000_000, 102, 0);

    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(result.withdrawals.len(), 3);

    // Verify all hashes are present
    let returned_hashes: Vec<Binary> = result
        .withdrawals
        .iter()
        .map(|w| w.withdraw_hash.clone())
        .collect();
    assert!(returned_hashes.contains(&hash1));
    assert!(returned_hashes.contains(&hash2));
    assert!(returned_hashes.contains(&hash3));

    // Verify nonces are present
    let returned_nonces: Vec<u64> = result.withdrawals.iter().map(|w| w.nonce).collect();
    assert!(returned_nonces.contains(&100));
    assert!(returned_nonces.contains(&101));
    assert!(returned_nonces.contains(&102));
}

#[test]
fn test_pending_withdrawals_pagination_limit() {
    let mut env = setup();

    // Submit 5 withdrawals
    for i in 200..205u64 {
        submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, i, 0);
    }

    // Query with limit=2
    let page1: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: Some(2),
            },
        )
        .unwrap();

    assert_eq!(page1.withdrawals.len(), 2);

    // Use the last hash as cursor for page 2
    let cursor = page1.withdrawals.last().unwrap().withdraw_hash.clone();
    let page2: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: Some(cursor.clone()),
                limit: Some(2),
            },
        )
        .unwrap();

    assert_eq!(page2.withdrawals.len(), 2);

    // Page 3 should have 1 remaining
    let cursor2 = page2.withdrawals.last().unwrap().withdraw_hash.clone();
    let page3: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: Some(cursor2),
                limit: Some(2),
            },
        )
        .unwrap();

    assert_eq!(page3.withdrawals.len(), 1);

    // Verify all 5 are unique
    let mut all_hashes: Vec<Binary> = Vec::new();
    for w in page1
        .withdrawals
        .iter()
        .chain(page2.withdrawals.iter())
        .chain(page3.withdrawals.iter())
    {
        assert!(
            !all_hashes.contains(&w.withdraw_hash),
            "Duplicate hash found"
        );
        all_hashes.push(w.withdraw_hash.clone());
    }
    assert_eq!(all_hashes.len(), 5);
}

#[test]
fn test_pending_withdrawals_shows_approved_and_cancelled() {
    let mut env = setup();

    // Submit 3 withdrawals
    let hash1 = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 300, 0);
    let _hash2 = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 301, 0);
    let hash3 = submit_withdraw(&mut env, "uluna", 1_000_000_000_000_000_000, 302, 0);

    // Approve hash1
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: hash1.clone(),
            },
            &[],
        )
        .unwrap();

    // Approve then cancel hash3
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: hash3.clone(),
            },
            &[],
        )
        .unwrap();
    env.app
        .execute_contract(
            env.canceler.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawCancel {
                withdraw_hash: hash3.clone(),
            },
            &[],
        )
        .unwrap();

    // Query all
    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(result.withdrawals.len(), 3);

    // Find each by nonce and verify status
    let w300 = result.withdrawals.iter().find(|w| w.nonce == 300).unwrap();
    assert!(w300.approved, "nonce 300 should be approved");
    assert!(!w300.cancelled, "nonce 300 should not be cancelled");
    assert!(!w300.executed, "nonce 300 should not be executed");
    assert!(
        w300.cancel_window_remaining > 0,
        "nonce 300 should have cancel window remaining"
    );

    let w301 = result.withdrawals.iter().find(|w| w.nonce == 301).unwrap();
    assert!(!w301.approved, "nonce 301 should not be approved");
    assert!(!w301.cancelled, "nonce 301 should not be cancelled");

    let w302 = result.withdrawals.iter().find(|w| w.nonce == 302).unwrap();
    assert!(w302.approved, "nonce 302 should be approved");
    assert!(w302.cancelled, "nonce 302 should be cancelled");
    assert_eq!(
        w302.cancel_window_remaining, 0,
        "cancelled entries should have 0 window remaining"
    );
}

#[test]
fn test_pending_withdrawals_max_limit_capped() {
    let env = setup();

    // Query with an absurdly high limit
    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: Some(9999),
            },
        )
        .unwrap();

    // Should succeed (just returns empty since no withdrawals)
    assert!(result.withdrawals.is_empty());
}

#[test]
fn test_pending_withdrawals_fields_match_single_query() {
    let mut env = setup();

    let withdraw_hash = submit_withdraw(
        &mut env,
        "uluna",
        1_000_000_000_000_000_000,
        400,
        500_000, // operator gas tip
    );

    // Approve it
    env.app
        .execute_contract(
            env.operator.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawApprove {
                withdraw_hash: withdraw_hash.clone(),
            },
            &[],
        )
        .unwrap();

    // Query via single-item query
    let single: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: withdraw_hash.clone(),
            },
        )
        .unwrap();

    // Query via list query
    let list: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(list.withdrawals.len(), 1);
    let entry = &list.withdrawals[0];

    // Compare all fields
    assert_eq!(entry.withdraw_hash, withdraw_hash);
    assert_eq!(entry.src_chain, single.src_chain);
    assert_eq!(entry.src_account, single.src_account);
    assert_eq!(entry.dest_account, single.dest_account);
    assert_eq!(entry.token, single.token);
    assert_eq!(entry.recipient, single.recipient);
    assert_eq!(entry.amount, single.amount);
    assert_eq!(entry.nonce, single.nonce);
    assert_eq!(entry.src_decimals, single.src_decimals);
    assert_eq!(entry.dest_decimals, single.dest_decimals);
    assert_eq!(entry.operator_gas, single.operator_gas);
    assert_eq!(entry.submitted_at, single.submitted_at);
    assert_eq!(entry.approved_at, single.approved_at);
    assert_eq!(entry.approved, single.approved);
    assert_eq!(entry.cancelled, single.cancelled);
    assert_eq!(entry.executed, single.executed);
    assert_eq!(
        entry.cancel_window_remaining,
        single.cancel_window_remaining
    );
}
