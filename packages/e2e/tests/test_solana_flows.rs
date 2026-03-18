//! End-to-end tests for Solana bridge flows.
//!
//! These tests require a running local environment:
//! - solana-test-validator on localhost:8899
//! - anvil on localhost:8545
//! - operator and canceler services

use eyre::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_transaction,
};
use std::str::FromStr;

fn solana_rpc() -> RpcClient {
    RpcClient::new_with_commitment(
        "http://localhost:8899".to_string(),
        CommitmentConfig::confirmed(),
    )
}

fn get_program_id() -> Pubkey {
    // Will be updated after deployment
    Pubkey::from_str("CL8YBr1dg3So1ana111111111111111111111111111").expect("Invalid program ID")
}

#[test]
#[ignore = "requires local Solana validator"]
fn test_solana_validator_running() {
    let client = solana_rpc();
    let version = client.get_version().expect("Failed to get Solana version");
    println!("Solana validator version: {}", version.solana_core);
    assert!(!version.solana_core.is_empty());
}

#[test]
#[ignore = "requires local Solana validator"]
fn test_solana_airdrop() {
    let client = solana_rpc();
    let keypair = Keypair::new();

    let sig = client
        .request_airdrop(&keypair.pubkey(), 1_000_000_000)
        .expect("Airdrop failed");
    client
        .confirm_transaction(&sig)
        .expect("Confirmation failed");

    let balance = client
        .get_balance(&keypair.pubkey())
        .expect("Balance check failed");
    assert_eq!(balance, 1_000_000_000);
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
    let sig = client
        .request_airdrop(&user.pubkey(), 10_000_000_000)
        .unwrap();
    client.confirm_transaction(&sig).unwrap();

    let program_id = get_program_id();
    let (bridge_pda, _) = Pubkey::find_program_address(&[b"bridge"], &program_id);

    match client.get_account(&bridge_pda) {
        Ok(account) => {
            println!("Bridge PDA exists with {} bytes", account.data.len());
        }
        Err(e) => {
            println!(
                "Bridge PDA not found (program may need initialization): {}",
                e
            );
            return;
        }
    }

    // TODO: Build and send deposit_native instruction
    println!("Solana to EVM deposit flow test placeholder");
}

#[test]
#[ignore = "requires full local environment"]
fn test_evm_to_solana_flow() {
    // 1. Deploy EVM deposit
    // 2. Wait for operator to detect
    // 3. Operator submits withdraw_approve on Solana
    // 4. User executes withdrawal after delay
    println!("EVM to Solana flow test placeholder");
}

#[test]
#[ignore = "requires full local environment"]
fn test_solana_cancel_flow() {
    // 1. Create a fraudulent approval on Solana
    // 2. Canceler detects and cancels
    // 3. Verify withdrawal is cancelled
    println!("Solana cancel flow test placeholder");
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

    println!("Cross-chain hash parity test passed");
    println!("Hash: 0x{}", hex::encode(hash));
}
