//! Integration tests for the V2 Fee System.
//!
//! Tests the fee manager: SetFeeParams, SetCustomAccountFee, RemoveCustomAccountFee,
//! fee queries (FeeConfig, AccountFee, HasCustomFee, CalculateFee), and fee
//! application in deposit flows.

use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::msg::{
    AccountFeeResponse, CalculateFeeResponse, ExecuteMsg, FeeConfigResponse, HasCustomFeeResponse,
    InstantiateMsg, QueryMsg,
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

fn setup() -> (App, Addr, Addr, Addr) {
    let mut app = App::default();
    let admin = Addr::unchecked("terra1admin");
    let operator = Addr::unchecked("terra1operator");
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

    // Register chain
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_1".to_string(),
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
            evm_token_address: "0x000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .to_string(),
            terra_decimals: 6,
            evm_decimals: 18,
        },
        &[],
    )
    .unwrap();

    (app, contract_addr, operator, user)
}

fn make_dest_account() -> Binary {
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(&[0xAB; 20]);
    Binary::from(bytes.to_vec())
}

// ============================================================================
// Fee Config Query Tests
// ============================================================================

#[test]
fn test_default_fee_config() {
    let (app, contract_addr, _operator, _user) = setup();

    let fee_config: FeeConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::FeeConfig {})
        .unwrap();

    assert_eq!(fee_config.standard_fee_bps, 50);
    assert_eq!(fee_config.discounted_fee_bps, 10);
    assert_eq!(fee_config.cl8y_threshold, Uint128::from(100_000_000u128));
    assert!(fee_config.cl8y_token.is_none());
    assert_eq!(fee_config.fee_recipient, Addr::unchecked("terra1admin"));
}

// ============================================================================
// SetFeeParams Tests
// ============================================================================

#[test]
fn test_set_fee_params_updates_standard_fee() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: Some(75),
            discounted_fee_bps: None,
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: None,
        },
        &[],
    )
    .unwrap();

    let fee_config: FeeConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::FeeConfig {})
        .unwrap();

    assert_eq!(fee_config.standard_fee_bps, 75);
    // Other fields unchanged
    assert_eq!(fee_config.discounted_fee_bps, 10);
}

#[test]
fn test_set_fee_params_updates_discounted_fee() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: None,
            discounted_fee_bps: Some(5),
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: None,
        },
        &[],
    )
    .unwrap();

    let fee_config: FeeConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::FeeConfig {})
        .unwrap();

    assert_eq!(fee_config.discounted_fee_bps, 5);
}

#[test]
fn test_set_fee_params_updates_fee_recipient() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let new_recipient = "terra1newrecipient";

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: None,
            discounted_fee_bps: None,
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: Some(new_recipient.to_string()),
        },
        &[],
    )
    .unwrap();

    let fee_config: FeeConfigResponse = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::FeeConfig {})
        .unwrap();

    assert_eq!(fee_config.fee_recipient, Addr::unchecked(new_recipient));
}

#[test]
fn test_set_fee_params_rejects_above_max() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Standard fee > 100 bps (1%)
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: Some(101),
            discounted_fee_bps: None,
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: None,
        },
        &[],
    );
    assert!(res.is_err());

    // Discounted fee > 100 bps
    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: None,
            discounted_fee_bps: Some(101),
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: None,
        },
        &[],
    );
    assert!(res.is_err());
}

#[test]
fn test_set_fee_params_non_admin_rejected() {
    let (mut app, contract_addr, _operator, user) = setup();

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: Some(50),
            discounted_fee_bps: None,
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: None,
        },
        &[],
    );
    assert!(res.is_err());
}

// ============================================================================
// Custom Account Fee Tests
// ============================================================================

#[test]
fn test_set_custom_account_fee() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Set custom fee for user
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 25,
        },
        &[],
    )
    .unwrap();

    // Query HasCustomFee
    let has_custom: HasCustomFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::HasCustomFee {
                account: user.to_string(),
            },
        )
        .unwrap();
    assert!(has_custom.has_custom_fee);

    // Query AccountFee
    let account_fee: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: user.to_string(),
            },
        )
        .unwrap();
    assert_eq!(account_fee.fee_bps, 25);
    assert_eq!(account_fee.fee_type, "custom");
}

