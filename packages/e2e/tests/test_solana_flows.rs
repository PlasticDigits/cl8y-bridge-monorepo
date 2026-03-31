//! Solana bridge **offline** checks (no RPC, no `#[ignore]`).
//! **INV-H1** — see `docs/SOLANA_BRIDGE_INVARIANTS.md`.
//!
//! Full on-chain flows (deposit → withdraw_submit → withdraw_approve with `NonceUsed` → execute)
//! live in [`packages/contracts-solana/tests/deposit_withdraw.test.ts`](../../contracts-solana/tests/deposit_withdraw.test.ts)
//! (`anchor test`). The Rust E2E binary tests live integration in
//! [`packages/e2e/src/tests/canceler_solana_destination.rs`](../src/tests/canceler_solana_destination.rs) when services run.

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;

/// Anchor workspace default in `packages/contracts-solana` (must match `declare_id!` / deploy keypair).
const DEFAULT_SOLANA_PROGRAM_ID: &str = "mYwQnKWjsX86Tmr2muoj19QtL1gmfX4iq5jZjCdG8Tb";

fn get_program_id() -> Pubkey {
    let s = std::env::var("SOLANA_PROGRAM_ID")
        .unwrap_or_else(|_| DEFAULT_SOLANA_PROGRAM_ID.to_string());
    Pubkey::from_str(s.trim()).expect("SOLANA_PROGRAM_ID must be a valid base58 pubkey")
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
}

/// Same 32-byte digests as `HashLib.t.sol` `test_DepositWithdraw_*` and
/// `cl8y_bridge::hash::tests::evm_vector_*` (INV-H1).
#[test]
fn test_hash_goldens_match_hashlib_t_sol() {
    use multichain_rs::hash::compute_xchain_hash_id;

    fn hex_b32(s: &str) -> [u8; 32] {
        let s = s.trim_start_matches("0x");
        assert_eq!(s.len(), 64, "expected 32-byte hex");
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("hex");
        }
        out
    }

    let evm_to_evm = compute_xchain_hash_id(
        &1u32.to_be_bytes(),
        &56u32.to_be_bytes(),
        &hex_b32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        &hex_b32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
        &hex_b32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
        1_000_000_000_000_000_000u128,
        42,
    );
    assert_eq!(
        evm_to_evm,
        hex_b32("11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54")
    );

    let evm_to_terra_uluna = compute_xchain_hash_id(
        &1u32.to_be_bytes(),
        &2u32.to_be_bytes(),
        &hex_b32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        &hex_b32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        &hex_b32("56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da"),
        995_000,
        1,
    );
    assert_eq!(
        evm_to_terra_uluna,
        hex_b32("92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453")
    );

    let cw20 = hex_b32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d");
    let evm_to_terra_cw20 = compute_xchain_hash_id(
        &1u32.to_be_bytes(),
        &2u32.to_be_bytes(),
        &hex_b32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        &cw20,
        &cw20,
        1_000_000,
        5,
    );
    assert_eq!(
        evm_to_terra_cw20,
        hex_b32("1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b")
    );

    let terra_to_evm_native = compute_xchain_hash_id(
        &2u32.to_be_bytes(),
        &1u32.to_be_bytes(),
        &hex_b32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        &hex_b32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        &hex_b32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
        500_000,
        3,
    );
    assert_eq!(
        terra_to_evm_native,
        hex_b32("076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438")
    );

    let terra_to_evm_cw20 = compute_xchain_hash_id(
        &2u32.to_be_bytes(),
        &1u32.to_be_bytes(),
        &hex_b32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        &hex_b32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
        &hex_b32("000000000000000000000000e7f1725e7734ce288f8367e1bb143e90bb3f0512"),
        2_500_000,
        7,
    );
    assert_eq!(
        terra_to_evm_cw20,
        hex_b32("f1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab")
    );
}

#[test]
fn test_pda_derivation_consistency() {
    let program_id = get_program_id();

    let (pda1, bump1) = derive_bridge_pda(&program_id);
    let (pda2, bump2) = derive_bridge_pda(&program_id);
    assert_eq!(pda1, pda2);
    assert_eq!(bump1, bump2);

    let (deposit_1, _) = derive_deposit_pda(&program_id, 1);
    let (deposit_2, _) = derive_deposit_pda(&program_id, 2);
    assert_ne!(deposit_1, deposit_2);

    let hash_a = [0xAAu8; 32];
    let hash_b = [0xBBu8; 32];
    let (pw_a, _) = derive_pending_withdraw_pda(&program_id, &hash_a);
    let (pw_b, _) = derive_pending_withdraw_pda(&program_id, &hash_b);
    assert_ne!(pw_a, pw_b);

    let canceler1 = Keypair::new();
    let canceler2 = Keypair::new();
    let (ce_1, _) = derive_canceler_entry_pda(&program_id, &canceler1.pubkey());
    let (ce_2, _) = derive_canceler_entry_pda(&program_id, &canceler2.pubkey());
    assert_ne!(ce_1, ce_2);
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

    let bytes = addr.to_bytes();
    assert_eq!(bytes.len(), 36);
    let recovered = UniversalAddress::from_bytes(&bytes).unwrap();
    assert_eq!(recovered.raw_address_bytes(), &pubkey[..]);

    let b58 = addr.to_solana_string().unwrap();
    let recovered_b58 = UniversalAddress::from_solana_base58(&b58).unwrap();
    assert_eq!(recovered_b58.raw_address_bytes(), &pubkey[..]);
}
