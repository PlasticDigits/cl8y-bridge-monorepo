//! Decimal normalization matching EVM `Bridge._normalizeDecimals`.

use crate::error::BridgeError;
use anchor_lang::prelude::*;

/// Normalize `amount` from `src_decimals` to `dest_decimals` (same semantics as Solidity).
pub fn normalize_decimals(amount: u128, src_decimals: u8, dest_decimals: u8) -> Result<u128> {
    if src_decimals == dest_decimals {
        return Ok(amount);
    }
    if src_decimals > dest_decimals {
        let exp = (src_decimals - dest_decimals) as u32;
        let div = 10u128.pow(exp);
        amount.checked_div(div).ok_or(BridgeError::ArithmeticOverflow.into())
    } else {
        let exp = (dest_decimals - src_decimals) as u32;
        let mul = 10u128.pow(exp);
        amount.checked_mul(mul).ok_or(BridgeError::ArithmeticOverflow.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Same formula as Solidity `Bridge._normalizeDecimals` (truncating division; checked multiply).
    fn reference_evm(amount: u128, src_decimals: u8, dest_decimals: u8) -> Option<u128> {
        if src_decimals == dest_decimals {
            return Some(amount);
        }
        if src_decimals > dest_decimals {
            let exp = (src_decimals - dest_decimals) as u32;
            let div = 10u128.pow(exp);
            Some(amount / div)
        } else {
            let exp = (dest_decimals - src_decimals) as u32;
            let mul = 10u128.pow(exp);
            amount.checked_mul(mul)
        }
    }

    #[test]
    fn known_vectors_18_to_9() {
        let a = 1_000_000_000_000_000_000u128; // 1e18
        let n = normalize_decimals(a, 18, 9).unwrap();
        assert_eq!(n, 1_000_000_000u128);
    }

    #[test]
    fn known_vectors_6_to_18() {
        let a = 1_000_000u128;
        let n = normalize_decimals(a, 6, 18).unwrap();
        assert_eq!(n, 1_000_000_000_000_000_000u128);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        #[test]
        fn prop_matches_evm_reference(
            amount in any::<u128>(),
            src in 0u8..=18u8,
            dest in 0u8..=18u8,
        ) {
            let got = normalize_decimals(amount, src, dest);
            let expected = reference_evm(amount, src, dest);
            prop_assert_eq!(got.ok(), expected);
        }
    }
}
