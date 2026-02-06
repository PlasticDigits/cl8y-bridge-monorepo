//! Integration tests for the Chain Registry system.
//!
//! Tests chain registration with auto-incrementing 4-byte IDs, duplicate
//! rejection, chain enable/disable, query by ID, query all chains, and
//! chain validation in deposit flows.

use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::msg::{
    ChainResponse, ChainsResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StatusResponse,
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

fn setup() -> (App, Addr) {
    let mut app = App::default();
    let admin = Addr::unchecked("terra1admin");
    let operator = Addr::unchecked("terra1operator");

    app.init_modules(|router, _, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(10_000_000_000, "uluna"))
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
            },
            &[],
            "cl8y-bridge",
            Some(admin.to_string()),
        )
        .unwrap();

    (app, contract_addr)
}

// ============================================================================
// Register Chain Tests
// ============================================================================

#[test]
fn test_register_single_chain() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    let res = app
        .execute_contract(
            admin.clone(),
            contract_addr.clone(),
            &ExecuteMsg::RegisterChain {
                identifier: "evm_1".to_string(),
            },
            &[],
        )
        .unwrap();

    // Check chain_id attribute
    let chain_id_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "chain_id")
        .map(|a| a.value.clone())
        .unwrap();

    // First chain should be 0x00000001
    assert_eq!(chain_id_attr, "0x00000001");

    // Check identifier attribute
    let identifier_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "identifier")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(identifier_attr, "evm_1");
}

#[test]
fn test_register_multiple_chains_auto_increment() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    let identifiers = vec![
        "evm_1",
        "evm_56",
        "evm_31337",
        "terraclassic_columbus-5",
        "terraclassic_localterra",
    ];

    for (i, identifier) in identifiers.iter().enumerate() {
        let res = app
            .execute_contract(
                admin.clone(),
                contract_addr.clone(),
                &ExecuteMsg::RegisterChain {
                    identifier: identifier.to_string(),
                },
                &[],
            )
            .unwrap();

        let chain_id_attr = res
            .events
            .iter()
            .flat_map(|e| &e.attributes)
            .find(|a| a.key == "chain_id")
            .map(|a| a.value.clone())
            .unwrap();

        let expected = format!("0x{:08x}", i + 1);
        assert_eq!(
            chain_id_attr, expected,
            "Chain {} should have ID {}",
            identifier, expected
        );
    }

    // Verify status shows correct count
    let status: StatusResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::Status {})
        .unwrap();
    assert_eq!(status.supported_chains, 5);
}

#[test]
fn test_register_chain_rejects_duplicate_identifier() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    // First registration succeeds
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    // Second registration with same identifier should fail
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("already registered"),
        "Expected already registered error, got: {}",
        err_str
    );
}

#[test]
fn test_register_chain_non_admin_rejected() {
    let (mut app, contract_addr) = setup();
    let random = Addr::unchecked("terra1random");

    let res = app.execute_contract(
        random.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("Unauthorized") || err_str.contains("admin"),
        "Expected unauthorized error, got: {}",
        err_str
    );
}

// ============================================================================
// Query Chain Tests
// ============================================================================

#[test]
fn test_query_chain_by_id() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_56".to_string(),
        },
        &[],
    )
    .unwrap();

    let chain: ChainResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chain {
                chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
        )
        .unwrap();

    assert_eq!(chain.identifier, "evm_56");
    assert!(chain.enabled);
    assert_eq!(chain.chain_id, Binary::from(vec![0, 0, 0, 1]));
    // Identifier hash should be 32 bytes (keccak256)
    assert_eq!(chain.identifier_hash.len(), 32);
}

#[test]
fn test_query_all_chains() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    let identifiers = ["evm_1", "evm_56", "terraclassic_columbus-5"];
    for ident in &identifiers {
        app.execute_contract(
            admin.clone(),
            contract_addr.clone(),
            &ExecuteMsg::RegisterChain {
                identifier: ident.to_string(),
            },
            &[],
        )
        .unwrap();
    }

    let chains: ChainsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chains {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(chains.chains.len(), 3);
}

