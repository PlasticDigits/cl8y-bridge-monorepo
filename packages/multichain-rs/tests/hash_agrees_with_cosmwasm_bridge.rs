//! V2 digest: `multichain-rs` must match CosmWasm `bridge::hash` (Terra contract).

use bridge::hash::compute_xchain_hash_id as cw_hash;
use multichain_rs::hash::compute_xchain_hash_id as mc_hash;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn proptest_multichain_rs_agrees_with_cosmwasm_bridge(
        sc in prop::array::uniform4(any::<u8>()),
        dc in prop::array::uniform4(any::<u8>()),
        sa in prop::array::uniform32(any::<u8>()),
        da in prop::array::uniform32(any::<u8>()),
        tok in prop::array::uniform32(any::<u8>()),
        amount in any::<u128>(),
        nonce in any::<u64>(),
    ) {
        let a = mc_hash(&sc, &dc, &sa, &da, &tok, amount, nonce);
        let b = cw_hash(&sc, &dc, &sa, &da, &tok, amount, nonce);
        prop_assert_eq!(a, b);
    }
}
