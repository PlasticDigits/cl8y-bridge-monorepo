//! Integration tests for the Universal Address Codec.
//!
//! Tests the address_codec module's encoding/decoding for EVM and Cosmos
//! addresses, bytes32 round-trips, chain type validation, strict mode,
//! and that encoded addresses work correctly in deposit flows.

use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::address_codec::{
    decode_bech32_address, encode_bech32_address, encode_evm_address, parse_evm_address,
    UniversalAddress, CHAIN_TYPE_BITCOIN, CHAIN_TYPE_COSMOS, CHAIN_TYPE_EVM, CHAIN_TYPE_SOLANA,
};
use bridge::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

// ============================================================================
// Test Helpers
// ============================================================================

fn contract_bridge() -> Box<dyn cw_multi_test::Contract<cosmwasm_std::Empty>> {
    let contract = ContractWrapper::new(
        bridge::contract::execute,
        bridge::contract::instantiate,
        bridge::contract::query,
    );
    Box::new(contract)
}

fn setup_bridge() -> (App, Addr) {
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
                this_chain_id: Binary::from(vec![0, 0, 0, 1]),
            },
            &[],
            "cl8y-bridge",
            Some(admin.to_string()),
        )
        .unwrap();

    // Register a chain so deposits work
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
            terra_decimals: 6,
            min_bridge_amount: None,
            max_bridge_amount: None,
        },
        &[],
    )
    .unwrap();

    // Set destination token mapping for uluna → chain 2
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetTokenDestination {
            token: "uluna".to_string(),
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_token: "000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
                .to_string(),
            dest_decimals: 18,
        },
        &[],
    )
    .unwrap();

    (app, contract_addr)
}

// ============================================================================
// EVM Address Tests
// ============================================================================

#[test]
fn test_evm_address_roundtrip() {
    let addr = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    let universal = UniversalAddress::from_evm(addr).unwrap();

    assert_eq!(universal.chain_type, CHAIN_TYPE_EVM);
    assert!(universal.is_evm());
    assert!(!universal.is_cosmos());
    assert!(universal.is_valid_chain_type());

    let bytes32 = universal.to_bytes32();
    assert_eq!(&bytes32[0..4], &[0, 0, 0, 1]); // EVM chain type

    let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
    assert_eq!(parsed.chain_type, CHAIN_TYPE_EVM);
    assert_eq!(parsed.raw_address, universal.raw_address);

    let recovered = parsed.to_evm_string().unwrap();
    assert_eq!(recovered.to_lowercase(), addr.to_lowercase());
}

#[test]
fn test_evm_address_without_0x_prefix() {
    let addr_no_prefix = "f39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    let raw = parse_evm_address(addr_no_prefix).unwrap();
    let addr_with_prefix = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    let raw2 = parse_evm_address(addr_with_prefix).unwrap();
    assert_eq!(raw, raw2);
}

#[test]
fn test_evm_address_invalid_length() {
    let result = parse_evm_address("0x1234");
    assert!(result.is_err());

    let result = parse_evm_address("0x");
    assert!(result.is_err());
}

#[test]
fn test_evm_address_invalid_hex() {
    let result = parse_evm_address("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ");
    assert!(result.is_err());
}

#[test]
fn test_encode_evm_address() {
    let raw = [0xABu8; 20];
    let encoded = encode_evm_address(&raw);
    // 20 bytes = 40 hex chars + "0x" prefix = 42 chars
    assert!(encoded.starts_with("0x"));
    assert_eq!(encoded.len(), 42);
    assert_eq!(encoded, "0xabababababababababababababababababababab");
}

// ============================================================================
// Cosmos Address Tests
// ============================================================================

#[test]
fn test_cosmos_address_roundtrip() {
    let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    let universal = UniversalAddress::from_cosmos(terra_addr).unwrap();

    assert_eq!(universal.chain_type, CHAIN_TYPE_COSMOS);
    assert!(universal.is_cosmos());
    assert!(!universal.is_evm());

    let bytes32 = universal.to_bytes32();
    assert_eq!(&bytes32[0..4], &[0, 0, 0, 2]); // Cosmos chain type

    let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
    assert_eq!(parsed.chain_type, CHAIN_TYPE_COSMOS);
    assert_eq!(parsed.raw_address, universal.raw_address);

    let recovered = parsed.to_terra_string().unwrap();
    assert_eq!(recovered.to_lowercase(), terra_addr.to_lowercase());
}

