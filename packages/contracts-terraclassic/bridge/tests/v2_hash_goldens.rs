//! V2 digest goldens (same vectors as `HashLib.t.sol` / Solana `hash.rs` / multichain-rs).
//! Standalone CosmWasm crate check — no multichain-rs dependency on the Terra workspace.

use bridge::hash::compute_xchain_hash_id;

fn hex32(s: &str) -> [u8; 32] {
    let s = s.trim_start_matches("0x");
    let mut out = [0u8; 32];
    hex::decode_to_slice(s, &mut out).expect("hex");
    out
}

#[test]
fn terra_to_solana_full_pubkey_matches_hashlib() {
    let h = compute_xchain_hash_id(
        &2u32.to_be_bytes(),
        &5u32.to_be_bytes(),
        &hex32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        &hex32("0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"),
        &hex32("cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"),
        500_000,
        99,
    );
    assert_eq!(
        h,
        hex32("5546e5381d73afc31ae405eea765c2c6c6ead75be0ccbf809cd0ad7be7059f71")
    );
}

#[test]
fn evm_to_evm_erc20_matches_hashlib() {
    let h = compute_xchain_hash_id(
        &1u32.to_be_bytes(),
        &56u32.to_be_bytes(),
        &hex32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        &hex32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
        &hex32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
        1_000_000_000_000_000_000,
        42,
    );
    assert_eq!(
        h,
        hex32("11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54")
    );
}
