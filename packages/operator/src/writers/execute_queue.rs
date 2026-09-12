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

pub(crate) fn is_cancel_window_active_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("cancelwindowactive")
        || lower.contains(&solidity_error_selector(CANCEL_WINDOW_ACTIVE_SIG))
}

/// Insert into the execute queue unless the hash is already queued or terminal.
///
/// Returns `true` when a new entry was inserted. Does not replace an existing
/// pending timer (A4 / INV-OP-W11).
pub(crate) fn enqueue_execution_if_absent<T>(
    terminal: &BoundedHashCache,
    pending: &mut BoundedPendingCache<T>,
    xchain_hash_id: [u8; 32],
    build: impl FnOnce() -> T,
) -> bool {
    if terminal.contains_key(&xchain_hash_id) {
        return false;
    }
    if pending.get(&xchain_hash_id).is_some() {
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
/// Period-full uses the capped RPC backoff max (retry after the window; never
/// terminal). `CancelWindowActive` and transport errors use short exponential
/// backoff. First retry (`attempts == 1`) waits `initial`.
pub(crate) fn execute_retry_delay_secs(
    attempts: u32,
    err: &str,
    backoff: ExecuteRetryBackoff,
    seed: u64,
) -> u64 {
    if is_period_rate_limit_error(err) {
        return backoff.max.as_secs().max(1);
    }
    // CancelWindowActive shares the short exponential path with RPC (not terminal).
    let _ = is_cancel_window_active_error(err);
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
    err: &str,
    backoff: ExecuteRetryBackoff,
    seed: u64,
) {
    *attempts = attempts.saturating_add(1);
    *approved_at = Instant::now();
    *delay_seconds = execute_retry_delay_secs(*attempts, err, backoff, seed);
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
        assert!(is_terminal_execute_error(&format!(
            "execution reverted, data: \"{BELOW_MIN_PER_TX_SELECTOR}\""
        )));
    }

    #[test]
    fn retry_delay_period_uses_max_cancel_and_rpc_use_short_backoff() {
        let b = backoff(2, 8);
        let period = execute_retry_delay_secs(1, "RateLimitExceededPerPeriod", b, 1);
        assert_eq!(period, 8);
        let cancel = execute_retry_delay_secs(1, "CancelWindowActive", b, 1);
        assert_eq!(cancel, 2);
        let rpc = execute_retry_delay_secs(1, "HTTP error 429", b, 1);
        assert_eq!(rpc, 2);
        let later = execute_retry_delay_secs(3, "HTTP error 429", b, 1);
        assert_eq!(later, 8);
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
            "CancelWindowActive",
            backoff(2, 8),
            42,
        );
        assert_eq!(attempts, 1);
        assert_eq!(delay, 2);
        assert!(approved_at.elapsed() < Duration::from_secs(2));
    }
}