#[test]
fn test_cosmos_from_addr() {
    let addr = Addr::unchecked("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v");
    let universal = UniversalAddress::from_addr(&addr).unwrap();
    assert_eq!(universal.chain_type, CHAIN_TYPE_COSMOS);
    assert!(universal.is_cosmos());
}

#[test]
fn test_bech32_roundtrip() {
    let terra_addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    let raw = decode_bech32_address(terra_addr).unwrap();
    assert_eq!(raw.len(), 20);

    let re_encoded = encode_bech32_address(&raw, "terra").unwrap();
    assert_eq!(re_encoded.to_lowercase(), terra_addr.to_lowercase());
}

#[test]
fn test_bech32_invalid_format() {
    let result = decode_bech32_address("notabech32address");
    // Should fail (no separator '1' or data too short)
    assert!(result.is_err());
}

// ============================================================================
// Bytes32 Serialization Tests
// ============================================================================

#[test]
fn test_bytes32_evm_roundtrip() {
    let addr = "0xdead000000000000000000000000000000000000";
    let universal = UniversalAddress::from_evm(addr).unwrap();
    let bytes32 = universal.to_bytes32();
    let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
    assert_eq!(universal, parsed);
}

#[test]
fn test_bytes32_cosmos_roundtrip() {
    let addr = "terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v";
    let universal = UniversalAddress::from_cosmos(addr).unwrap();
    let bytes32 = universal.to_bytes32();
    let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
    assert_eq!(universal, parsed);
}

