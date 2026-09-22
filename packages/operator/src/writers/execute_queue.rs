//! Shared dest-execute queue helpers (INV-OP-W11 / GL-170).
//!
//! Enumeration re-queue must not reset an in-flight timer. Retryable execute
//! failures stay in `pending_executions` with backoff; only
//! [`super::is_terminal_execute_error`] may enter `terminal_executions`.

use crate::bounded_cache::{BoundedHashCache, BoundedPendingCache};
use crate::poll_config::jittered_exponential_backoff;
use std::time::{Duration, Instant};

/// Solidity `RateLimitExceededPerPeriod(uint256,uint256,uint256)` (not terminal).
pub(crate) const RATE_LIMIT_EXCEEDED_PER_PERIOD_SIG: &str =
    "RateLimitExceededPerPeriod(uint256,uint256,uint256)";
/// Solidity `CancelWindowActive(uint256)` (retryable).
pub(crate) const CANCEL_WINDOW_ACTIVE_SIG: &str = "CancelWindowActive(uint256)";

/// `TokenRegistry.RATE_LIMIT_WINDOW` (24 hours). Used when the window end is unknown.
pub(crate) const TOKEN_REGISTRY_RATE_LIMIT_WINDOW_SECS: u64 = 24 * 60 * 60;

/// How long to wait after a period-full revert (T4). Never uses the short RPC cap.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PeriodWait {
    /// On-chain `windowStart + RATE_LIMIT_WINDOW`.
    Until(u64),
    /// Registry read failed. Wait one full window instead of polling every backoff max.
    FullWindow,
}

/// Pre-send classification of a dest payout against TokenRegistry withdraw limits.
///
/// Temporary period-full stays retryable. Permanent over-max / below-min do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WithdrawRateLimitGate {
    Clear,
    /// `used + amount > maxPerPeriod` inside the current window.
    Wait {
        retry_at_unix: u64,
    },
    BelowMin,
    OverMaxPerTx,
    OverMaxPerPeriod,
}

/// Decimal-normalized payout and TokenRegistry withdraw snapshot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WithdrawLimitSnapshot {
    pub amount: u128,
    pub min_per_tx: u128,
    pub max_per_tx: u128,
    pub max_per_period: u128,
    pub window_start: u64,
    pub used: u128,
    pub now_unix: u64,
    pub window_secs: u64,
}

/// Match `TokenRegistry._checkAndUpdateRateLimit` / `getWithdrawRateLimitWindow`.
///
/// `max == 0` means unlimited. A rolled window (`now >= windowStart + window`) has
/// `used == 0`. Amounts above `maxPerPeriod` stay blocked after the roll.
pub(crate) fn classify_withdraw_rate_limit(s: WithdrawLimitSnapshot) -> WithdrawRateLimitGate {
    if s.min_per_tx != 0 && s.amount < s.min_per_tx {
        return WithdrawRateLimitGate::BelowMin;
    }
    if s.max_per_tx != 0 && s.amount > s.max_per_tx {
        return WithdrawRateLimitGate::OverMaxPerTx;
    }
    if s.max_per_period == 0 {
        return WithdrawRateLimitGate::Clear;
    }
    if s.amount > s.max_per_period {
        return WithdrawRateLimitGate::OverMaxPerPeriod;
    }
    let window_secs = s.window_secs.max(1);
    let rolled = s.window_start == 0 || s.now_unix >= s.window_start.saturating_add(window_secs);
    if rolled {
        return WithdrawRateLimitGate::Clear;
    }
    if s.used.saturating_add(s.amount) > s.max_per_period {
        return WithdrawRateLimitGate::Wait {
            retry_at_unix: s.window_start.saturating_add(window_secs),
        };
    }
    WithdrawRateLimitGate::Clear
}

/// Keccak-256 first 4 bytes as `0x` hex (Solidity error selector).
pub(crate) fn solidity_error_selector(sig: &str) -> String {
    use tiny_keccak::{Hasher, Keccak};
    let mut hasher = Keccak::v256();
    hasher.update(sig.as_bytes());
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    format!("0x{}", hex::encode(&out[..4]))
}

pub(crate) fn is_period_rate_limit_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("ratelimitexceededperperiod")
        || lower.contains(&solidity_error_selector(RATE_LIMIT_EXCEEDED_PER_PERIOD_SIG))
}

/// GuardBridge `WithdrawRateLimitExceeded(address,uint256,uint256,uint256)`.
///
/// Same 24h window as TokenRegistry, but a different revert. Not terminal.
pub(crate) fn is_guard_withdraw_rate_limit_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("withdrawratelimitexceeded")
        || lower.contains(&solidity_error_selector(
            "WithdrawRateLimitExceeded(address,uint256,uint256,uint256)",
        ))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn is_cancel_window_active_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("cancelwindowactive")
        || lower.contains(&solidity_error_selector(CANCEL_WINDOW_ACTIVE_SIG))
}

