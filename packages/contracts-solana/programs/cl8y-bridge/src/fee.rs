//! Deposit fee split: `fee = floor(amount * fee_bps / 10000)`, `net = amount - fee`.
//! Used by [`instructions::deposit_native`](crate::instructions::deposit_native) and
//! [`instructions::deposit_spl`](crate::instructions::deposit_spl). On-chain `fee_bps` is capped at 100
//! in [`initialize`](crate::instructions::initialize) and [`set_config`](crate::instructions::set_config).

use crate::error::BridgeError;
use anchor_lang::prelude::*;

/// Returns `(fee, net_amount)` for a deposit gross `amount`.
pub(crate) fn deposit_fee_and_net(amount: u64, fee_bps: u16) -> Result<(u64, u64)> {
    let fee = (amount as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(BridgeError::ArithmeticOverflow)?
        .checked_div(10000)
        .ok_or(BridgeError::ArithmeticOverflow)? as u64;
    let net_amount = amount
        .checked_sub(fee)
        .ok_or(BridgeError::FeeExceedsAmount)?;
    Ok((fee, net_amount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]

        #[test]
        fn fee_plus_net_equals_gross(amount in 1u64..=u64::MAX, fee_bps in 0u16..=10_000u16) {
            let (fee, net) = deposit_fee_and_net(amount, fee_bps).unwrap();
            prop_assert_eq!(fee + net, amount);
            prop_assert!(net <= amount);
            prop_assert!(fee <= amount);
        }

        /// Matches on-chain cap: `fee_bps <= 100` (1%).
        #[test]
        fn onchain_bps_cap_invariants(amount in 1u64..=u64::MAX, fee_bps in 0u16..=100u16) {
            let (fee, net) = deposit_fee_and_net(amount, fee_bps).unwrap();
            prop_assert_eq!(fee + net, amount);
            let expected_fee = ((amount as u128) * (fee_bps as u128) / 10000) as u64;
            prop_assert_eq!(fee, expected_fee);
        }
    }
}