#[test]
fn test_query_chains_pagination() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Register 5 chains
    for i in 1..=5 {
        app.execute_contract(
            admin.clone(),
            contract_addr.clone(),
            &ExecuteMsg::RegisterChain {
                identifier: format!("chain_{}", i),
            },
            &[],
        )
        .unwrap();
    }

    // Query with limit 2
    let page1: ChainsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chains {
                start_after: None,
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(page1.chains.len(), 2);

    // Query with start_after
    let page2: ChainsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chains {
                start_after: Some(page1.chains.last().unwrap().chain_id.clone()),
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(page2.chains.len(), 2);

    // Page 3 should have remaining 1
    let page3: ChainsResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chains {
                start_after: Some(page2.chains.last().unwrap().chain_id.clone()),
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(page3.chains.len(), 1);
}

// ============================================================================
// Update Chain Tests
// ============================================================================

#[test]
fn test_update_chain_disable() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    // Disable the chain
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateChain {
            chain_id: Binary::from(vec![0, 0, 0, 1]),
            enabled: Some(false),
        },
        &[],
    )
    .unwrap();

    let chain: ChainResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chain {
                chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
        )
        .unwrap();

    assert!(!chain.enabled);
}

#[test]
fn test_update_chain_reenable() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    // Disable then re-enable
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateChain {
            chain_id: Binary::from(vec![0, 0, 0, 1]),
            enabled: Some(false),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateChain {
            chain_id: Binary::from(vec![0, 0, 0, 1]),
            enabled: Some(true),
        },
        &[],
    )
    .unwrap();

    let chain: ChainResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chain {
                chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
        )
        .unwrap();

    assert!(chain.enabled);
}

#[test]
fn test_update_chain_nonexistent_rejected() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateChain {
            chain_id: Binary::from(vec![0, 0, 0, 99]),
            enabled: Some(false),
        },
        &[],
    );

    assert!(res.is_err());
}

#[test]
fn test_update_chain_non_admin_rejected() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");
    let random = Addr::unchecked("terra1random");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    let res = app.execute_contract(
        random.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateChain {
            chain_id: Binary::from(vec![0, 0, 0, 1]),
            enabled: Some(false),
        },
        &[],
    );

    assert!(res.is_err());
}

// ============================================================================
// Chain Validation in Deposits
// ============================================================================

#[test]
fn test_deposit_to_registered_chain_succeeds() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Register chain
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    // Add token
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

    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(&[0xAB; 20]);

    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 1]),
            dest_account: Binary::from(dest_account.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    );

    assert!(res.is_ok());
}

#[test]
fn test_deposit_to_unregistered_chain_rejected() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Register chain 1 but try to deposit to chain 99
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    // Add token
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

    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(&[0xAB; 20]);

    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 99]),
            dest_account: Binary::from(dest_account.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("not registered") || err_str.contains("not supported"),
        "Expected chain not registered error, got: {}",
        err_str
    );
}

#[test]
fn test_deposit_to_disabled_chain_rejected() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Register and disable chain
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::UpdateChain {
            chain_id: Binary::from(vec![0, 0, 0, 1]),
            enabled: Some(false),
        },
        &[],
    )
    .unwrap();

    // Add token
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

    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(&[0xAB; 20]);

    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 1]),
            dest_account: Binary::from(dest_account.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    );

    assert!(res.is_err());
    let err_str = res.unwrap_err().root_cause().to_string();
    assert!(
        err_str.contains("disabled"),
        "Expected chain disabled error, got: {}",
        err_str
    );
}

// ============================================================================
// Identifier Hash Tests
// ============================================================================

#[test]
fn test_chain_identifier_hash_is_keccak256() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    let chain: ChainResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chain {
                chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
        )
        .unwrap();

    // keccak256("evm_1") should be deterministic and 32 bytes
    assert_eq!(chain.identifier_hash.len(), 32);

    // Register same identifier would fail, confirming it's stored
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    );
    assert!(res.is_err());
}

#[test]
fn test_different_identifiers_different_hashes() {
    let (mut app, contract_addr) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_56".to_string(),
        },
        &[],
    )
    .unwrap();

    let chain1: ChainResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chain {
                chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
        )
        .unwrap();

    let chain2: ChainResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::Chain {
                chain_id: Binary::from(vec![0, 0, 0, 2]),
            },
        )
        .unwrap();

    assert_ne!(chain1.identifier_hash, chain2.identifier_hash);
}
