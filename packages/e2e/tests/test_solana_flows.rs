//! End-to-end tests for Solana bridge flows.
//!
//! These tests require a running local environment:
//! - solana-test-validator on localhost:8899
//! - anvil on localhost:8545
//! - operator and canceler services
//!
//! **Operator keypair:** the bridge `operator` pubkey must be signable locally. Resolution order:
//! 1. `SOLANA_OPERATOR_KEYPAIR` — path to a JSON keypair file whose pubkey matches on-chain `operator`
//! 2. `ANCHOR_WALLET` or `SOLANA_KEYPAIR`, else `~/.config/solana/id.json` — used if its pubkey matches `operator`
//!
//! **Admin keypair:** `ANCHOR_WALLET` / `SOLANA_KEYPAIR` / default Solana CLI path — must match on-chain `admin`
//! for `add_canceler` / `withdraw_reenable`.
//!
//! **Program ID:** set `SOLANA_PROGRAM_ID` to the deployed bridge program (same as `anchor deploy` /
//! `solana-keygen pubkey packages/contracts-solana/target/deploy/cl8y_bridge-keypair.json`).
//! If unset, tests fall back to the workspace placeholder id (only valid when that id is what is deployed).

use borsh::{BorshDeserialize, BorshSerialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};
use std::str::FromStr;
use std::thread;
use std::time::Duration;

const EVM_CHAIN_ID: [u8; 4] = [0x00, 0x00, 0x00, 0x01];

/// Matches `NATIVE_SOL_TOKEN` in `cl8y-bridge` — native SOL withdrawals must use this as `dest_token`.
const NATIVE_SOL_TOKEN_BYTES: [u8; 32] = [0u8; 32];

/// Anchor workspace default in `packages/contracts-solana` (must match `declare_id!` / deploy keypair).
const DEFAULT_SOLANA_PROGRAM_ID: &str = "CL8YBr1dg3So1ana111111111111111111111111111";

const DEFAULT_SOLANA_RPC_URL: &str = "http://localhost:8899";

fn solana_rpc() -> RpcClient {
    let url =
        std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| DEFAULT_SOLANA_RPC_URL.to_string());
    RpcClient::new_with_commitment(url, CommitmentConfig::confirmed())
}

/// Transfer SOL from the admin wallet to a test account.
/// SOL distribution is handled by the setup script (`setup-bridge.sh`),
/// not the validator faucet — the cl8y_faucet program is for test SPL tokens only.
fn fund_from_admin(client: &RpcClient, recipient: &Pubkey, lamports: u64) {
    let balance = client.get_balance(recipient).unwrap_or(0);
    if balance >= lamports {
        return;
    }

    let admin = load_keypair_json(&default_wallet_path());
    let admin_balance = client
        .get_balance(&admin.pubkey())
        .expect("admin wallet must be funded by setup script (run: make deploy)");
    assert!(
        admin_balance >= lamports,
        "Admin wallet {} has {} lamports but {} needed. Run: make deploy (setup-bridge.sh funds SOL)",
        admin.pubkey(),
        admin_balance,
        lamports,
    );

    let ix = solana_sdk::system_instruction::transfer(&admin.pubkey(), recipient, lamports);
    let bh = client.get_latest_blockhash().expect("blockhash");
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&admin.pubkey()), &[&admin], bh);
    client
        .send_and_confirm_transaction(&tx)
        .expect("SOL transfer from admin to test account");
}

fn get_program_id() -> Pubkey {
    let s = std::env::var("SOLANA_PROGRAM_ID")
        .unwrap_or_else(|_| DEFAULT_SOLANA_PROGRAM_ID.to_string());
    Pubkey::from_str(s.trim()).unwrap_or_else(|e| {
        panic!(
            "Invalid SOLANA_PROGRAM_ID {:?}: {}. Set to your deployed program id (see packages/contracts-solana/target/deploy/cl8y_bridge-keypair.json).",
            std::env::var("SOLANA_PROGRAM_ID").ok(),
            e
        )
    })
}

fn derive_bridge_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"bridge"], program_id)
}

