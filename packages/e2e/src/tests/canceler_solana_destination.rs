//! Solana-destination (EVM→Solana) canceler fraud coverage.
//!
//! Requires a running Solana validator, deployed program, operator keypair, and canceler built with
//! `SOLANA_ENABLED=true` (plus `SOLANA_RPC_URL`, `SOLANA_PROGRAM_ID`, `SOLANA_PRIVATE_KEY`, etc.).

use crate::services::{find_project_root, ServiceManager};
use crate::tests::canceler_helpers::check_canceler_health;
use crate::{E2eConfig, TestResult};
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
use std::time::{Duration, Instant};
use tracing::info;

const FRAUD_DETECTION_TIMEOUT: Duration = Duration::from_secs(45);
const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_secs(2);

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let h = hash(preimage.as_bytes());
    let mut d = [0u8; 8];
    d.copy_from_slice(&h.to_bytes()[..8]);
    d
}

fn derive_bridge_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"bridge"], program_id)
}

fn derive_pending_withdraw_pda(program_id: &Pubkey, transfer_hash: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"withdraw", transfer_hash], program_id)
}

fn derive_executed_hash_pda(program_id: &Pubkey, transfer_hash: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"executed", transfer_hash], program_id)
}

fn derive_chain_entry_pda(program_id: &Pubkey, src_chain: &[u8; 4]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"chain", src_chain.as_ref()], program_id)
}

fn derive_token_mapping_pda(
    program_id: &Pubkey,
    src_chain: &[u8; 4],
    src_token: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"token", src_chain.as_ref(), src_token.as_ref()],
        program_id,
    )
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

fn parse_bridge_account(data: &[u8]) -> eyre::Result<BridgeConfigData> {
    if data.len() < 8 {
        return Err(eyre::eyre!("bridge account data too short"));
    }
    BridgeConfigData::try_from_slice(&data[8..]).map_err(|e| eyre::eyre!("borsh bridge: {}", e))
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
    src_decimals: u8,
    dest_decimals: u8,
    operator_gas: u64,
    approved: bool,
    approved_at: i64,
    cancelled: bool,
    executed: bool,
    bump: u8,
}

fn parse_pending_withdraw(data: &[u8]) -> eyre::Result<PendingWithdrawData> {
    if data.len() < 8 {
        return Err(eyre::eyre!("pending withdraw data too short"));
    }
    PendingWithdrawData::try_from_slice(&data[8..])
        .map_err(|e| eyre::eyre!("borsh pending_withdraw: {}", e))
}

#[derive(BorshSerialize)]
struct WithdrawSubmitArgs {
    src_chain: [u8; 4],
    src_account: [u8; 32],
    src_token: [u8; 32],
    dest_token: [u8; 32],
    dest_account: [u8; 32],
    amount: u128,
    nonce: u64,
    operator_gas: u64,
}

#[derive(BorshSerialize)]
struct WithdrawApproveArgs {
    transfer_hash: [u8; 32],
}

fn build_withdraw_submit_ix(
    program_id: Pubkey,
    bridge: Pubkey,
    src_chain_entry: Pubkey,
    token_mapping: Pubkey,
    pending_withdraw: Pubkey,
    executed_hash_check: Pubkey,
    payer: Pubkey,
    args: WithdrawSubmitArgs,
) -> Instruction {
    let mut data = anchor_discriminator("withdraw_submit").to_vec();
    data.extend(args.try_to_vec().expect("borsh"));
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bridge, false),
            AccountMeta::new_readonly(src_chain_entry, false),
            AccountMeta::new_readonly(token_mapping, false),
            AccountMeta::new(pending_withdraw, false),
            AccountMeta::new_readonly(executed_hash_check, false),
            AccountMeta::new(payer, true),
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

fn load_keypair_json(path: &std::path::Path) -> eyre::Result<Keypair> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| eyre::eyre!("read keypair {}: {}", path.display(), e))?;
    let bytes: Vec<u8> =
        serde_json::from_str(&s).map_err(|e| eyre::eyre!("parse keypair json: {}", e))?;
    Keypair::from_bytes(&bytes).map_err(|e| eyre::eyre!("Keypair::from_bytes: {}", e))
}

