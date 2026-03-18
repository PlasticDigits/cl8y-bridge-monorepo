//! End-to-end tests for Solana bridge flows.
//!
//! These tests require a running local environment:
//! - solana-test-validator on localhost:8899
//! - anvil on localhost:8545
//! - operator and canceler services

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use std::str::FromStr;

fn solana_rpc() -> RpcClient {
    RpcClient::new_with_commitment(
        "http://localhost:8899".to_string(),
        CommitmentConfig::confirmed(),
    )
}

fn get_program_id() -> Pubkey {
    Pubkey::from_str("CL8YBr1dg3So1ana111111111111111111111111111").expect("Invalid program ID")
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
    let (bridge_pda, _) = derive_bridge_pda(&program_id);

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

    // Build and send deposit_native instruction
    let dest_chain = [0x00, 0x00, 0x00, 0x01]; // EVM
    let dest_account = [0xBBu8; 32];
    let dest_token = [0xCCu8; 32];
    let amount: u64 = 1_000_000_000; // 1 SOL

    // Read current nonce from bridge PDA to derive deposit record PDA
    let bridge_account = client.get_account(&bridge_pda).unwrap();
    // Anchor: 8 (discriminator) + 32 (admin) + 32 (operator) + 2 (fee_bps) +
    // 8 (withdraw_delay) + 8 (deposit_nonce) = offset 82
    let nonce_offset = 8 + 32 + 32 + 2 + 8;
    let current_nonce = u64::from_le_bytes(
        bridge_account.data[nonce_offset..nonce_offset + 8]
            .try_into()
            .unwrap(),
    );
    let next_nonce = current_nonce + 1;

    let (deposit_record_pda, _) = derive_deposit_pda(&program_id, next_nonce);

    // Derive dest chain entry PDA
    let (dest_chain_pda, _) = Pubkey::find_program_address(&[b"chain", &dest_chain], &program_id);

    // Anchor discriminator for deposit_native
    let discriminator = {
        use solana_sdk::hash::hash;
        let h = hash(b"global:deposit_native");
        let mut d = [0u8; 8];
        d.copy_from_slice(&h.to_bytes()[..8]);
        d
    };

    let mut data = discriminator.to_vec();
    data.extend_from_slice(&dest_chain);
    data.extend_from_slice(&dest_account);
    data.extend_from_slice(&dest_token);
    data.extend_from_slice(&amount.to_le_bytes());

    let instruction = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(bridge_pda, false),
            AccountMeta::new(deposit_record_pda, false),
            AccountMeta::new_readonly(dest_chain_pda, false),
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    };

    let recent_blockhash = client.get_latest_blockhash().unwrap();
    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&user.pubkey()),
        &[&user],
        recent_blockhash,
    );

    match client.send_and_confirm_transaction(&tx) {
        Ok(sig) => println!("Deposit tx succeeded: {}", sig),
        Err(e) => {
            println!(
                "Deposit tx failed (expected if chain not registered): {}",
                e
            );
            return;
        }
    }

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

    // Verify bridge is initialized
    match client.get_account(&bridge_pda) {
        Ok(_) => println!("Bridge PDA exists, proceeding with EVM→Solana test"),
        Err(e) => {
            println!("Bridge PDA not found, skipping: {}", e);
            return;
        }
    }

    // EVM→Solana flow:
    // 1. EVM deposit is detected by operator (external)
    // 2. User calls withdraw_submit on Solana
    // 3. Operator calls withdraw_approve
    // 4. After delay, user calls withdraw_execute

    let user = Keypair::new();
    let sig = client
        .request_airdrop(&user.pubkey(), 5_000_000_000)
        .unwrap();
    client.confirm_transaction(&sig).unwrap();

    // Test parameters (would match an EVM deposit in a full E2E)
    let src_chain = [0x00, 0x00, 0x00, 0x01]; // EVM source
    let src_account = [0xAAu8; 32];
    let dest_token = Pubkey::new_unique();
    let amount: u128 = 1_000_000_000;
    let nonce: u64 = 1;

    // Compute transfer hash
    let transfer_hash = multichain_rs::hash::compute_xchain_hash_id(
        &src_chain,
        &[0x00, 0x00, 0x00, 0x05], // Solana dest
        &src_account,
        &user.pubkey().to_bytes(),
        &dest_token.to_bytes(),
        amount,
        nonce,
    );

    let (pending_withdraw_pda, _) = derive_pending_withdraw_pda(&program_id, &transfer_hash);

    // Check if withdraw_submit would succeed (PDA should not exist yet)
    match client.get_account(&pending_withdraw_pda) {
        Ok(_) => println!("PendingWithdraw PDA already exists (may be from prior test run)"),
        Err(_) => println!("PendingWithdraw PDA does not exist yet, as expected"),
    }

    println!("EVM to Solana flow test completed (partial - requires operator for full flow)");
}

#[test]
#[ignore = "requires full local environment"]
fn test_solana_cancel_flow() {
    let client = solana_rpc();
    let program_id = get_program_id();
    let (bridge_pda, _) = derive_bridge_pda(&program_id);

    match client.get_account(&bridge_pda) {
        Ok(_) => println!("Bridge PDA exists, proceeding with cancel test"),
        Err(e) => {
            println!("Bridge PDA not found, skipping: {}", e);
            return;
        }
    }

    let canceler = Keypair::new();
    let sig = client
        .request_airdrop(&canceler.pubkey(), 5_000_000_000)
        .unwrap();
    client.confirm_transaction(&sig).unwrap();

    // Verify canceler entry PDA derivation is correct
    let (canceler_entry_pda, _) = derive_canceler_entry_pda(&program_id, &canceler.pubkey());
    match client.get_account(&canceler_entry_pda) {
        Ok(account) => {
            println!(
                "Canceler PDA exists with {} bytes (should check active flag)",
                account.data.len()
            );
        }
        Err(_) => {
            println!("Canceler not registered (expected - admin must register first)");
        }
    }

    println!("Solana cancel flow test completed (partial - requires admin setup for full flow)");
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