fn derive_deposit_pda(program_id: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"deposit", &nonce.to_le_bytes()], program_id)
}

fn derive_pending_withdraw_pda(program_id: &Pubkey, transfer_hash: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"withdraw", transfer_hash], program_id)
}

fn derive_canceler_entry_pda(program_id: &Pubkey, canceler: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"canceler", canceler.as_ref()], program_id)
}

fn derive_executed_hash_pda(program_id: &Pubkey, transfer_hash: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"executed", transfer_hash], program_id)
}

fn derive_chain_pda(program_id: &Pubkey, chain_id: &[u8; 4]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"chain", chain_id], program_id)
}

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let h = hash(preimage.as_bytes());
    let mut d = [0u8; 8];
    d.copy_from_slice(&h.to_bytes()[..8]);
    d
}

#[derive(BorshDeserialize)]
#[allow(dead_code)]
struct BridgeConfigData {
    admin: [u8; 32],
    operator: [u8; 32],
    fee_bps: u16,
    withdraw_delay: i64,
    deposit_nonce: u64,
    accrued_native_fees: u64,
    paused: bool,
    chain_id: [u8; 4],
    bump: u8,
}

fn parse_bridge_account(data: &[u8]) -> BridgeConfigData {
    assert!(
        data.len() >= 8,
        "bridge account data too short for discriminator"
    );
    BridgeConfigData::try_from_slice(&data[8..]).expect("borsh decode BridgeConfig")
}

#[derive(BorshDeserialize)]
#[allow(dead_code)]
struct PendingWithdrawData {
    transfer_hash: [u8; 32],
    src_chain: [u8; 4],
    src_account: [u8; 32],
    dest_account: [u8; 32],
    token: [u8; 32],
    amount: u128,
    nonce: u64,
    approved: bool,
    approved_at: i64,
    cancelled: bool,
    executed: bool,
    bump: u8,
}

fn parse_pending_withdraw(data: &[u8]) -> PendingWithdrawData {
    PendingWithdrawData::try_from_slice(&data[8..]).expect("borsh decode PendingWithdraw")
}

fn default_wallet_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("ANCHOR_WALLET") {
        return std::path::PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("SOLANA_KEYPAIR") {
        return std::path::PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config/solana/id.json")
}

fn load_keypair_json(path: &std::path::Path) -> Keypair {
    let s = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read keypair {}: {}", path.display(), e));
    let bytes: Vec<u8> =
        serde_json::from_str(&s).unwrap_or_else(|e| panic!("parse keypair json: {}", e));
    Keypair::from_bytes(&bytes).expect("Keypair::from_bytes")
}

/// Resolve a keypair whose pubkey matches `expected`, or panic with guidance.
fn resolve_keypair_for_pubkey(expected: &Pubkey, role: &str) -> Keypair {
    let expected_bytes = expected.to_bytes();

    if let Ok(p) = std::env::var("SOLANA_OPERATOR_KEYPAIR") {
        if role == "operator" {
            let kp = load_keypair_json(std::path::Path::new(&p));
            if kp.pubkey().to_bytes() == expected_bytes {
                return kp;
            }
        }
    }

    let default_path = default_wallet_path();
    let kp = load_keypair_json(&default_path);
    if kp.pubkey().to_bytes() == expected_bytes {
        return kp;
    }

    if role == "operator" {
        if let Ok(p) = std::env::var("SOLANA_OPERATOR_KEYPAIR") {
            let kp2 = load_keypair_json(std::path::Path::new(&p));
            if kp2.pubkey().to_bytes() == expected_bytes {
                return kp2;
            }
        }
    }

    panic!(
        "{}: need a local keypair for pubkey {}. For operator set SOLANA_OPERATOR_KEYPAIR to a JSON keypair whose pubkey matches the bridge operator, or re-initialize the bridge so operator matches ANCHOR_WALLET ({}).",
        role,
        expected,
        default_path.display()
    );
}

#[derive(BorshSerialize)]
struct WithdrawSubmitArgs {
    src_chain: [u8; 4],
    src_account: [u8; 32],
    dest_token: [u8; 32],
    amount: u128,
    nonce: u64,
}

#[derive(BorshSerialize)]
struct WithdrawApproveArgs {
    transfer_hash: [u8; 32],
}

#[derive(BorshSerialize)]
struct DepositNativeArgs {
    dest_chain: [u8; 4],
    dest_account: [u8; 32],
    dest_token: [u8; 32],
    amount: u64,
}

#[derive(BorshSerialize)]
struct AddCancelerArgs {
    canceler: [u8; 32],
    active: bool,
}

fn build_withdraw_submit_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    pending_withdraw: Pubkey,
    executed_hash_check: Pubkey,
    recipient: Pubkey,
    args: WithdrawSubmitArgs,
) -> Instruction {
    let mut data = anchor_discriminator("withdraw_submit").to_vec();
    data.extend(args.try_to_vec().expect("borsh"));
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(bridge, false),
            AccountMeta::new(pending_withdraw, false),
            AccountMeta::new_readonly(executed_hash_check, false),
            AccountMeta::new(recipient, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn build_withdraw_approve_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    pending_withdraw: Pubkey,
    operator: Pubkey,
    transfer_hash: [u8; 32],
) -> Instruction {
    let mut data = anchor_discriminator("withdraw_approve").to_vec();
    data.extend(
        WithdrawApproveArgs { transfer_hash }
            .try_to_vec()
            .expect("borsh"),
    );
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(bridge, false),
            AccountMeta::new(pending_withdraw, false),
            AccountMeta::new_readonly(operator, true),
        ],
        data,
    }
}

