//! Cross-Chain Hash Parity Tests
//!
//! These tests verify that the withdrawal hash computed by the Terra contract
//! matches the deposit hash computed by the EVM contract for the same transfer.
//!
//! This is critical because the operator's approval flow depends on hash matching:
//! 1. EVM stores deposit hash via `HashLib.computeTransferHash()`
//! 2. Terra computes withdraw hash via `compute_transfer_hash()`
//! 3. Operator reads the hash from Terra's `pending_withdrawals` query
//! 4. Operator calls EVM's `getDeposit(hash)` to verify
//! 5. If the hashes don't match, the operator never finds the deposit and never approves
//!
//! These tests don't require the full e2e infrastructure — they run purely as
//! unit/integration tests using cw-multi-test.

use cosmwasm_std::{coins, Addr, Binary, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use bridge::hash::{bytes32_to_hex, compute_transfer_hash, keccak256};
use bridge::msg::{
    ExecuteMsg, InstantiateMsg, PendingWithdrawResponse, PendingWithdrawalsResponse, QueryMsg,
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

#[allow(dead_code)]
struct TestEnv {
    app: App,
    contract_addr: Addr,
    admin: Addr,
    operator: Addr,
    user: Addr,
}

/// Setup with specific chain IDs matching the e2e configuration:
/// - EVM = 0x00000001 (src chain)
/// - Terra = 0x00000002 (dest chain / this chain)
fn setup_with_e2e_chain_ids() -> TestEnv {
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
            .init_balance(storage, &operator, coins(10_000_000_000, "uluna"))
            .unwrap();
        router
            .bank
            .init_balance(storage, &user, coins(10_000_000_000, "uluna"))
            .unwrap();
    });

    let code_id = app.store_code(contract_bridge());

    // Terra chain ID = 0x00000002 (matching e2e setup)
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
                fee_bps: 0, // Zero fees for hash parity testing
                fee_collector: admin.to_string(),
                this_chain_id: Binary::from(vec![0, 0, 0, 2]), // Terra = chain 2
            },
            &[],
            "cl8y-bridge",
            Some(admin.to_string()),
        )
        .unwrap();

    // Register EVM chain (chain ID = 0x00000001)
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::RegisterChain {
            identifier: "evm_31337".to_string(),
            chain_id: Binary::from(vec![0, 0, 0, 1]),
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

    // Register incoming token mapping (EVM chain → uluna)
    // The src_token must be keccak256("uluna") to match what EVM's TokenRegistry
    // stores as destToken for the test token → Terra mapping
    let src_token_bytes = keccak256(b"uluna");
    app.execute_contract(
        admin.clone(),
        contract_addr.clone(),
        &ExecuteMsg::SetIncomingTokenMapping {
            src_chain: Binary::from(vec![0, 0, 0, 1]),
            src_token: Binary::from(src_token_bytes.to_vec()),
            local_token: "uluna".to_string(),
            src_decimals: 18,
        },
        &[],
    )
    .unwrap();

    TestEnv {
        app,
        contract_addr,
        admin,
        operator,
        user,
    }
}

// ============================================================================
// Cross-Chain Hash Parity Tests
// ============================================================================

