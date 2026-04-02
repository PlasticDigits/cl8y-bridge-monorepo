use solana_program::keccak;

/// Compute the cross-chain transfer hash matching Solidity `HashLib.computeXchainHashId`
/// (`packages/contracts-evm/src/lib/HashLib.sol`), Rust `compute_xchain_hash_id`
/// (`packages/multichain-rs/src/hash.rs`), and Terra `compute_xchain_hash_id`
/// (`packages/contracts-terraclassic/bridge/src/hash.rs`).
///
/// **Docs:** `docs/crosschain-parity.md`, `docs/SOLANA_BRIDGE_INVARIANTS.md` (INV-H1).
///
/// **Tests:** Vectors in `#[cfg(test)]` below vs `packages/contracts-evm/test/HashLib.t.sol`;
/// TS `packages/contracts-solana/tests/hash_parity.test.ts`; E2E
/// `packages/e2e/tests/test_solana_flows.rs`; CosmWasm parity
/// `packages/multichain-rs/tests/hash_agrees_with_cosmwasm_bridge.rs`.
///
/// Layout: 7 x 32 = 224 bytes (abi.encode compatible)
///   - srcChain:    bytes4 left-aligned in 32-byte slot
///   - destChain:   bytes4 left-aligned in 32-byte slot
///   - srcAccount:  32 bytes
///   - destAccount: 32 bytes
///   - token:       32 bytes (destination token)
///   - amount:      uint256 big-endian (u128 right-aligned in 32-byte slot)
///   - nonce:       uint256 big-endian (u64 right-aligned in 32-byte slot)
pub fn compute_transfer_hash(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    let mut buf = [0u8; 224];

    // srcChain: 4 bytes left-aligned in 32-byte slot
    buf[0..4].copy_from_slice(src_chain);

    // destChain: 4 bytes left-aligned in 32-byte slot
    buf[32..36].copy_from_slice(dest_chain);

    // srcAccount: 32 bytes
    buf[64..96].copy_from_slice(src_account);

    // destAccount: 32 bytes
    buf[96..128].copy_from_slice(dest_account);

    // token: 32 bytes (destination token)
    buf[128..160].copy_from_slice(token);

    // amount: uint256 big-endian, u128 right-aligned in slot (bytes 16..32 of the slot)
    buf[176..192].copy_from_slice(&amount.to_be_bytes());

    // nonce: uint256 big-endian, u64 right-aligned in slot (bytes 24..32 of the slot)
    buf[216..224].copy_from_slice(&nonce.to_be_bytes());

    keccak::hash(&buf).to_bytes()
}