fn resolve_keypair_for_pubkey(expected: &Pubkey, role: &str) -> eyre::Result<Keypair> {
    let expected_bytes = expected.to_bytes();

    if let Ok(p) = std::env::var("SOLANA_OPERATOR_KEYPAIR") {
        if role == "operator" {
            let kp = load_keypair_json(std::path::Path::new(&p))?;
            if kp.pubkey().to_bytes() == expected_bytes {
                return Ok(kp);
            }
        }
    }

    let default_path = default_wallet_path();
    let kp = load_keypair_json(&default_path)?;
    if kp.pubkey().to_bytes() == expected_bytes {
        return Ok(kp);
    }

    if role == "operator" {
        if let Ok(p) = std::env::var("SOLANA_OPERATOR_KEYPAIR") {
            let kp2 = load_keypair_json(std::path::Path::new(&p))?;
            if kp2.pubkey().to_bytes() == expected_bytes {
                return Ok(kp2);
            }
        }
    }

    Err(eyre::eyre!(
        "{}: need keypair for pubkey {} (try SOLANA_OPERATOR_KEYPAIR or ANCHOR_WALLET matching bridge operator)",
        role,
        expected
    ))
}

/// Transfer SOL from the admin wallet to a test account.
/// Consistent with test_solana_flows.rs — SOL distribution is a setup concern,
/// not an RPC airdrop concern (cl8y_faucet is for test SPL tokens only).
fn fund_from_admin(client: &RpcClient, recipient: &Pubkey, lamports: u64) -> eyre::Result<()> {
    let balance = client.get_balance(recipient).unwrap_or(0);
    if balance >= lamports {
        return Ok(());
    }

    let admin = load_keypair_json(&default_wallet_path())?;
    let ix = solana_sdk::system_instruction::transfer(&admin.pubkey(), recipient, lamports);
    let bh = client
        .get_latest_blockhash()
        .map_err(|e| eyre::eyre!("blockhash: {}", e))?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&admin.pubkey()), &[&admin], bh);
    client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| eyre::eyre!("fund_from_admin transfer: {}", e))?;
    Ok(())
}

fn send_tx(client: &RpcClient, payer: &Keypair, ixs: Vec<Instruction>) -> eyre::Result<()> {
    let bh = client
        .get_latest_blockhash()
        .map_err(|e| eyre::eyre!("blockhash: {}", e))?;
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer.pubkey()), &[payer], bh);
    client
        .send_and_confirm_transaction(&tx)
        .map_err(|e| eyre::eyre!("send_and_confirm_transaction: {}", e))?;
    Ok(())
}