#[test]
fn test_set_custom_fee_by_operator() {
    let (mut app, contract_addr, operator, user) = setup();

    // Operator can also set custom fees
    app.execute_contract(
        operator.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 30,
        },
        &[],
    )
    .unwrap();

    let has_custom: HasCustomFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::HasCustomFee {
                account: user.to_string(),
            },
        )
        .unwrap();
    assert!(has_custom.has_custom_fee);
}

#[test]
fn test_set_custom_fee_rejects_above_max() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    let res = app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 101,
        },
        &[],
    );
    assert!(res.is_err());
}

#[test]
fn test_set_custom_fee_zero_allowed() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Zero fee (free bridging for VIPs)
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

    let account_fee: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: user.to_string(),
            },
        )
        .unwrap();
    assert_eq!(account_fee.fee_bps, 0);
    assert_eq!(account_fee.fee_type, "custom");
}

#[test]
fn test_remove_custom_account_fee() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Set then remove
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 25,
        },
        &[],
    )
    .unwrap();

    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RemoveCustomAccountFee {
            account: user.to_string(),
        },
        &[],
    )
    .unwrap();

    // Should fall back to standard
    let has_custom: HasCustomFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::HasCustomFee {
                account: user.to_string(),
            },
        )
        .unwrap();
    assert!(!has_custom.has_custom_fee);

    let account_fee: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: user.to_string(),
            },
        )
        .unwrap();
    assert_eq!(account_fee.fee_bps, 50); // back to standard
    assert_eq!(account_fee.fee_type, "standard");
}

#[test]
fn test_set_custom_fee_non_authorized_rejected() {
    let (mut app, contract_addr, _operator, user) = setup();
    let random = Addr::unchecked("terra1random");

    let res = app.execute_contract(
        random.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 25,
        },
        &[],
    );
    assert!(res.is_err());
}

// ============================================================================
// CalculateFee Query Tests
// ============================================================================

#[test]
fn test_calculate_fee_standard() {
    let (app, contract_addr, _operator, user) = setup();

    let result: CalculateFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::CalculateFee {
                depositor: user.to_string(),
                amount: Uint128::from(1_000_000u128),
            },
        )
        .unwrap();

    // 0.5% of 1_000_000 = 5_000
    assert_eq!(result.fee_amount, Uint128::from(5_000u128));
    assert_eq!(result.fee_bps, 50);
    assert_eq!(result.fee_type, "standard");
}

#[test]
fn test_calculate_fee_with_custom() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Set custom fee of 0.25% (25 bps)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user.to_string(),
            fee_bps: 25,
        },
        &[],
    )
    .unwrap();

    let result: CalculateFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::CalculateFee {
                depositor: user.to_string(),
                amount: Uint128::from(1_000_000u128),
            },
        )
        .unwrap();

    // 0.25% of 1_000_000 = 2_500
    assert_eq!(result.fee_amount, Uint128::from(2_500u128));
    assert_eq!(result.fee_bps, 25);
    assert_eq!(result.fee_type, "custom");
}

#[test]
fn test_calculate_fee_zero_amount() {
    let (app, contract_addr, _operator, user) = setup();

    let result: CalculateFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::CalculateFee {
                depositor: user.to_string(),
                amount: Uint128::zero(),
            },
        )
        .unwrap();

    assert_eq!(result.fee_amount, Uint128::zero());
}

#[test]
fn test_calculate_fee_max_1_percent() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");

    // Set maximum allowed custom fee (100 bps = 1%)
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

    let result: CalculateFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::CalculateFee {
                depositor: user.to_string(),
                amount: Uint128::from(10_000u128),
            },
        )
        .unwrap();

    // 1% of 10_000 = 100
    assert_eq!(result.fee_amount, Uint128::from(100u128));
    assert_eq!(result.fee_bps, 100);
}

// ============================================================================
// Fee Applied in Deposit Flow Tests
// ============================================================================

#[test]
fn test_deposit_applies_standard_fee() {
    let (mut app, contract_addr, _operator, user) = setup();
    let dest_account = make_dest_account();

    let res = app
        .execute_contract(
            user.clone(),
            contract_addr.clone(),
            &ExecuteMsg::DepositNative {
                dest_chain: Binary::from(vec![0, 0, 0, 2]),
                dest_account,
            },
            &coins(1_000_000, "uluna"),
        )
        .unwrap();

    // Check fee attribute
    let fee_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "fee")
        .map(|a| a.value.clone())
        .unwrap();

    // Standard fee: 0.5% of 1_000_000 = 5_000
    assert_eq!(fee_attr, "5000");

    // Check fee_type attribute
    let fee_type_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "fee_type")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(fee_type_attr, "standard");

    // Net amount should be 1_000_000 - 5_000 = 995_000
    let amount_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "amount")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(amount_attr, "995000");
}

