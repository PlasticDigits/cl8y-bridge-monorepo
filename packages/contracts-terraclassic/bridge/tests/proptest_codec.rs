//! Property tests for `UniversalAddress` and V2 `compute_xchain_hash_id`.
//! Run with `PROPTEST_CASES=4096 cargo test -p bridge proptest_` for heavier fuzzing.

use bridge::address_codec::UniversalAddress;
use bridge::hash::compute_xchain_hash_id;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn proptest_solana_hash_bytes_is_full_pubkey(pubkey in prop::array::uniform32(any::<u8>())) {
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        prop_assert_eq!(addr.to_hash_bytes(), pubkey);
    }

    #[test]
    fn proptest_solana_lossless_bytes_roundtrip(pubkey in prop::array::uniform32(any::<u8>())) {
        let addr = UniversalAddress::from_solana(&pubkey).unwrap();
        let bytes = addr.to_bytes();
        prop_assert_eq!(bytes.len(), 36);
        let back = UniversalAddress::from_bytes(&bytes).unwrap();
        prop_assert_eq!(back.chain_type, bridge::address_codec::CHAIN_TYPE_SOLANA);
        prop_assert_eq!(back.to_hash_bytes(), pubkey);
    }

    #[test]
    fn proptest_solana_lossy_bytes32_collision_only_same_prefix28(
        a in prop::array::uniform32(any::<u8>()),
        b in prop::array::uniform32(any::<u8>()),
    ) {
        let ua = UniversalAddress::from_solana(&a).unwrap();
        let ub = UniversalAddress::from_solana(&b).unwrap();
        if ua.to_bytes32() == ub.to_bytes32() {
            prop_assert_eq!(&a[..28], &b[..28]);
        }
    }

    #[test]
    fn proptest_evm_cosmos_bytes32_strict_roundtrip(
        raw in prop::array::uniform20(any::<u8>()),
        chain in prop::sample::select(vec![1u32, 2u32]),
    ) {
        let addr = UniversalAddress::new(chain, raw).unwrap();
        let b32 = addr.to_bytes32();
        let back = UniversalAddress::from_bytes32_strict(&b32).unwrap();
        prop_assert_eq!(back.chain_type, chain);
        prop_assert_eq!(back.raw_address_bytes(), raw.as_slice());
    }

    #[test]
    fn proptest_xchain_hash_different_dest_changes_hash(
        dest_a in prop::array::uniform32(any::<u8>()),
        dest_b in prop::array::uniform32(any::<u8>()),
    ) {
        prop_assume!(dest_a != dest_b);
        let src_chain = [0u8, 0, 0, 2];
        let dest_chain = [0u8, 0, 0, 5];
        let src_account = [0xABu8; 32];
        let token = [0x11u8; 32];
        let h1 = compute_xchain_hash_id(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_a,
            &token,
            1_000_000,
            1,
        );
        let h2 = compute_xchain_hash_id(
            &src_chain,
            &dest_chain,
            &src_account,
            &dest_b,
            &token,
            1_000_000,
            1,
        );
        prop_assert_ne!(h1, h2);
    }
}