/// EVM→Solana fraudulent `withdraw_approve` (no matching EVM deposit) must be cancelled by the canceler.
pub(super) async fn run_solana_destination_fraud_test(config: &E2eConfig) -> TestResult {
    let start = Instant::now();
    let name = "canceler_detects_fraud_on_solana_destination";

    let solana_enabled = std::env::var("SOLANA_ENABLED")
        .ok()
        .map(|v| {
            let l = v.to_lowercase();
            l == "1" || l == "true"
        })
        .unwrap_or(false);

    if !solana_enabled {
        return TestResult::skip(
            name,
            "SOLANA_ENABLED is not true — start canceler with Solana watcher to run this test",
        );
    }

    let project_root = find_project_root();
    let manager = ServiceManager::new(&project_root);
    if !manager.is_canceler_running() && !check_canceler_health().await {
        return TestResult::skip(name, "Canceler service is not running");
    }

    let rpc_url =
        std::env::var("SOLANA_RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8899".to_string());
    let program_id_str = std::env::var("SOLANA_PROGRAM_ID")
        .unwrap_or_else(|_| "4XX8ndYXupw4Sb4SsRgAPTmBJJjfZbg8rWjj87iKEhVt".to_string());

    let evm_v2: [u8; 4] = config.evm.v2_chain_id.to_be_bytes();

    let pending_pda: Pubkey = match tokio::task::spawn_blocking({
        let rpc_url = rpc_url.clone();
        let program_id_str = program_id_str.clone();
        move || -> eyre::Result<Pubkey> {
            let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
            let _ = client
                .get_version()
                .map_err(|e| eyre::eyre!("Solana RPC unreachable ({}): {}", program_id_str, e))?;

            let program_id = Pubkey::from_str(&program_id_str)
                .map_err(|e| eyre::eyre!("invalid SOLANA_PROGRAM_ID: {}", e))?;
            let (bridge_pda, _) = derive_bridge_pda(&program_id);
            let bridge_account = client.get_account(&bridge_pda).map_err(|e| {
                eyre::eyre!("bridge PDA missing — deploy Solana program first: {}", e)
            })?;
            let bridge_data = parse_bridge_account(&bridge_account.data)?;
            let solana_chain_id = bridge_data.chain_id;

            let operator_pk = Pubkey::new_from_array(bridge_data.operator);
            let operator = resolve_keypair_for_pubkey(&operator_pk, "operator")?;

            let user = Keypair::new();
            fund_from_admin(&client, &user.pubkey(), 10_000_000_000)?;

            // No matching EVM deposit for this nonce / src_account.
            let withdraw_nonce: u64 = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .saturating_add(9_000_000);
            let withdraw_amount: u128 = 100_000_000;
            let src_account = [0x77u8; 32];
            let src_token = [0u8; 32];
            let dest_token = Keypair::new().pubkey();
            let dest_account = user.pubkey().to_bytes();

            let transfer_hash = multichain_rs::hash::compute_xchain_hash_id(
                &evm_v2,
                &solana_chain_id,
                &src_account,
                &dest_account,
                &dest_token.to_bytes(),
                withdraw_amount,
                withdraw_nonce,
            );

            let (pending_withdraw_pda, _) =
                derive_pending_withdraw_pda(&program_id, &transfer_hash);
            let (executed_hash_check_pda, _) =
                derive_executed_hash_pda(&program_id, &transfer_hash);
            let (src_chain_entry_pda, _) = derive_chain_entry_pda(&program_id, &evm_v2);
            let (token_mapping_pda, _) = derive_token_mapping_pda(&program_id, &evm_v2, &src_token);

            let submit_ix = build_withdraw_submit_ix(
                program_id,
                bridge_pda,
                src_chain_entry_pda,
                token_mapping_pda,
                pending_withdraw_pda,
                executed_hash_check_pda,
                user.pubkey(),
                WithdrawSubmitArgs {
                    src_chain: evm_v2,
                    src_account,
                    src_token,
                    dest_token: dest_token.to_bytes(),
                    dest_account,
                    amount: withdraw_amount,
                    nonce: withdraw_nonce,
                    operator_gas: 0,
                },
            );
            send_tx(&client, &user, vec![submit_ix])?;

            let approve_ix = build_withdraw_approve_ix(
                program_id,
                bridge_pda,
                pending_withdraw_pda,
                operator.pubkey(),
                transfer_hash,
            );
            send_tx(&client, &operator, vec![approve_ix])?;

            Ok(pending_withdraw_pda)
        }
    })
    .await
    {
        Ok(Ok(pda)) => pda,
        Ok(Err(e)) => {
            return TestResult::skip(name, format!("Solana setup failed: {}", e));
        }
        Err(e) => {
            return TestResult::fail(
                name,
                format!("Solana task join error: {}", e),
                start.elapsed(),
            );
        }
    };

    info!("Waiting for canceler to cancel fraudulent Solana destination approval...");
    let poll_start = Instant::now();

    while poll_start.elapsed() < FRAUD_DETECTION_TIMEOUT {
        let cancelled: bool = match tokio::task::spawn_blocking({
            let rpc_url = rpc_url.clone();
            let pending_pda = pending_pda;
            move || -> eyre::Result<bool> {
                let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
                let acc = client.get_account(&pending_pda)?;
                let pw = parse_pending_withdraw(&acc.data)?;
                Ok(pw.cancelled)
            }
        })
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(_)) => false,
            Err(_) => false,
        };

        if cancelled {
            info!(
                "Solana destination fraud cancelled in {:?}",
                poll_start.elapsed()
            );
            return TestResult::pass(name, start.elapsed());
        }

        tokio::time::sleep(CANCELLATION_POLL_INTERVAL).await;
    }

    TestResult::fail(
        name,
        "Canceler did not cancel fraudulent Solana destination approval within timeout",
        start.elapsed(),
    )
}
