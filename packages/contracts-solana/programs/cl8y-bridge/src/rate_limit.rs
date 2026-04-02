//! Withdraw rate limiting aligned with TerraClassic `check_and_update_rate_limit` + EVM `TokenRegistry._checkAndUpdateRateLimit` (withdraw path).
//!
//! - Limits apply to the **payout amount** after `normalize_decimals` (same as Terra execute and EVM bridge withdraw).
//! - 24h window: `RATE_LIMIT_WINDOW_SECS` (86400), same as `RATE_LIMIT_PERIOD` / `RATE_LIMIT_WINDOW` on Terra/EVM.

use crate::error::BridgeError;
use anchor_lang::prelude::{error, Result};

/// 24 hours in seconds (matches Terra `RATE_LIMIT_PERIOD` and EVM `RATE_LIMIT_WINDOW`).
pub const RATE_LIMIT_WINDOW_SECS: i64 = 86_400;

/// When SPL mint supply is unavailable (e.g. native SOL path passes `mint_supply == 0`), use this
/// per-period floor. EVM leaves limits unset when ERC-20 `totalSupply() == 0`; Solana keeps a finite
/// cap here so native payouts are not implicitly uncapped. Operators should still set explicit limits
/// via `set_rate_limit` for production native routes.
pub const DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY: u128 = 100_000_000_000_000_000_000;

/// `minPerTransaction = supply / DEFAULT_MIN_DIVISOR` (0.0001% of supply), matching EVM `TokenRegistry`.
pub const DEFAULT_MIN_DIVISOR: u128 = 1_000_000;
/// `maxPerTransaction` / default `maxPerPeriod` = `supply / DEFAULT_MAX_DIVISOR` (0.01% of supply).
pub const DEFAULT_MAX_DIVISOR: u128 = 10_000;

/// Resolve effective (min, max per tx, max per period) from stored config and optional mint supply.
///
/// - `explicit_config == true`: use stored fields (EVM `setRateLimit` / admin `set_rate_limit`).
/// - `explicit_config == false`: **EVM / Terra registration defaults** — same as EVM `_setDefaultRateLimits`
///   and Terra `add_token` auto `RATE_LIMITS`: `min = supply / 1_000_000`, `max_per_tx = supply / 10_000`,
///   `max_per_period = max_per_tx`. If `mint_supply == 0` (native SOL execute path), only
///   [`DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY`] applies as the period cap; min and per-tx stay zero.
pub fn resolve_effective_limits(
    explicit_config: bool,
    min_per_transaction: u128,
    max_per_transaction: u128,
    max_per_period: u128,
    mint_supply: u128,
) -> (u128, u128, u128) {
    if explicit_config {
        return (
            min_per_transaction,
            max_per_transaction,
            max_per_period,
        );
    }
    if mint_supply == 0 {
        return (0, 0, DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY);
    }
    let min = mint_supply / DEFAULT_MIN_DIVISOR;
    let max_tx = mint_supply / DEFAULT_MAX_DIVISOR;
    let max_period = max_tx;
    (min, max_tx, max_period)
}

/// Enforce limits and update rolling 24h window (Terra + EVM semantics).
pub fn check_and_update_withdraw_rate_limit(
    now_ts: i64,
    payout_amount: u128,
    min_per_tx: u128,
    max_per_tx: u128,
    max_per_period: u128,
    window_start: &mut i64,
    used: &mut u128,
) -> Result<()> {
    if min_per_tx != 0 && payout_amount < min_per_tx {
        return Err(error!(BridgeError::RateLimitBelowMin));
    }
    if max_per_tx != 0 && payout_amount > max_per_tx {
        return Err(error!(BridgeError::RateLimitExceededPerTx));
    }
    // Unlimited period when `max_per_period == 0` (explicit admin choice or implicit tiny supply).
    if max_per_period == 0 {
        return Ok(());
    }

    if *window_start == 0 {
        *window_start = now_ts;
        *used = 0;
    } else if now_ts >= *window_start + RATE_LIMIT_WINDOW_SECS {
        *window_start = now_ts;
        *used = 0;
    }

    let new_used = used
        .checked_add(payout_amount)
        .ok_or(error!(BridgeError::ArithmeticOverflow))?;
    if new_used > max_per_period {
        return Err(error!(BridgeError::RateLimitExceededPerPeriod));
    }
    *used = new_used;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicit_defaults_match_evm_token_registry() {
        // supply 1_000_000: min = 1, max_tx = max_period = 100
        let (min, max_tx, max_p) = resolve_effective_limits(false, 0, 0, 0, 1_000_000);
        assert_eq!(min, 1);
        assert_eq!(max_tx, 100);
        assert_eq!(max_p, 100);
    }

    #[test]
    fn implicit_large_supply_matches_formula() {
        let supply = 10_000_000_000_000u128;
        let (min, max_tx, max_p) = resolve_effective_limits(false, 0, 0, 0, supply);
        assert_eq!(min, supply / DEFAULT_MIN_DIVISOR);
        assert_eq!(max_tx, supply / DEFAULT_MAX_DIVISOR);
        assert_eq!(max_p, max_tx);
    }

    #[test]
    fn implicit_zero_mint_supply_uses_period_floor_only() {
        let (min, max_tx, max_p) = resolve_effective_limits(false, 0, 0, 0, 0);
        assert_eq!(min, 0);
        assert_eq!(max_tx, 0);
        assert_eq!(max_p, DEFAULT_RATE_LIMIT_IF_ZERO_SUPPLY);
    }

    #[test]
    fn explicit_passthrough() {
        let (min, max_tx, max_p) = resolve_effective_limits(true, 5, 100, 1000, 999_999_999);
        assert_eq!((min, max_tx, max_p), (5, 100, 1000));
    }

    #[test]
    fn window_resets_after_24h() {
        let mut ws = 1000i64;
        let mut used = 500u128;
        check_and_update_withdraw_rate_limit(
            1000 + RATE_LIMIT_WINDOW_SECS,
            100u128,
            0,
            0,
            1000,
            &mut ws,
            &mut used,
        )
        .unwrap();
        assert_eq!(ws, 1000 + RATE_LIMIT_WINDOW_SECS);
        assert_eq!(used, 100);
    }

    #[test]
    fn per_period_exceeded() {
        let mut ws = 0i64;
        let mut used = 0u128;
        let err = check_and_update_withdraw_rate_limit(
            10_000,
            600u128,
            0,
            0,
            1000,
            &mut ws,
            &mut used,
        );
        assert!(err.is_ok());
        let err2 = check_and_update_withdraw_rate_limit(
            10_001,
            500u128,
            0,
            0,
            1000,
            &mut ws,
            &mut used,
        );
        assert!(err2.is_err());
    }

    #[test]
    fn max_per_period_zero_is_unlimited() {
        let mut ws = 0i64;
        let mut used = 0u128;
        check_and_update_withdraw_rate_limit(
            1,
            u128::MAX - 1,
            0,
            0,
            0,
            &mut ws,
            &mut used,
        )
        .unwrap();
        assert_eq!(used, 0); // no accounting
    }

    #[test]
    fn min_per_tx_evm_style() {
        let mut ws = 0i64;
        let mut used = 0u128;
        let r = check_and_update_withdraw_rate_limit(1, 4u128, 5, 0, 0, &mut ws, &mut used);
        assert!(r.is_err());
    }

    #[test]
    fn max_per_tx_cap() {
        let mut ws = 0i64;
        let mut used = 0u128;
        let r = check_and_update_withdraw_rate_limit(1, 11u128, 0, 10, 0, &mut ws, &mut used);
        assert!(r.is_err());
    }
}