fn build_withdraw_execute_native_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    pending_withdraw: Pubkey,
    executed_hash: Pubkey,
    recipient: Pubkey,
) -> Instruction {
    let data = anchor_discriminator("withdraw_execute_native").to_vec();
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bridge, false),
            AccountMeta::new(pending_withdraw, false),
            AccountMeta::new(executed_hash, false),
            AccountMeta::new(recipient, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn build_deposit_native_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    deposit_record: Pubkey,
    dest_chain_entry: Pubkey,
    depositor: Pubkey,
    args: DepositNativeArgs,
) -> Instruction {
    let mut data = anchor_discriminator("deposit_native").to_vec();
    data.extend(args.try_to_vec().expect("borsh"));
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bridge, false),
            AccountMeta::new(deposit_record, false),
            AccountMeta::new_readonly(dest_chain_entry, false),
            AccountMeta::new(depositor, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn build_add_canceler_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    canceler_entry: Pubkey,
    admin: Pubkey,
    canceler: Pubkey,
    active: bool,
) -> Instruction {
    let mut data = anchor_discriminator("add_canceler").to_vec();
    data.extend(
        AddCancelerArgs {
            canceler: canceler.to_bytes(),
            active,
        }
        .try_to_vec()
        .expect("borsh"),
    );
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(bridge, false),
            AccountMeta::new(canceler_entry, false),
            AccountMeta::new(admin, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

fn build_withdraw_cancel_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    pending_withdraw: Pubkey,
    canceler_entry: Pubkey,
    canceler: Pubkey,
) -> Instruction {
    let data = anchor_discriminator("withdraw_cancel").to_vec();
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(bridge, false),
            AccountMeta::new(pending_withdraw, false),
            AccountMeta::new_readonly(canceler_entry, false),
            AccountMeta::new_readonly(canceler, true),
        ],
        data,
    }
}

fn build_withdraw_reenable_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    pending_withdraw: Pubkey,
    admin: Pubkey,
) -> Instruction {
    let data = anchor_discriminator("withdraw_reenable").to_vec();
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(bridge, false),
            AccountMeta::new(pending_withdraw, false),
            AccountMeta::new_readonly(admin, true),
        ],
        data,
    }
}

fn send_tx(client: &RpcClient, payer: &Keypair, ixs: Vec<Instruction>) {
    let bh = client.get_latest_blockhash().expect("blockhash");
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[payer], bh);
    client
        .send_and_confirm_transaction(&tx)
        .expect("send_and_confirm_transaction");
}

