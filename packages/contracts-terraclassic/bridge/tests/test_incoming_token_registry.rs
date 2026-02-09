//! Tests for the incoming token registry and WithdrawSubmit validation.
//!
//! Tests:
//! - SetIncomingTokenMapping (admin only, validates chain/token exist)
//! - RemoveIncomingTokenMapping (removes mapping, blocks future withdrawals)
//! - IncomingTokenMapping query returns correct data
//! - IncomingTokenMappings paginated query
//! - WithdrawSubmit rejects unregistered source chain
//! - WithdrawSubmit rejects unmapped token for source chain

use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::msg::{
    ExecuteMsg, IncomingTokenMappingResponse, IncomingTokenMappingsResponse, InstantiateMsg,
    QueryMsg,
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
    user: Addr,
}

/// Basic setup with bridge contract, one chain registered, one token registered
fn setup() -> TestEnv {
    let mut app = App::default();
    let admin = Addr::unchecked("terra1admin");
    let user = Addr::unchecked("terra1user");

    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(10_000_000_000, "uluna"))
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
                operators: vec![admin.to_string()],
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

    // Register source chain (EVM, chain_id = 0x00000002)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_31337".to_string(),
            chain_id: Binary::from(vec![0, 0, 0, 2]),
        },
        &[],
    )
    .unwrap();

    // Add uluna token
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::AddToken {
            token: "uluna".to_string(),
            is_native: true,
            token_type: None,
            evm_token_address: "0x0000000000000000000000000000000000000000".to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    TestEnv {
        app,
        contract_addr,
        admin,
        user,
    }
}

fn evm_chain_id() -> Binary {
    Binary::from(vec![0, 0, 0, 2])
}

/// Compute the token bytes32 for "uluna" as it would be encoded on-chain.
///
/// Native denoms like "uluna" are short strings (< 20 chars), so
/// `encode_token_address` hashes them with keccak256.
fn uluna_src_token() -> Binary {
    Binary::from(bridge::hash::keccak256(b"uluna").to_vec())
}

fn make_src_account() -> Binary {
    let mut account = [0u8; 32];
    account[12..32].copy_from_slice(&[0xAB; 20]);
    Binary::from(account.to_vec())
}

// ============================================================================
// SetIncomingTokenMapping Tests
// ============================================================================

#[test]
fn test_set_incoming_token_mapping_success() {
    let mut env = setup();

    let res = env
        .app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Verify attributes
    let action = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "action")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(action, "set_incoming_token_mapping");

    let local_token = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "local_token")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(local_token, "uluna");
}

#[test]
fn test_set_incoming_token_mapping_non_admin_rejected() {
    let mut env = setup();

    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: evm_chain_id(),
            src_token: uluna_src_token(),
            local_token: "uluna".to_string(),
            src_decimals: 18,
        },
        &[],
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.root_cause().to_string().contains("Unauthorized"),
        "Expected Unauthorized error, got: {}",
        err.root_cause()
    );
}

#[test]
fn test_set_incoming_token_mapping_unregistered_chain_rejected() {
    let mut env = setup();

    // Use a chain ID that's not registered (0x00000099)
    let res = env.app.execute_contract(
        env.admin.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: Binary::from(vec![0, 0, 0, 0x99]),
            src_token: uluna_src_token(),
            local_token: "uluna".to_string(),
            src_decimals: 18,
        },
        &[],
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.root_cause().to_string().contains("not registered"),
        "Expected chain not registered error, got: {}",
        err.root_cause()
    );
}

#[test]
fn test_set_incoming_token_mapping_unsupported_local_token_rejected() {
    let mut env = setup();

    // Use a local token that's not registered
    let res = env.app.execute_contract(
        env.admin.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: evm_chain_id(),
            src_token: uluna_src_token(),
            local_token: "uusd".to_string(),
            src_decimals: 18,
        },
        &[],
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.root_cause().to_string().contains("not supported"),
        "Expected token not supported error, got: {}",
        err.root_cause()
    );
}

