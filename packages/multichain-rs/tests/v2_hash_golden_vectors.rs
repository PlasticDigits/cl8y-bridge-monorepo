//! Fixed V2 xchain vectors shared with `HashLib.t.sol` and Solana `hash.rs` golden tests.
//! Confirms multichain-rs, CosmWasm `bridge`, and expected digests agree (no RPC / validator).

use bridge::hash::compute_xchain_hash_id as cw_hash;
use multichain_rs::hash::compute_xchain_hash_id as mc_hash;

fn hex32(s: &str) -> [u8; 32] {
    let s = s.trim_start_matches("0x");
    let mut out = [0u8; 32];
    hex::decode_to_slice(s, &mut out).expect("valid 32-byte hex");
    out
}

fn assert_mc_cw_expected(
    src_chain: [u8; 4],
    dest_chain: [u8; 4],
    src_account: [u8; 32],
    dest_account: [u8; 32],
    token: [u8; 32],
    amount: u128,
    nonce: u64,
    expected_hex: &str,
) {
    let want = hex32(expected_hex);
    let a = mc_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );
    let b = cw_hash(
        &src_chain,
        &dest_chain,
        &src_account,
        &dest_account,
        &token,
        amount,
        nonce,
    );
    assert_eq!(a, want, "multichain-rs digest mismatch");
    assert_eq!(b, want, "CosmWasm bridge digest mismatch");
    assert_eq!(a, b);
}

#[test]
fn golden_evm_to_evm_erc20() {
    assert_mc_cw_expected(
        1u32.to_be_bytes(),
        56u32.to_be_bytes(),
        hex32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        hex32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
        hex32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
        1_000_000_000_000_000_000,
        42,
        "11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54",
    );
}

#[test]
fn golden_evm_to_terra_uluna() {
    assert_mc_cw_expected(
        1u32.to_be_bytes(),
        2u32.to_be_bytes(),
        hex32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        hex32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        hex32("56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da"),
        995_000,
        1,
        "92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453",
    );
}

#[test]
fn golden_evm_to_terra_cw20_token_field() {
    let cw20 = hex32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d");
    assert_mc_cw_expected(
        1u32.to_be_bytes(),
        2u32.to_be_bytes(),
        hex32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        cw20,
        cw20,
        1_000_000,
        5,
        "1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b",
    );
}

#[test]
fn golden_terra_to_evm_native_to_erc20() {
    assert_mc_cw_expected(
        2u32.to_be_bytes(),
        1u32.to_be_bytes(),
        hex32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        hex32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        hex32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
        500_000,
        3,
        "076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438",
    );
}

#[test]
fn golden_terra_to_evm_cw20_to_erc20() {
    assert_mc_cw_expected(
        2u32.to_be_bytes(),
        1u32.to_be_bytes(),
        hex32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        hex32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
        hex32("000000000000000000000000e7f1725e7734ce288f8367e1bb143e90bb3f0512"),
        2_500_000,
        7,
        "f1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab",
    );
}

/// `HashLib.t.sol` `test_TransferHash_TerraToSolana_FullPubkeyDest_CrossChainParity`
#[test]
fn golden_terra_to_solana_full_pubkey_dest() {
    assert_mc_cw_expected(
        2u32.to_be_bytes(),
        5u32.to_be_bytes(),
        hex32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
        hex32("0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"),
        hex32("cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"),
        500_000,
        99,
        "5546e5381d73afc31ae405eea765c2c6c6ead75be0ccbf809cd0ad7be7059f71",
    );
}