#[test]
#[ignore = "requires local Solana validator"]
fn test_solana_validator_running() {
    let client = solana_rpc();
    let version = client.get_version().expect("Failed to get Solana version");
    println!("Solana validator version: {}", version.solana_core);
    assert!(!version.solana_core.is_empty());
}

/// Verify that the setup script (`setup-bridge.sh`) funded the admin wallet with SOL.
/// SOL distribution is a setup concern — the cl8y_faucet program handles test SPL tokens only.
#[test]
#[ignore = "requires local Solana validator"]
fn test_solana_admin_funded() {
    let client = solana_rpc();
    let admin_path = default_wallet_path();
    let admin = load_keypair_json(&admin_path);

    let balance = client
        .get_balance(&admin.pubkey())
        .expect("Admin wallet balance check failed — is the validator running?");
    println!(
        "Admin wallet {} balance: {} lamports ({:.2} SOL)",
        admin.pubkey(),
        balance,
        balance as f64 / 1_000_000_000.0
    );
    assert!(
        balance >= 1_000_000_000,
        "Admin wallet should have >= 1 SOL (funded by setup-bridge.sh). Got {} lamports. Run: make deploy",
        balance
    );
}

#[test]
#[ignore = "requires deployed Solana program"]
fn test_solana_bridge_program_exists() {
    let client = solana_rpc();
    let program_id = get_program_id();

    let account = client
        .get_account(&program_id)
        .expect("Program account not found");
    assert!(account.executable, "Program should be executable");
}

#[test]
#[ignore = "requires full local environment"]
fn test_solana_to_evm_deposit_flow() {
    let client = solana_rpc();
    let user = Keypair::new();
    fund_from_admin(&client, &user.pubkey(), 10_000_000_000);

    let program_id = get_program_id();
    let (bridge_pda, _) = derive_bridge_pda(&program_id);

    let bridge_account = client
        .get_account(&bridge_pda)
        .expect("Bridge PDA must exist after make deploy / initialization");
    println!("Bridge PDA exists with {} bytes", bridge_account.data.len());

    let bridge_data = parse_bridge_account(&bridge_account.data);
    let (dest_chain_pda, _) = derive_chain_pda(&program_id, &EVM_CHAIN_ID);

    // Build and send deposit_native instruction
    let dest_chain = EVM_CHAIN_ID;
    let dest_account = [0xBBu8; 32];
    let dest_token = [0xCCu8; 32];
    let amount: u64 = 1_000_000_000; // 1 SOL

    let current_nonce = bridge_data.deposit_nonce;
    let next_nonce = current_nonce + 1;

    let (deposit_record_pda, _) = derive_deposit_pda(&program_id, next_nonce);

    let ix = build_deposit_native_ix(
        program_id,
        bridge_pda,
        deposit_record_pda,
        dest_chain_pda,
        user.pubkey(),
        DepositNativeArgs {
            dest_chain,
            dest_account,
            dest_token,
            amount,
        },
    );

    let bh = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&user.pubkey()), &[&user], bh);

    client
        .send_and_confirm_transaction(&tx)
        .expect("deposit_native must succeed (EVM chain must be registered on Solana)");

    // Verify deposit record PDA was created
    let deposit_account = client
        .get_account(&deposit_record_pda)
        .expect("Deposit record PDA not found after deposit");
    assert!(
        deposit_account.data.len() >= 8 + 32,
        "Deposit record should contain transfer_hash"
    );

    println!("Solana to EVM deposit flow test PASSED");
}