#[test]
fn test_bytes32_reserved_zeroed() {
    let universal =
        UniversalAddress::from_evm("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
    let bytes32 = universal.to_bytes32();
    // Reserved bytes (24-31) should be zero
    assert_eq!(&bytes32[24..32], &[0u8; 8]);
}

#[test]
fn test_bytes32_with_reserved() {
    let mut reserved = [0u8; 8];
    reserved[7] = 0xFF;
    let universal =
        UniversalAddress::new_with_reserved(CHAIN_TYPE_EVM, [0xAA; 20], reserved).unwrap();
    let bytes32 = universal.to_bytes32();

    // Reserved byte at position 31 should be 0xFF
    assert_eq!(bytes32[31], 0xFF);

    let parsed = UniversalAddress::from_bytes32(&bytes32).unwrap();
    assert_eq!(parsed.reserved, reserved);
}

// ============================================================================
// Chain Type Validation Tests
// ============================================================================

#[test]
fn test_invalid_chain_type_zero() {
    let result = UniversalAddress::new(0, [0u8; 20]);
    assert!(result.is_err());
}

#[test]
fn test_invalid_chain_type_from_bytes() {
    let bytes32 = [0u8; 32]; // chain type = 0
    let result = UniversalAddress::from_bytes32(&bytes32);
    assert!(result.is_err());
}

#[test]
fn test_valid_chain_types() {
    for chain_type in [
        CHAIN_TYPE_EVM,
        CHAIN_TYPE_COSMOS,
        CHAIN_TYPE_SOLANA,
        CHAIN_TYPE_BITCOIN,
    ] {
        let addr = UniversalAddress::new(chain_type, [0x11; 20]).unwrap();
        assert!(addr.is_valid_chain_type());
    }
}

#[test]
fn test_unknown_chain_type() {
    // Chain type 99 is valid to create but not in the known range
    let addr = UniversalAddress::new(99, [0x11; 20]).unwrap();
    assert!(!addr.is_valid_chain_type());
}

#[test]
fn test_evm_string_wrong_type() {
    let cosmos_addr =
        UniversalAddress::from_cosmos("terra1x46rqay4d3cssq8gxxvqz8xt6nwlz4td20k38v").unwrap();
    let result = cosmos_addr.to_evm_string();
    assert!(result.is_err());
}

#[test]
fn test_cosmos_string_wrong_type() {
    let evm_addr =
        UniversalAddress::from_evm("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
    let result = evm_addr.to_cosmos_string("terra");
    assert!(result.is_err());
}

// ============================================================================
// Strict Validation Tests
// ============================================================================

#[test]
fn test_strict_rejects_nonzero_reserved() {
    let mut bytes32 = [0u8; 32];
    bytes32[3] = 1; // valid chain type = 1
    bytes32[31] = 1; // non-zero reserved
    let result = UniversalAddress::from_bytes32_strict(&bytes32);
    assert!(result.is_err());
}

#[test]
fn test_strict_accepts_zero_reserved() {
    let mut bytes32 = [0u8; 32];
    bytes32[3] = 1; // valid chain type = 1
    let result = UniversalAddress::from_bytes32_strict(&bytes32);
    assert!(result.is_ok());
}

// ============================================================================
// Deposit Flow Integration with Address Codec
// ============================================================================

#[test]
fn test_deposit_with_evm_dest_account() {
    let (mut app, contract_addr) = setup_bridge();
    let user = Addr::unchecked("terra1admin"); // admin has funds

    // Encode an EVM address as the destination account
    let dest_addr =
        UniversalAddress::from_evm("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
    let dest_bytes32 = dest_addr.to_bytes32();

    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(dest_bytes32.to_vec()),
        },
        &coins(1_000_000, "uluna"),
    );

    assert!(
        res.is_ok(),
        "DepositNative with EVM dest failed: {:?}",
        res.err()
    );

    // Verify the deposit was stored
    let deposit: Option<bridge::msg::DepositInfoResponse> = app
        .wrap()
        .query_wasm_smart(&contract_addr, &QueryMsg::DepositByNonce { nonce: 0 })
        .unwrap();
    assert!(deposit.is_some());

    // Verify the dest_account matches what we encoded
    let d = deposit.unwrap();
    assert_eq!(d.dest_account.to_vec(), dest_bytes32.to_vec());
}

#[test]
fn test_deposit_with_32_byte_dest_account() {
    let (mut app, contract_addr) = setup_bridge();
    let user = Addr::unchecked("terra1admin");

    // Create a raw 32-byte destination (simulating another chain's format)
    let mut dest_account_bytes = [0u8; 32];
    dest_account_bytes[12..32].copy_from_slice(&[
        0xDE, 0xAD, 0xDE, 0xAD, 0xDE, 0xAD, 0xDE, 0xAD, 0xDE, 0xAD, 0xDE, 0xAD, 0xDE, 0xAD, 0xDE,
        0xAD, 0xDE, 0xAD, 0xDE, 0xAD,
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

    assert!(
        res.is_ok(),
        "DepositNative with 32-byte dest failed: {:?}",
        res.err()
    );
}

#[test]
fn test_deposit_rejects_invalid_dest_account_length() {
    let (mut app, contract_addr) = setup_bridge();
    let user = Addr::unchecked("terra1admin");

    // Send 20 bytes instead of 32
    let res = app.execute_contract(
        user.clone(),
        contract_addr.clone(),
        &ExecuteMsg::DepositNative {
            dest_chain: Binary::from(vec![0, 0, 0, 2]),
            dest_account: Binary::from(vec![0xAB; 20]),
        },
        &coins(1_000_000, "uluna"),
    );

    assert!(res.is_err(), "Should reject non-32-byte dest_account");
}

// ============================================================================
// Cross-Chain Address Compatibility Tests
// ============================================================================

#[test]
fn test_different_addresses_produce_different_bytes32() {
    let evm1 = UniversalAddress::from_evm("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266").unwrap();
    let evm2 = UniversalAddress::from_evm("0x70997970C51812dc3A010C7d01b50e0d17dc79C8").unwrap();

    assert_ne!(evm1.to_bytes32(), evm2.to_bytes32());
}

#[test]
fn test_same_raw_different_chain_type() {
    let raw = [0xAB; 20];
    let evm = UniversalAddress::new(CHAIN_TYPE_EVM, raw).unwrap();
    let cosmos = UniversalAddress::new(CHAIN_TYPE_COSMOS, raw).unwrap();

    // Same raw address but different chain types should produce different bytes32
    assert_ne!(evm.to_bytes32(), cosmos.to_bytes32());
    assert_ne!(evm, cosmos);
}