/// Insert into the execute queue unless the hash is already queued or terminal.
///
/// Returns `true` when a new entry was inserted. Does not replace an existing
/// pending timer, including a TTL-hidden row that is still stored (A4 / INV-OP-W11).
/// Evicted rows (gone from the map) are inserted again from enumeration.
pub(crate) fn enqueue_execution_if_absent<T>(
    terminal: &BoundedHashCache,
    pending: &mut BoundedPendingCache<T>,
    xchain_hash_id: [u8; 32],
    build: impl FnOnce() -> T,
) -> bool {
    if terminal.contains_key(&xchain_hash_id) {
        return false;
    }
    if pending.contains_stored(&xchain_hash_id) {
        return false;
    }
    pending.insert(xchain_hash_id, build());
    true
}

#[derive(Clone, Copy)]
pub(crate) struct ExecuteRetryBackoff {
    pub initial: Duration,
    pub max: Duration,
    pub jitter_bps: u32,
}

/// Seconds to wait before the next `withdrawExecute*` after a retryable failure.
///
/// Period-full waits until `period` (`windowStart + RATE_LIMIT_WINDOW`, or one
/// full window when the registry read failed). It does **not** use
/// `rpc_backoff_max` (T4). `CancelWindowActive` and transport errors use short
/// exponential backoff. First retry (`attempts == 1`) waits `initial`.
pub(crate) fn execute_retry_delay_secs(
    attempts: u32,
    backoff: ExecuteRetryBackoff,
    seed: u64,
    now_unix: u64,
    period: Option<PeriodWait>,
) -> u64 {
    if let Some(period) = period {
        return match period {
            PeriodWait::Until(end) => end.saturating_sub(now_unix).max(1),
            PeriodWait::FullWindow => TOKEN_REGISTRY_RATE_LIMIT_WINDOW_SECS.max(1),
        };
    }
    // CancelWindowActive shares the short exponential path with RPC (not terminal).
    let exp_attempt = attempts.saturating_sub(1);
    jittered_exponential_backoff(
        exp_attempt,
        backoff.initial,
        backoff.max,
        backoff.jitter_bps,
        seed,
    )
    .as_secs()
    .max(1)
}