#[test]
#[ignore = "requires full local environment"]
fn test_evm_to_solana_flow() {
    let client = solana_rpc();
    let program_id = get_program_id();
    let (bridge_pda, _) = derive_bridge_pda(&program_id);

    let bridge_account = client
        .get_account(&bridge_pda)
        .expect("Bridge PDA must exist after make deploy / initialization");
    let bridge_data = parse_bridge_account(&bridge_account.data);
    let solana_chain_id = bridge_data.chain_id;
    let withdraw_delay = bridge_data.withdraw_delay.max(0) as u64;

    let operator_pk = Pubkey::new_from_array(bridge_data.operator);
    let operator = resolve_keypair_for_pubkey(&operator_pk, "operator");

    let (dest_chain_pda, _) = derive_chain_pda(&program_id, &EVM_CHAIN_ID);

    let user = Keypair::new();
    fund_from_admin(&client, &user.pubkey(), 10_000_000_000);

    // Fund bridge via deposit_native (same pattern as TS integration tests)
    let fund_amount: u64 = 2_000_000_000;
    let current_nonce = bridge_data.deposit_nonce;
    let fund_nonce = current_nonce + 1;
    let (deposit_record_pda, _) = derive_deposit_pda(&program_id, fund_nonce);
    let deposit_ix = build_deposit_native_ix(
        program_id,
        bridge_pda,
        deposit_record_pda,
        dest_chain_pda,
        user.pubkey(),
        DepositNativeArgs {
            dest_chain: EVM_CHAIN_ID,
            dest_account: [0x11u8; 32],
            dest_token: [0x22u8; 32],
            amount: fund_amount,
        },
    );
    send_tx(&client, &user, vec![deposit_ix]);

    // Unique withdrawal (avoid collision with prior runs)
    let withdraw_nonce: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let withdraw_amount: u128 = 500_000_000;
    let src_account = [0xAAu8; 32];

    let transfer_hash = multichain_rs::hash::compute_xchain_hash_id(
        &EVM_CHAIN_ID,
        &solana_chain_id,
        &src_account,
        &user.pubkey().to_bytes(),
        &NATIVE_SOL_TOKEN_BYTES,
        withdraw_amount,
        withdraw_nonce,
    );

    let (pending_withdraw_pda, _) = derive_pending_withdraw_pda(&program_id, &transfer_hash);
    let (executed_hash_check_pda, _) = derive_executed_hash_pda(&program_id, &transfer_hash);

    let submit_ix = build_withdraw_submit_ix(
        program_id,
        bridge_pda,
        pending_withdraw_pda,
        executed_hash_check_pda,
        user.pubkey(),
        WithdrawSubmitArgs {
            src_chain: EVM_CHAIN_ID,
            src_account,
            dest_token: NATIVE_SOL_TOKEN_BYTES,
            amount: withdraw_amount,
            nonce: withdraw_nonce,
        },
    );
    send_tx(&client, &user, vec![submit_ix]);

    let approve_ix = build_withdraw_approve_ix(
        program_id,
        bridge_pda,
        pending_withdraw_pda,
        operator.pubkey(),
        transfer_hash,
    );
    send_tx(&client, &operator, vec![approve_ix]);

    thread::sleep(Duration::from_secs(withdraw_delay.saturating_add(2)));

    let (executed_hash_pda, _) = derive_executed_hash_pda(&program_id, &transfer_hash);
    let balance_before = client.get_balance(&user.pubkey()).unwrap();

    let exec_ix = build_withdraw_execute_native_ix(
        program_id,
        bridge_pda,
        pending_withdraw_pda,
        executed_hash_pda,
        user.pubkey(),
    );
    send_tx(&client, &user, vec![exec_ix]);

    let balance_after = client.get_balance(&user.pubkey()).unwrap();
    assert!(
        balance_after > balance_before,
        "recipient balance should increase after withdraw_execute_native"
    );

    client
        .get_account(&executed_hash_pda)
        .expect("ExecutedHash PDA must exist after successful execution");

    println!("EVM→Solana withdraw_execute_native flow PASSED");
}