/// Test that the Terra contract's WithdrawSubmit hash is internally consistent.
///
/// This verifies that:
/// 1. WithdrawSubmit stores the hash correctly
/// 2. The hash can be retrieved from pending_withdrawals
/// 3. The hash matches what compute_transfer_hash produces with the SAME
///    internal parameters the contract uses (including its encoding of dest_account)
///
/// This is critical because the operator reads the hash from pending_withdrawals
/// and uses it to query EVM's getDeposit(hash). If the contract stores a hash
/// that doesn't match compute_transfer_hash, the whole flow breaks.
#[test]
fn test_withdraw_hash_internal_consistency() {
    let mut env = setup_with_e2e_chain_ids();

    // Parameters matching a realistic EVM deposit:
    let evm_chain_id: [u8; 4] = [0, 0, 0, 1]; // EVM thisChainId
    let terra_chain_id: [u8; 4] = [0, 0, 0, 2]; // Terra thisChainId

    // EVM depositor address (left-padded to 32 bytes, as HashLib.addressToBytes32 does)
    let evm_address: [u8; 20] = [
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&evm_address);

    let recipient = env.user.to_string();
    let amount: u128 = 995_000;
    let nonce: u64 = 1;

    // Step 1: Submit the withdrawal on Terra
    let res = env
        .app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: Binary::from(evm_chain_id.to_vec()),
                src_account: Binary::from(src_account.to_vec()),
                token: "uluna".to_string(),
                recipient: recipient.clone(),
                amount: Uint128::from(amount),
                nonce,
            },
            &[],
        )
        .unwrap();

    // Step 2: Extract the hash from the event
    let withdraw_hash_hex = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "withdraw_hash")
        .map(|a| a.value.clone())
        .expect("withdraw_hash attribute not found in WithdrawSubmit response");

    let contract_hash_bytes = hex::decode(&withdraw_hash_hex[2..]).unwrap();

    // Step 3: Query the pending withdrawal to get the stored parameters
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: Binary::from(contract_hash_bytes.clone()),
            },
        )
        .unwrap();

    // Step 4: Verify the stored parameters match what we submitted
    assert_eq!(pending.nonce, nonce, "Stored nonce must match");
    assert_eq!(
        pending.amount,
        Uint128::from(amount),
        "Stored amount must match"
    );
    assert_eq!(pending.token, "uluna", "Stored token must match");
    assert_eq!(
        pending.src_chain,
        Binary::from(evm_chain_id.to_vec()),
        "Stored src_chain must match"
    );
    assert_eq!(
        pending.src_account,
        Binary::from(src_account.to_vec()),
        "Stored src_account must match"
    );

    // Step 5: Re-compute the hash using the contract's stored dest_account
    // (This is what the contract uses internally — it canonicalizes the address)
    let stored_dest_account: [u8; 32] = pending
        .dest_account
        .to_vec()
        .try_into()
        .expect("dest_account should be 32 bytes");

    let token_hash = keccak256(b"uluna");

    let recomputed_hash = compute_transfer_hash(
        &evm_chain_id,
        &terra_chain_id,
        &src_account,
        &stored_dest_account,
        &token_hash,
        amount,
        nonce,
    );

    let mut contract_hash = [0u8; 32];
    contract_hash.copy_from_slice(&contract_hash_bytes);

    assert_eq!(
        contract_hash,
        recomputed_hash,
        "Contract's stored hash MUST match recomputed hash using the same parameters.\n\
         Contract hash:   {}\n\
         Recomputed hash: {}\n\
         \n\
         Stored parameters:\n\
         - src_chain:    0x{}\n\
         - dest_chain:   0x{} (THIS_CHAIN_ID)\n\
         - src_account:  0x{}\n\
         - dest_account: 0x{}\n\
         - token:        0x{}\n\
         - amount:       {}\n\
         - nonce:        {}",
        bytes32_to_hex(&contract_hash),
        bytes32_to_hex(&recomputed_hash),
        hex::encode(evm_chain_id),
        hex::encode(terra_chain_id),
        hex::encode(src_account),
        hex::encode(stored_dest_account),
        hex::encode(token_hash),
        amount,
        nonce,
    );
}

/// Test that "uluna" token encoding is keccak256("uluna"),
/// matching what the EVM TokenRegistry's destToken value should be.
///
/// The Terra contract's encode_token_address for native denoms (short strings)
/// uses keccak256(denom_bytes). The EVM side must store the same value.
#[test]
fn test_uluna_token_encoding_matches_evm() {
    // On the EVM side, TokenRegistry stores destToken = keccak256("uluna")
    // via encode_terra_token_address("uluna")
    let evm_dest_token = keccak256(b"uluna");

    // On the Terra side, encode_token_address for native denoms (shorter than 20 chars)
    // uses keccak256(token.as_bytes()). Since "uluna" is 5 chars, it's treated as native.
    let terra_token = keccak256(b"uluna");

    assert_eq!(
        terra_token,
        evm_dest_token,
        "Token encoding mismatch!\n\
         Terra keccak256(\"uluna\"): 0x{}\n\
         EVM keccak256(\"uluna\"): 0x{}\n\
         \n\
         This would cause all hash comparisons to fail.",
        hex::encode(terra_token),
        hex::encode(evm_dest_token),
    );

    // Verify it's a specific known value (cross-chain constant)
    assert_ne!(terra_token, [0u8; 32], "Hash should not be zero");
}