/// Reference Keccak over the same 224-byte layout as `compute_transfer_hash`, using `tiny-keccak`.
/// Must stay in lockstep with `packages/multichain-rs/src/hash.rs` `compute_xchain_hash_id`.
#[cfg(test)]
fn reference_xchain_hash_tiny_keccak(
    src_chain: &[u8; 4],
    dest_chain: &[u8; 4],
    src_account: &[u8; 32],
    dest_account: &[u8; 32],
    token: &[u8; 32],
    amount: u128,
    nonce: u64,
) -> [u8; 32] {
    use tiny_keccak::{Hasher, Keccak};

    let mut data = [0u8; 224];
    data[0..4].copy_from_slice(src_chain);
    data[32..36].copy_from_slice(dest_chain);
    data[64..96].copy_from_slice(src_account);
    data[96..128].copy_from_slice(dest_account);
    data[128..160].copy_from_slice(token);
    let amount_bytes = amount.to_be_bytes();
    data[160 + 16..192].copy_from_slice(&amount_bytes);
    let nonce_bytes = nonce.to_be_bytes();
    data[192 + 24..224].copy_from_slice(&nonce_bytes);

    let mut hasher = Keccak::v256();
    hasher.update(&data);
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn hex_bytes32(s: &str) -> [u8; 32] {
        let s = s.trim_start_matches("0x");
        assert_eq!(s.len(), 64, "expected 32-byte hex");
        let mut out = [0u8; 32];
        for i in 0..32 {
            out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("hex digit");
        }
        out
    }

    /// Golden vectors from `packages/contracts-evm/test/HashLib.t.sol` (must match `main` / EVM).
    #[test]
    fn evm_vector_evm_to_evm_erc20() {
        let h = compute_transfer_hash(
            &1u32.to_be_bytes(),
            &56u32.to_be_bytes(),
            &hex_bytes32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            &hex_bytes32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
            &hex_bytes32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
            1_000_000_000_000_000_000u128,
            42,
        );
        assert_eq!(
            h,
            hex_bytes32("11c90f88a3d48e75a39bc219d261069075a136436ae06b2b571b66a9a600aa54")
        );
    }

    #[test]
    fn evm_vector_evm_to_terra_uluna() {
        let h = compute_transfer_hash(
            &1u32.to_be_bytes(),
            &2u32.to_be_bytes(),
            &hex_bytes32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            &hex_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
            &hex_bytes32("56fa6c6fbc36d8c245b0a852a43eb5d644e8b4c477b27bfab9537c10945939da"),
            995_000,
            1,
        );
        assert_eq!(
            h,
            hex_bytes32("92b16cdec59cb405996f66a9153c364ed635f40f922b518885aa76e5e9c23453")
        );
    }

    #[test]
    fn evm_vector_evm_to_terra_cw20_token_field() {
        let cw20 = hex_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d");
        let h = compute_transfer_hash(
            &1u32.to_be_bytes(),
            &2u32.to_be_bytes(),
            &hex_bytes32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            &cw20,
            &cw20,
            1_000_000,
            5,
        );
        assert_eq!(
            h,
            hex_bytes32("1ec7d94b0f068682032903f83c88fd643d03969e04875ec7ea70f02d1a74db7b")
        );
    }

    #[test]
    fn evm_vector_terra_to_evm_native_to_erc20() {
        let h = compute_transfer_hash(
            &2u32.to_be_bytes(),
            &1u32.to_be_bytes(),
            &hex_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
            &hex_bytes32("000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
            &hex_bytes32("0000000000000000000000005fbdb2315678afecb367f032d93f642f64180aa3"),
            500_000,
            3,
        );
        assert_eq!(
            h,
            hex_bytes32("076a0951bf01eaaf385807d46f1bdfaa4e3f88d7ba77aae03c65871f525a7438")
        );
    }

    #[test]
    fn evm_vector_terra_to_evm_cw20_to_erc20() {
        let h = compute_transfer_hash(
            &2u32.to_be_bytes(),
            &1u32.to_be_bytes(),
            &hex_bytes32("00000000000000000000000035743074956c710800e83198011ccbd4ddf1556d"),
            &hex_bytes32("00000000000000000000000070997970c51812dc3a010c7d01b50e0d17dc79c8"),
            &hex_bytes32("000000000000000000000000e7f1725e7734ce288f8367e1bb143e90bb3f0512"),
            2_500_000,
            7,
        );
        assert_eq!(
            h,
            hex_bytes32("f1ab14494f74acdd3a622cd214e6d0ebde29121309203a6bd7509bf3025c22ab")
        );
    }

    #[test]
    fn max_amount_nonce_matches_reference_impl() {
        let a = u128::MAX;
        let n = u64::MAX;
        let z = [0u8; 32];
        let got = compute_transfer_hash(&[1, 2, 3, 4], &[5, 6, 7, 8], &z, &z, &z, a, n);
        let want = reference_xchain_hash_tiny_keccak(&[1, 2, 3, 4], &[5, 6, 7, 8], &z, &z, &z, a, n);
        assert_eq!(got, want);
    }

    proptest! {
        #[test]
        fn proptest_matches_tiny_keccak_reference(
            sc in prop::array::uniform4(any::<u8>()),
            dc in prop::array::uniform4(any::<u8>()),
            sa in prop::array::uniform32(any::<u8>()),
            da in prop::array::uniform32(any::<u8>()),
            tok in prop::array::uniform32(any::<u8>()),
            amount in any::<u128>(),
            nonce in any::<u64>(),
        ) {
            let got = compute_transfer_hash(&sc, &dc, &sa, &da, &tok, amount, nonce);
            let want = reference_xchain_hash_tiny_keccak(&sc, &dc, &sa, &da, &tok, amount, nonce);
            prop_assert_eq!(got, want);
        }
    }
}