#[test]
#[ignore = "requires full local environment"]
fn test_solana_cancel_flow() {
    let client = solana_rpc();
    let program_id = get_program_id();
    let (bridge_pda, _) = derive_bridge_pda(&program_id);

    let bridge_account = client
        .get_account(&bridge_pda)
        .expect("Bridge PDA must exist after make deploy / initialization");
    let bridge_data = parse_bridge_account(&bridge_account.data);
    let solana_chain_id = bridge_data.chain_id;

    let admin_pk = Pubkey::new_from_array(bridge_data.admin);
    let admin = resolve_keypair_for_pubkey(&admin_pk, "admin");

    let operator_pk = Pubkey::new_from_array(bridge_data.operator);
    let operator = resolve_keypair_for_pubkey(&operator_pk, "operator");

    let canceler = Keypair::new();
    fund_from_admin(&client, &canceler.pubkey(), 5_000_000_000);

    let (canceler_entry_pda, _) = derive_canceler_entry_pda(&program_id, &canceler.pubkey());
    let add_ix = build_add_canceler_ix(
        program_id,
        bridge_pda,
        canceler_entry_pda,
        admin.pubkey(),
        canceler.pubkey(),
        true,
    );
    send_tx(&client, &admin, vec![add_ix]);

    let user = Keypair::new();
    fund_from_admin(&client, &user.pubkey(), 10_000_000_000);

    let (dest_chain_pda, _) = derive_chain_pda(&program_id, &EVM_CHAIN_ID);
    let bridge_refresh =
        parse_bridge_account(&client.get_account(&bridge_pda).expect("bridge").data);
    let fund_nonce = bridge_refresh.deposit_nonce + 1;
    let (deposit_record_pda, _) = derive_deposit_pda(&program_id, fund_nonce);
    send_tx(
        &client,
        &user,
        vec![build_deposit_native_ix(
            program_id,
            bridge_pda,
            deposit_record_pda,
            dest_chain_pda,
            user.pubkey(),
            DepositNativeArgs {
                dest_chain: EVM_CHAIN_ID,
                dest_account: [0x33u8; 32],
                dest_token: [0x44u8; 32],
                amount: 2_000_000_000,
            },
        )],
    );

    let withdraw_nonce: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_add(10_000);
    let withdraw_amount: u128 = 100_000_000;
    let src_account = [0xCCu8; 32];
    let dest_token = Keypair::new().pubkey();
    let transfer_hash = multichain_rs::hash::compute_xchain_hash_id(
        &EVM_CHAIN_ID,
        &solana_chain_id,
        &src_account,
        &user.pubkey().to_bytes(),
        &dest_token.to_bytes(),
        withdraw_amount,
        withdraw_nonce,
    );

    let (pending_withdraw_pda, _) = derive_pending_withdraw_pda(&program_id, &transfer_hash);
    let (executed_check, _) = derive_executed_hash_pda(&program_id, &transfer_hash);

    send_tx(
        &client,
        &user,
        vec![build_withdraw_submit_ix(
            program_id,
            bridge_pda,
            pending_withdraw_pda,
            executed_check,
            user.pubkey(),
            WithdrawSubmitArgs {
                src_chain: EVM_CHAIN_ID,
                src_account,
                dest_token: dest_token.to_bytes(),
                amount: withdraw_amount,
                nonce: withdraw_nonce,
            },
        )],
    );

    send_tx(
        &client,
        &operator,
        vec![build_withdraw_approve_ix(
            program_id,
            bridge_pda,
            pending_withdraw_pda,
            operator.pubkey(),
            transfer_hash,
        )],
    );

    send_tx(
        &client,
        &canceler,
        vec![build_withdraw_cancel_ix(
            program_id,
            bridge_pda,
            pending_withdraw_pda,
            canceler_entry_pda,
            canceler.pubkey(),
        )],
    );

    let pw_data = parse_pending_withdraw(
        &client
            .get_account(&pending_withdraw_pda)
            .expect("pending withdraw")
            .data,
    );
    assert!(pw_data.cancelled, "cancelled flag must be true");

    send_tx(
        &client,
        &admin,
        vec![build_withdraw_reenable_ix(
            program_id,
            bridge_pda,
            pending_withdraw_pda,
            admin.pubkey(),
        )],
    );

    let pw2 = parse_pending_withdraw(
        &client
            .get_account(&pending_withdraw_pda)
            .expect("pending withdraw after reenable")
            .data,
    );
    assert!(
        !pw2.cancelled,
        "cancelled must be false after withdraw_reenable"
    );

    println!("Solana cancel / reenable flow PASSED");
}