#[test]
fn test_deposit_applies_custom_fee() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let dest_account = make_dest_account();

    // Set custom fee of 0 bps (free)
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

    let res = app
        .execute_contract(
            user.clone(),
            contract_addr.clone(),
            &ExecuteMsg::DepositNative {
                dest_chain: Binary::from(vec![0, 0, 0, 2]),
                dest_account,
            },
            &coins(1_000_000, "uluna"),
        )
        .unwrap();

    // Check fee is 0
    let fee_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "fee")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(fee_attr, "0");

    // Check fee_type is custom
    let fee_type_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "fee_type")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(fee_type_attr, "custom");

    // Net amount = full amount
    let amount_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "amount")
        .map(|a| a.value.clone())
        .unwrap();
    assert_eq!(amount_attr, "1000000");
}

#[test]
fn test_deposit_fee_applied_after_set_fee_params() {
    let (mut app, contract_addr, _operator, user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let dest_account = make_dest_account();

    // Change standard fee to 1% (100 bps)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetFeeParams {
            standard_fee_bps: Some(100),
            discounted_fee_bps: None,
            cl8y_threshold: None,
            cl8y_token: None,
            fee_recipient: None,
        },
        &[],
    )
    .unwrap();

    let res = app
        .execute_contract(
            user.clone(),
            contract_addr.clone(),
            &ExecuteMsg::DepositNative {
                dest_chain: Binary::from(vec![0, 0, 0, 2]),
                dest_account,
            },
            &coins(1_000_000, "uluna"),
        )
        .unwrap();

    let fee_attr = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "fee")
        .map(|a| a.value.clone())
        .unwrap();

    // 1% of 1_000_000 = 10_000
    assert_eq!(fee_attr, "10000");
}

// ============================================================================
// Fee Priority Tests
// ============================================================================

#[test]
fn test_custom_fee_overrides_standard() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let account = Addr::unchecked("terra1special");

    // Standard is 50 bps
    // Set custom to 10 bps
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: account.to_string(),
            fee_bps: 10,
        },
        &[],
    )
    .unwrap();

    let result: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: account.to_string(),
            },
        )
        .unwrap();

    assert_eq!(result.fee_bps, 10);
    assert_eq!(result.fee_type, "custom");
}

#[test]
fn test_remove_custom_reverts_to_standard() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let account = Addr::unchecked("terra1special");

    // Set custom
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: account.to_string(),
            fee_bps: 10,
        },
        &[],
    )
    .unwrap();

    // Remove custom
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RemoveCustomAccountFee {
            account: account.to_string(),
        },
        &[],
    )
    .unwrap();

    let result: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: account.to_string(),
            },
        )
        .unwrap();

    assert_eq!(result.fee_bps, 50); // Standard
    assert_eq!(result.fee_type, "standard");
}

#[test]
fn test_multiple_accounts_different_fees() {
    let (mut app, contract_addr, _operator, _user) = setup();
    let admin = Addr::unchecked("terra1admin");
    let user_a = "terra1usera";
    let user_b = "terra1userb";
    let user_c = "terra1userc";

    // user_a: custom 0 bps (free)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user_a.to_string(),
            fee_bps: 0,
        },
        &[],
    )
    .unwrap();

    // user_b: custom 100 bps (max)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetCustomAccountFee {
            account: user_b.to_string(),
            fee_bps: 100,
        },
        &[],
    )
    .unwrap();

    // user_c: no custom (standard 50 bps)

    let fee_a: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: user_a.to_string(),
            },
        )
        .unwrap();
    let fee_b: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: user_b.to_string(),
            },
        )
        .unwrap();
    let fee_c: AccountFeeResponse = app
        .wrap()
        .query_wasm_smart(
            &contract_addr,
            &QueryMsg::AccountFee {
                account: user_c.to_string(),
            },
        )
        .unwrap();

    assert_eq!(fee_a.fee_bps, 0);
    assert_eq!(fee_a.fee_type, "custom");
    assert_eq!(fee_b.fee_bps, 100);
    assert_eq!(fee_b.fee_type, "custom");
    assert_eq!(fee_c.fee_bps, 50);
    assert_eq!(fee_c.fee_type, "standard");
}