/// Increment attempts and schedule the next execute try from *now*.
///
/// Resets the local timer only after a failed send (not on enumeration
/// re-queue). On-chain `approvedAt + cancelWindow` is unchanged.
pub(crate) fn apply_execute_retry_backoff(
    attempts: &mut u32,
    approved_at: &mut Instant,
    delay_seconds: &mut u64,
    backoff: ExecuteRetryBackoff,
    seed: u64,
    now_unix: u64,
    period: Option<PeriodWait>,
) {
    *attempts = attempts.saturating_add(1);
    *approved_at = Instant::now();
    *delay_seconds = execute_retry_delay_secs(*attempts, backoff, seed, now_unix, period);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::writers::{is_terminal_execute_error, BELOW_MIN_PER_TX_SELECTOR};

    fn backoff(initial: u64, max: u64) -> ExecuteRetryBackoff {
        ExecuteRetryBackoff {
            initial: Duration::from_secs(initial),
            max: Duration::from_secs(max),
            jitter_bps: 0,
        }
    }

    #[test]
    fn enqueue_skips_terminal_and_does_not_reset_delay() {
        let mut terminal = BoundedHashCache::new(16, 3600);
        let mut pending = BoundedPendingCache::new(16, 3600);
        let hash = [7u8; 32];

        assert!(enqueue_execution_if_absent(
            &terminal,
            &mut pending,
            hash,
            || 100u64
        ));
        assert_eq!(pending.get(&hash), Some(&100u64));

        assert!(!enqueue_execution_if_absent(
            &terminal,
            &mut pending,
            hash,
            || 0u64
        ));
        assert_eq!(
            pending.get(&hash),
            Some(&100u64),
            "in-flight delay must not reset on re-queue"
        );

        terminal.insert(hash);
        let other = [8u8; 32];
        terminal.insert(other);
        assert!(!enqueue_execution_if_absent(
            &terminal,
            &mut pending,
            other,
            || 1u64
        ));
        assert!(pending.get(&other).is_none());
    }

    #[test]
    fn period_rate_limit_and_cancel_window_are_not_terminal() {
        let period_sel = solidity_error_selector(RATE_LIMIT_EXCEEDED_PER_PERIOD_SIG);
        let cancel_sel = solidity_error_selector(CANCEL_WINDOW_ACTIVE_SIG);
        assert!(!is_terminal_execute_error("RateLimitExceededPerPeriod"));
        assert!(!is_terminal_execute_error(&format!(
            "execution reverted, data: \"{period_sel}\""
        )));
        assert!(!is_terminal_execute_error("CancelWindowActive"));
        assert!(!is_terminal_execute_error(&format!(
            "execution reverted, data: \"{cancel_sel}\""
        )));
        assert!(!is_period_rate_limit_error("CancelWindowActive"));
        assert!(is_period_rate_limit_error(&format!(
            "Failed to send withdraw tx: {period_sel}"
        )));
        assert!(is_cancel_window_active_error(&format!(
            "Failed to send withdraw tx: {cancel_sel}"
        )));
        let guard_sel =
            solidity_error_selector("WithdrawRateLimitExceeded(address,uint256,uint256,uint256)");
        assert!(is_guard_withdraw_rate_limit_error(&format!(
            "execution reverted, data: \"{guard_sel}\""
        )));
        assert!(!is_period_rate_limit_error(&guard_sel));
        assert!(!is_terminal_execute_error("WithdrawRateLimitExceeded"));
        assert!(is_terminal_execute_error(&format!(
            "execution reverted, data: \"{BELOW_MIN_PER_TX_SELECTOR}\""
        )));
    }

    #[test]
    fn retry_delay_period_waits_until_window_end_not_rpc_cap() {
        let b = backoff(2, 8);
        let now = 1_700_000_000u64;
        let end = now + 3_600;
        let period = execute_retry_delay_secs(1, b, 1, now, Some(PeriodWait::Until(end)));
        assert_eq!(period, 3_600);
        let unknown = execute_retry_delay_secs(1, b, 1, now, Some(PeriodWait::FullWindow));
        assert_eq!(unknown, TOKEN_REGISTRY_RATE_LIMIT_WINDOW_SECS);
        assert_ne!(period, b.max.as_secs());
        let cancel = execute_retry_delay_secs(1, b, 1, now, None);
        assert_eq!(cancel, 2);
        let rpc = execute_retry_delay_secs(1, b, 1, now, None);
        assert_eq!(rpc, 2);
        let later = execute_retry_delay_secs(3, b, 1, now, None);
        assert_eq!(later, 8);
    }

    fn snap(amount: u128, used: u128, max_per_period: u128) -> WithdrawLimitSnapshot {
        WithdrawLimitSnapshot {
            amount,
            min_per_tx: 10,
            max_per_tx: 1_000,
            max_per_period,
            window_start: 1_000,
            used,
            now_unix: 1_100,
            window_secs: 86_400,
        }
    }

    #[test]
    fn classify_period_full_waits_and_over_max_is_permanent() {
        let wait = classify_withdraw_rate_limit(snap(50, 980, 1_000));
        assert_eq!(
            wait,
            WithdrawRateLimitGate::Wait {
                retry_at_unix: 1_000 + 86_400
            }
        );
        let mut over_period = snap(1_001, 0, 1_000);
        over_period.max_per_tx = 10_000;
        assert_eq!(
            classify_withdraw_rate_limit(over_period),
            WithdrawRateLimitGate::OverMaxPerPeriod
        );
        let mut cleared = snap(20, 0, 1_000);
        cleared.now_unix = 1_000 + 86_400;
        assert_eq!(
            classify_withdraw_rate_limit(cleared),
            WithdrawRateLimitGate::Clear
        );
        let mut below = snap(5, 0, 1_000);
        below.amount = 5;
        assert_eq!(
            classify_withdraw_rate_limit(below),
            WithdrawRateLimitGate::BelowMin
        );
        let mut per_tx = snap(20, 0, 10_000);
        per_tx.amount = 1_001;
        assert_eq!(
            classify_withdraw_rate_limit(per_tx),
            WithdrawRateLimitGate::OverMaxPerTx
        );
        let mut unlimited = snap(5_000, 0, 0);
        unlimited.min_per_tx = 0;
        unlimited.max_per_tx = 0;
        assert_eq!(
            classify_withdraw_rate_limit(unlimited),
            WithdrawRateLimitGate::Clear
        );
    }

    #[test]
    fn apply_backoff_increments_attempts_and_sets_delay() {
        let mut attempts = 0u32;
        let mut approved_at = Instant::now() - Duration::from_secs(300);
        let mut delay = 0u64;
        apply_execute_retry_backoff(
            &mut attempts,
            &mut approved_at,
            &mut delay,
            backoff(2, 8),
            42,
            1_700_000_000,
            None,
        );
        assert_eq!(attempts, 1);
        assert_eq!(delay, 2);
        assert!(approved_at.elapsed() < Duration::from_secs(2));
    }
}