#[test]
fn test_hash_parity_offchain() {
    use multichain_rs::hash::compute_xchain_hash_id;

    let src_chain = [0x00, 0x00, 0x00, 0x05]; // Solana
    let dest_chain = [0x00, 0x00, 0x00, 0x01]; // EVM
    let src_account = [0xAAu8; 32];
    let dest_account = [0xBBu8; 32];
    let token = [0xCCu8; 32];
    let amount: u128 = 1_000_000_000;
    let nonce: u64 = 1;

    let hash = compute_xchain_hash_id(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );

    let hash2 = compute_xchain_hash_id(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );
    assert_eq!(hash, hash2, "Hash must be deterministic");

    let hash3 = compute_xchain_hash_id(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount + 1,
        nonce,
    );
    assert_ne!(
        hash, hash3,
        "Different amounts must produce different hashes"
    );

    // Verify different nonces produce different hashes
    let hash4 = compute_xchain_hash_id(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce + 1,
    );
    assert_ne!(
        hash, hash4,
        "Different nonces must produce different hashes"
    );

    // Verify swapping src/dest chains produces different hashes
    let hash5 = compute_xchain_hash_id(
        &dest_chain, // swapped
        &src_chain,  // swapped
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );
    assert_ne!(hash, hash5, "Swapped chains must produce different hashes");

    println!("Cross-chain hash parity test passed");
    println!("Hash: 0x{}", hex::encode(hash));
}

#[test]
fn test_pda_derivation_consistency() {
    let program_id = get_program_id();

    // Bridge PDA should be deterministic
    let (pda1, bump1) = derive_bridge_pda(&program_id);
    let (pda2, bump2) = derive_bridge_pda(&program_id);
    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);

    // Deposit PDAs with different nonces should be different
    let (deposit_1, _) = derive_deposit_pda(&program_id, 1);
    let (deposit_2, _) = derive_deposit_pda(&program_id, 2);
    assert_ne!(deposit_1, deposit_2);

    // PendingWithdraw PDAs with different hashes should be different
    let hash_a = [0xAAu8; 32];
    let hash_b = [0xBBu8; 32];
    let (pw_a, _) = derive_pending_withdraw_pda(&program_id, &hash_a);
    let (pw_b, _) = derive_pending_withdraw_pda(&program_id, &hash_b);
    assert_ne!(pw_a, pw_b);

    // Canceler PDAs with different pubkeys should be different
    let canceler1 = Keypair::new();
    let canceler2 = Keypair::new();
    let (ce_1, _) = derive_canceler_entry_pda(&program_id, &canceler1.pubkey());
    let (ce_2, _) = derive_canceler_entry_pda(&program_id, &canceler2.pubkey());
    assert_ne!(ce_1, ce_2);

    println!("PDA derivation consistency test passed");
}

#[test]
fn test_solana_address_encoding() {
    use multichain_rs::address_codec::{UniversalAddress, CHAIN_TYPE_SOLANA};

    let pubkey = Keypair::new().pubkey().to_bytes();
    let addr = UniversalAddress::from_solana(&pubkey).unwrap();

    assert_eq!(addr.chain_type, CHAIN_TYPE_SOLANA);
    assert!(addr.is_solana());
    assert_eq!(addr.raw_address_bytes(), &pubkey[..]);
    assert_eq!(addr.to_hash_bytes(), pubkey);

    // to_bytes roundtrip (lossless 36-byte encoding)
    let bytes = addr.to_bytes();
    assert_eq!(bytes.len(), 36);
    let recovered = UniversalAddress::from_bytes(&bytes).unwrap();
    assert_eq!(recovered.raw_address_bytes(), &pubkey[..]);

    // base58 roundtrip
    let b58 = addr.to_solana_string().unwrap();
    let recovered_b58 = UniversalAddress::from_solana_base58(&b58).unwrap();
    assert_eq!(recovered_b58.raw_address_bytes(), &pubkey[..]);
}