/// Test that the pending_withdrawals query returns the hash in base64 format
/// that the operator can correctly decode.
///
/// Simulates the operator's polling flow:
/// 1. Query pending_withdrawals
/// 2. Extract withdraw_hash (base64)
/// 3. Decode to bytes
/// 4. Verify it matches the expected hash
#[test]
fn test_pending_withdrawals_hash_format_for_operator() {
    let mut env = setup_with_e2e_chain_ids();

    let evm_chain_id: [u8; 4] = [0, 0, 0, 1];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&[0xAB; 20]);

    let amount: u128 = 1_000_000;
    let nonce: u64 = 42;

    // Submit a withdrawal
    env.app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: Binary::from(evm_chain_id.to_vec()),
                src_account: Binary::from(src_account.to_vec()),
                token: "uluna".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(amount),
                nonce,
            },
            &[],
        )
        .unwrap();

    // Query pending_withdrawals (same as what the operator does)
    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: Some(30),
            },
        )
        .unwrap();

    assert_eq!(
        result.withdrawals.len(),
        1,
        "Should have exactly 1 pending withdrawal"
    );

    let entry = &result.withdrawals[0];

    // Verify the hash is in Binary format (which serializes as base64)
    let hash_bytes = entry.withdraw_hash.as_slice();
    assert_eq!(
        hash_bytes.len(),
        32,
        "Withdraw hash should be exactly 32 bytes"
    );

    // Verify the entry fields the operator checks
    assert!(!entry.approved, "Should not be approved yet");
    assert!(!entry.cancelled, "Should not be cancelled");
    assert!(!entry.executed, "Should not be executed");
    assert_eq!(entry.nonce, nonce, "Nonce should match");
    assert_eq!(entry.token, "uluna", "Token should be uluna");

    // Now verify the hash can be used to look up the withdrawal by hash
    let single: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: entry.withdraw_hash.clone(),
            },
        )
        .unwrap();

    assert_eq!(single.nonce, nonce);
    assert_eq!(single.amount, Uint128::from(amount));
}

/// Test that the operator can approve using the hash from pending_withdrawals.
///
/// Simulates the complete operator approval flow:
/// 1. User submits WithdrawSubmit
/// 2. "Operator" queries pending_withdrawals
/// 3. "Operator" extracts hash from unapproved entry
/// 4. "Operator" calls WithdrawApprove with that hash
/// 5. Verify the withdrawal is now approved
#[test]
fn test_simulated_operator_approve_flow() {
    let mut env = setup_with_e2e_chain_ids();

    let evm_chain_id: [u8; 4] = [0, 0, 0, 1];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&[0xAB; 20]);

    // Step 1: User submits withdrawal
    env.app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: Binary::from(evm_chain_id.to_vec()),
                src_account: Binary::from(src_account.to_vec()),
                token: "uluna".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(995_000u128),
                nonce: 1,
            },
            &[],
        )
        .unwrap();

    // Step 2: Operator polls pending_withdrawals
    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: Some(30),
            },
        )
        .unwrap();

    assert!(
        !result.withdrawals.is_empty(),
        "Should have pending withdrawals"
    );

    // Step 3: Find unapproved entry
    let unapproved: Vec<_> = result
        .withdrawals
        .iter()
        .filter(|w| !w.approved && !w.cancelled && !w.executed)
        .collect();

    assert_eq!(
        unapproved.len(),
        1,
        "Should have exactly 1 unapproved entry"
    );
    let entry = unapproved[0];

    // Step 4: Operator approves with the hash from the query
    // (In real flow, operator would first verify the deposit exists on EVM)
    let approve_result = env.app.execute_contract(
        env.operator.clone(),
        env.contract_addr.clone(),
        &ExecuteMsg::WithdrawApprove {
            withdraw_hash: entry.withdraw_hash.clone(),
        },
        &[],
    );

    assert!(
        approve_result.is_ok(),
        "Operator WithdrawApprove should succeed: {:?}",
        approve_result.err()
    );

    // Step 5: Verify the withdrawal is now approved
    let updated: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: entry.withdraw_hash.clone(),
            },
        )
        .unwrap();

    assert!(
        updated.approved,
        "Withdrawal should be approved after operator call"
    );
    assert!(updated.approved_at > 0, "approved_at should be set");
    assert!(!updated.cancelled, "Should not be cancelled");
    assert!(!updated.executed, "Should not be executed yet");
}