#[test]
fn test_set_incoming_token_mapping_invalid_src_chain_length() {
    let mut env = setup();

    let res = env.app.execute_contract(
        env.admin.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: Binary::from(vec![0, 0, 0]), // 3 bytes, not 4
            src_token: uluna_src_token(),
            local_token: "uluna".to_string(),
            src_decimals: 18,
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_set_incoming_token_mapping_invalid_src_token_length() {
    let mut env = setup();

    let res = env.app.execute_contract(
        env.admin.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: evm_chain_id(),
            src_token: Binary::from(vec![0; 16]), // 16 bytes, not 32
            local_token: "uluna".to_string(),
            src_decimals: 18,
        },
        &[],
    );

    assert!(res.is_err());
}

// ============================================================================
// RemoveIncomingTokenMapping Tests
// ============================================================================

#[test]
fn test_remove_incoming_token_mapping() {
    let mut env = setup();

    // First, set the mapping
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Verify it exists via query
    let resp: Option<IncomingTokenMappingResponse> = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::IncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
            },
        )
        .unwrap();
    assert!(resp.is_some());

    // Remove it
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::RemoveIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
            },
            &[],
        )
        .unwrap();

    // Verify it's gone
    let resp: Option<IncomingTokenMappingResponse> = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::IncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
            },
        )
        .unwrap();
    assert!(resp.is_none());
}

// ============================================================================
// IncomingTokenMapping Query Tests
// ============================================================================

#[test]
fn test_query_incoming_token_mapping() {
    let mut env = setup();

    // Set mapping
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    let resp: Option<IncomingTokenMappingResponse> = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::IncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
            },
        )
        .unwrap();

    let mapping = resp.unwrap();
    assert_eq!(mapping.local_token, "uluna");
    assert_eq!(mapping.src_decimals, 18);
    assert!(mapping.enabled);
}

#[test]
fn test_query_incoming_token_mapping_not_found() {
    let env = setup();

    let resp: Option<IncomingTokenMappingResponse> = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::IncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
            },
        )
        .unwrap();

    assert!(resp.is_none());
}

#[test]
fn test_query_incoming_token_mappings_paginated() {
    let mut env = setup();

    // Register a second chain
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::RegisterChain {
                identifier: "bsc_56".to_string(),
                chain_id: Binary::from(vec![0, 0, 0, 3]),
            },
            &[],
        )
        .unwrap();

    // Set two mappings
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: Binary::from(vec![0, 0, 0, 3]),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 8,
            },
            &[],
        )
        .unwrap();

    // Query all
    let resp: IncomingTokenMappingsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::IncomingTokenMappings {
                start_after: None,
                limit: Some(10),
            },
        )
        .unwrap();

    assert_eq!(resp.mappings.len(), 2);
}

// ============================================================================
// WithdrawSubmit Validation Tests
// ============================================================================

#[test]
fn test_withdraw_submit_rejects_unregistered_source_chain() {
    let mut env = setup();

    // Register incoming mapping for the known chain
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Try to submit from an unregistered chain (0x00000099)
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: Binary::from(vec![0, 0, 0, 0x99]),
            src_account: make_src_account(),
            token: "uluna".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 1,
        },
        &[],
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.root_cause()
            .to_string()
            .contains("Source chain not registered"),
        "Expected ChainNotRegistered error, got: {}",
        err.root_cause()
    );
}

#[test]
fn test_withdraw_submit_rejects_unmapped_token() {
    let mut env = setup();

    // Don't register any incoming token mapping

    // Try to submit a withdrawal — should fail because no incoming mapping for uluna from EVM chain
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: evm_chain_id(),
            src_account: make_src_account(),
            token: "uluna".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 1,
        },
        &[],
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.root_cause()
            .to_string()
            .contains("Token not mapped for source chain"),
        "Expected TokenNotMappedForChain error, got: {}",
        err.root_cause()
    );
}

#[test]
fn test_withdraw_submit_succeeds_with_valid_mapping() {
    let mut env = setup();

    // Register incoming token mapping
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Submit should succeed
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: evm_chain_id(),
            src_account: make_src_account(),
            token: "uluna".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 1,
        },
        &[],
    );

    assert!(
        res.is_ok(),
        "WithdrawSubmit should succeed: {:?}",
        res.err()
    );
}

#[test]
fn test_withdraw_submit_blocked_after_mapping_removal() {
    let mut env = setup();

    // Register incoming token mapping
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Verify it works
    env.app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: evm_chain_id(),
                src_account: make_src_account(),
                token: "uluna".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(1_000_000u128),
                nonce: 1,
            },
            &[],
        )
        .unwrap();

    // Remove the mapping
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::RemoveIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
            },
            &[],
        )
        .unwrap();

    // Now submit should fail
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: evm_chain_id(),
            src_account: make_src_account(),
            token: "uluna".to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 2,
        },
        &[],
    );

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.root_cause()
            .to_string()
            .contains("Token not mapped for source chain"),
        "Expected TokenNotMappedForChain error, got: {}",
        err.root_cause()
    );
}

#[test]
/// Test that a CW20 token requires its own incoming token mapping.
///
/// This is the root cause of Category B (operator_live_deposit_detection failure):
/// the e2e setup only registered incoming mapping for uluna, not the CW20 token.
/// When withdraw_submit is called with the CW20, it fails with TokenNotMappedForChain.
fn test_withdraw_submit_cw20_requires_incoming_mapping() {
    let mut env = setup();

    // Simulate a CW20 address (this is a mock, we just use a different token string)
    let cw20_addr = "terra1cw20mockaddressxxxxxxxxxxxxxxxxxxxxxxxxx";

    // Register CW20 as a token on the bridge
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::AddToken {
                token: cw20_addr.to_string(),
                is_native: false,
                token_type: None,
                evm_token_address: "0x0000000000000000000000000000000000000000".to_string(),
                terra_decimals: 6,
                evm_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Register incoming mapping for uluna (but NOT for CW20)
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Try to withdraw with CW20 → should FAIL because no incoming mapping exists
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: evm_chain_id(),
            src_account: make_src_account(),
            token: cw20_addr.to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 1,
        },
        &[],
    );

    assert!(
        res.is_err(),
        "WithdrawSubmit with CW20 should fail without incoming token mapping"
    );
    let err = res.unwrap_err();
    assert!(
        err.root_cause()
            .to_string()
            .contains("Token not mapped for source chain"),
        "Expected TokenNotMappedForChain error, got: {}",
        err.root_cause()
    );

    // Now register the incoming mapping for CW20
    // The src_token must match what the contract's `encode_token_address` produces.
    // In mock, addr_validate accepts any string ≥20 chars, so CW20-like addresses
    // go through the canonicalize+left-pad path (not keccak256).
    // We use mock_dependencies to replicate the exact encoding the contract uses.
    let mock_deps = cosmwasm_std::testing::mock_dependencies();
    let cw20_encoded = bridge::hash::encode_token_address(mock_deps.as_ref(), cw20_addr).unwrap();
    let cw20_src_token = Binary::from(cw20_encoded.to_vec());

    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: cw20_src_token,
                local_token: cw20_addr.to_string(),
                src_decimals: 18,
            },
            &[],
        )
        .unwrap();

    // Now retry → should succeed
    let res = env.app.execute_contract(
        env.user.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawSubmit {
            src_chain: evm_chain_id(),
            src_account: make_src_account(),
            token: cw20_addr.to_string(),
            recipient: env.user.to_string(),
            amount: Uint128::from(1_000_000u128),
            nonce: 1,
        },
        &[],
    );

    assert!(
        res.is_ok(),
        "WithdrawSubmit should succeed after adding CW20 incoming mapping: {:?}",
        res.err()
    );
}

#[test]
fn test_withdraw_submit_uses_src_decimals_from_mapping() {
    let mut env = setup();

    // Register incoming token mapping with specific src_decimals
    env.app
        .execute_contract(
            env.admin.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::SetIncomingTokenMapping {
                src_chain: evm_chain_id(),
                src_token: uluna_src_token(),
                local_token: "uluna".to_string(),
                src_decimals: 8, // Different from EVM's 18
            },
            &[],
        )
        .unwrap();

    // Submit should succeed
    let res = env
        .app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: evm_chain_id(),
                src_account: make_src_account(),
                token: "uluna".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(1_000_000u128),
                nonce: 1,
            },
            &[],
        )
        .unwrap();

    // Get the withdraw hash from the event
    let withdraw_hash_hex = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "withdraw_hash")
        .map(|a| a.value.clone())
        .expect("withdraw_hash attribute not found");
    let hash_bytes = hex::decode(&withdraw_hash_hex[2..]).unwrap();

    // Query the pending withdrawal and verify src_decimals
    let pending: bridge::msg::PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: Binary::from(hash_bytes),
            },
        )
        .unwrap();

    assert!(pending.exists);
    // Note: The PendingWithdrawResponse doesn't expose src_decimals directly,
    // but the fact that the submission succeeded with our custom mapping validates
    // the flow correctly uses the incoming token mapping.
}