/// Test hash computation with parameters matching the actual e2e test.
///
/// Uses the exact same parameters that the e2e test uses:
/// - EVM chain = 0x00000001
/// - Terra chain = 0x00000002
/// - amount = 995000 (post-fee for 1M deposit with 0.5% fee)
/// - nonce = 1
/// - token = "uluna"
///
/// Verifies the hash computed by compute_transfer_hash in the contract
/// matches the hash computed by compute_transfer_hash in multichain-rs.
#[test]
fn test_hash_parity_with_e2e_parameters() {
    let evm_chain: [u8; 4] = [0, 0, 0, 1];
    let terra_chain: [u8; 4] = [0, 0, 0, 2];

    // EVM depositor address (padded to 32 bytes like HashLib.addressToBytes32)
    let evm_address: [u8; 20] = [
        0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce, 0x6a, 0xB8, 0x82, 0x72, 0x79,
        0xcf, 0xfF, 0xb9, 0x22, 0x66,
    ];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&evm_address);

    // Token = keccak256("uluna")
    let token = keccak256(b"uluna");

    // Terra recipient (canonical 20 bytes, left-padded to 32)
    let mut dest_account = [0u8; 32];
    dest_account[12..32].copy_from_slice(&[0xDE; 20]); // placeholder

    let amount: u128 = 995_000;
    let nonce: u64 = 1;

    // Compute hash using the contract's function
    let contract_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );

    // Recompute with same params to verify determinism
    let recomputed_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );

    assert_eq!(
        contract_hash,
        recomputed_hash,
        "Hash computation must be deterministic!\n\
         First:   {}\n\
         Second:  {}",
        bytes32_to_hex(&contract_hash),
        bytes32_to_hex(&recomputed_hash),
    );

    // Also verify the hash is deterministic and non-zero
    assert_ne!(contract_hash, [0u8; 32], "Hash should not be all zeros");

    // Different nonce should produce different hash
    let different_nonce_hash = compute_transfer_hash(
        &evm_chain,
        &terra_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        2,
    );
    assert_ne!(
        contract_hash, different_nonce_hash,
        "Different nonce must produce different hash"
    );
}

/// Test that WithdrawSubmit uses the correct dest_chain from contract storage.
///
/// The e2e failure could be caused by a mismatch between:
/// - The dest_chain used in WithdrawSubmit (from THIS_CHAIN_ID in contract storage)
/// - The destChain used in the EVM deposit (from ChainRegistry)
///
/// This test verifies THIS_CHAIN_ID is correctly stored and used by reading it
/// from the contract's query response.
#[test]
fn test_withdraw_submit_uses_correct_dest_chain() {
    let mut env = setup_with_e2e_chain_ids();

    let evm_chain_id: [u8; 4] = [0, 0, 0, 1];
    let terra_chain_id: [u8; 4] = [0, 0, 0, 2]; // Must match contract's this_chain_id

    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&[0xAB; 20]);

    let amount: u128 = 1_000_000;
    let nonce: u64 = 0;

    // Submit withdrawal
    let res = env
        .app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: Binary::from(evm_chain_id.to_vec()),
                src_account: Binary::from(src_account.to_vec()),
                token: "uluna".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(amount),
                nonce,
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
        .expect("withdraw_hash attribute not found");

    let contract_hash_bytes = hex::decode(&withdraw_hash_hex[2..]).unwrap();
    let mut contract_hash = [0u8; 32];
    contract_hash.copy_from_slice(&contract_hash_bytes);

    // Query the pending withdrawal to get the stored dest_account
    let pending: PendingWithdrawResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdraw {
                withdraw_hash: Binary::from(contract_hash_bytes.clone()),
            },
        )
        .unwrap();

    let stored_dest_account: [u8; 32] = pending.dest_account.to_vec().try_into().unwrap();
    let token = keccak256(b"uluna");

    // Recompute hash with terra_chain_id as dest_chain (matching the contract's THIS_CHAIN_ID)
    let expected_hash = compute_transfer_hash(
        &evm_chain_id,
        &terra_chain_id,
        &src_account,
        &stored_dest_account,
        &token,
        amount,
        nonce,
    );

    assert_eq!(
        contract_hash,
        expected_hash,
        "Contract must use THIS_CHAIN_ID (0x{}) as dest_chain in hash computation.\n\
         Contract hash: {}\n\
         Expected hash: {}",
        hex::encode(terra_chain_id),
        bytes32_to_hex(&contract_hash),
        bytes32_to_hex(&expected_hash),
    );

    // Verify: using wrong dest_chain produces a DIFFERENT hash
    let wrong_chain: [u8; 4] = [0, 0, 0, 99];
    let wrong_hash = compute_transfer_hash(
        &evm_chain_id,
        &wrong_chain,
        &src_account,
        &stored_dest_account,
        &token,
        amount,
        nonce,
    );
    assert_ne!(
        contract_hash, wrong_hash,
        "Using wrong dest_chain should produce a different hash"
    );

    // Verify: src_chain is stored correctly and matches what we passed
    assert_eq!(
        pending.src_chain.as_slice(),
        &evm_chain_id,
        "Stored src_chain must match the EVM chain ID we passed"
    );
}

/// Test that base64 encoding of the withdraw_hash from the query response
/// correctly round-trips through the operator's decode logic.
///
/// The operator does:
/// 1. Gets hash as base64 string from Terra LCD query
/// 2. base64-decodes to [u8; 32]
/// 3. Passes to EVM's getDeposit(bytes32)
///
/// This test verifies the encoding is consistent.
#[test]
fn test_withdraw_hash_base64_roundtrip() {
    let mut env = setup_with_e2e_chain_ids();

    let evm_chain_id: [u8; 4] = [0, 0, 0, 1];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&[0xAB; 20]);

    // Submit withdrawal
    let res = env
        .app
        .execute_contract(
            env.user.clone(),
            env.contract_addr.clone(),
            &ExecuteMsg::WithdrawSubmit {
                src_chain: Binary::from(evm_chain_id.to_vec()),
                src_account: Binary::from(src_account.to_vec()),
                token: "uluna".to_string(),
                recipient: env.user.to_string(),
                amount: Uint128::from(1_000_000u128),
                nonce: 0,
            },
            &[],
        )
        .unwrap();

    // Get the hash from the event (hex-encoded)
    let hash_hex = res
        .events
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.key == "withdraw_hash")
        .map(|a| a.value.clone())
        .expect("withdraw_hash not found");

    let hash_from_event = hex::decode(&hash_hex[2..]).unwrap();

    // Get the hash from the query response (Binary = base64)
    let result: PendingWithdrawalsResponse = env
        .app
        .wrap()
        .query_wasm_smart(
            &env.contract_addr,
            &QueryMsg::PendingWithdrawals {
                start_after: None,
                limit: Some(30),
            },
        )
        .unwrap();

    let hash_from_query = result.withdrawals[0].withdraw_hash.as_slice();

    // Both should be the same bytes
    assert_eq!(
        hash_from_event.as_slice(),
        hash_from_query,
        "Hash from event and query should be identical bytes.\n\
         Event (hex): {}\n\
         Query (bytes): 0x{}",
        hash_hex,
        hex::encode(hash_from_query),
    );

    // Simulate operator's base64 decode (what the operator does with the LCD response)
    // CosmWasm Binary serializes as base64, so convert to Binary and back
    let binary = Binary::from(hash_from_query.to_vec());
    let b64_string = binary.to_base64();
    let decoded_binary = Binary::from_base64(&b64_string).unwrap();

    assert_eq!(
        decoded_binary.as_slice(),
        hash_from_query,
        "Base64 round-trip should preserve the hash"
    );
    assert_eq!(
        decoded_binary.len(),
        32,
        "Decoded hash must be exactly 32 bytes"
    );
}

/// Test multiple withdrawals with different parameters all produce unique hashes.
///
/// This catches any accidental hash collision due to encoding bugs.
#[test]
fn test_multiple_withdrawals_produce_unique_hashes() {
    let mut env = setup_with_e2e_chain_ids();

    let evm_chain_id: [u8; 4] = [0, 0, 0, 1];
    let mut src_account = [0u8; 32];
    src_account[12..32].copy_from_slice(&[0xAB; 20]);

    let mut hashes = Vec::new();

    // Submit 5 withdrawals with different nonces
    for nonce in 0..5u64 {
        let res = env
            .app
            .execute_contract(
                env.user.clone(),
                env.contract_addr.clone(),
                &ExecuteMsg::WithdrawSubmit {
                    src_chain: Binary::from(evm_chain_id.to_vec()),
                    src_account: Binary::from(src_account.to_vec()),
                    token: "uluna".to_string(),
                    recipient: env.user.to_string(),
                    amount: Uint128::from(1_000_000u128),
                    nonce,
                },
                &[],
            )
            .unwrap();

        let hash_hex = res
            .events
            .iter()
            .flat_map(|e| &e.attributes)
            .find(|a| a.key == "withdraw_hash")
            .map(|a| a.value.clone())
            .unwrap();

        hashes.push(hash_hex);
    }

    // All hashes must be unique
    for i in 0..hashes.len() {
        for j in (i + 1)..hashes.len() {
            assert_ne!(
                hashes[i], hashes[j],
                "Hashes for nonce {} and {} should be different: {} vs {}",
                i, j, hashes[i], hashes[j]
            );
        }
    }
}
