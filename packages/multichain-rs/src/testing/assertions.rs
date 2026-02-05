//! Common Test Assertions
//!
//! Provides assertion helpers for E2E tests to verify bridge operations.

use crate::types::{ChainId, EvmAddress};
use eyre::{eyre, Result};

/// Assert that two chain IDs are equal
pub fn assert_chain_id_eq(actual: &ChainId, expected: &ChainId) -> Result<()> {
    if actual != expected {
        return Err(eyre!(
            "Chain ID mismatch: expected {}, got {}",
            expected.to_u32(),
            actual.to_u32()
        ));
    }
    Ok(())
}

/// Assert that two EVM addresses are equal
pub fn assert_evm_address_eq(actual: &EvmAddress, expected: &EvmAddress) -> Result<()> {
    if actual != expected {
        return Err(eyre!(
            "EVM address mismatch: expected {}, got {}",
            expected.as_hex(),
            actual.as_hex()
        ));
    }
    Ok(())
}

/// Assert that an amount is within a tolerance of expected
pub fn assert_amount_approx_eq(actual: u128, expected: u128, tolerance: u128) -> Result<()> {
    let diff = if actual > expected {
        actual - expected
    } else {
        expected - actual
    };

    if diff > tolerance {
        return Err(eyre!(
            "Amount mismatch: expected {} ± {}, got {} (diff: {})",
            expected,
            tolerance,
            actual,
            diff
        ));
    }
    Ok(())
}

/// Assert that a hash matches expected
pub fn assert_hash_eq(actual: &[u8; 32], expected: &[u8; 32]) -> Result<()> {
    if actual != expected {
        return Err(eyre!(
            "Hash mismatch: expected 0x{}, got 0x{}",
            hex::encode(expected),
            hex::encode(actual)
        ));
    }
    Ok(())
}

/// Assert that a balance increased by expected amount (with tolerance for fees)
pub fn assert_balance_increased(
    balance_before: u128,
    balance_after: u128,
    expected_increase: u128,
    fee_tolerance: u128,
) -> Result<()> {
    let actual_increase = balance_after.saturating_sub(balance_before);
    let min_increase = expected_increase.saturating_sub(fee_tolerance);
    let max_increase = expected_increase.saturating_add(fee_tolerance);

    if actual_increase < min_increase || actual_increase > max_increase {
        return Err(eyre!(
            "Balance increase mismatch: expected {} ± {}, got {} (before: {}, after: {})",
            expected_increase,
            fee_tolerance,
            actual_increase,
            balance_before,
            balance_after
        ));
    }
    Ok(())
}

/// Assert that a balance decreased by expected amount (with tolerance for fees)
pub fn assert_balance_decreased(
    balance_before: u128,
    balance_after: u128,
    expected_decrease: u128,
    fee_tolerance: u128,
) -> Result<()> {
    let actual_decrease = balance_before.saturating_sub(balance_after);
    let min_decrease = expected_decrease.saturating_sub(fee_tolerance);
    let max_decrease = expected_decrease.saturating_add(fee_tolerance);

    if actual_decrease < min_decrease || actual_decrease > max_decrease {
        return Err(eyre!(
            "Balance decrease mismatch: expected {} ± {}, got {} (before: {}, after: {})",
            expected_decrease,
            fee_tolerance,
            actual_decrease,
            balance_before,
            balance_after
        ));
    }
    Ok(())
}

/// Assert that a withdrawal is in pending state
pub fn assert_withdrawal_pending(approved: bool, cancelled: bool, executed: bool) -> Result<()> {
    if approved || cancelled || executed {
        return Err(eyre!(
            "Expected pending withdrawal, got: approved={}, cancelled={}, executed={}",
            approved,
            cancelled,
            executed
        ));
    }
    Ok(())
}

/// Assert that a withdrawal is approved but not yet executable
pub fn assert_withdrawal_approved(approved: bool, cancelled: bool, executed: bool) -> Result<()> {
    if !approved {
        return Err(eyre!("Expected approved withdrawal, got not approved"));
    }
    if cancelled {
        return Err(eyre!("Expected approved withdrawal, got cancelled"));
    }
    if executed {
        return Err(eyre!("Expected approved withdrawal, got already executed"));
    }
    Ok(())
}

/// Assert that a withdrawal is cancelled
pub fn assert_withdrawal_cancelled(approved: bool, cancelled: bool, executed: bool) -> Result<()> {
    if !cancelled {
        return Err(eyre!(
            "Expected cancelled withdrawal, got: approved={}, cancelled={}, executed={}",
            approved,
            cancelled,
            executed
        ));
    }
    Ok(())
}

/// Assert that a withdrawal is executed
pub fn assert_withdrawal_executed(approved: bool, cancelled: bool, executed: bool) -> Result<()> {
    if !executed {
        return Err(eyre!(
            "Expected executed withdrawal, got: approved={}, cancelled={}, executed={}",
            approved,
            cancelled,
            executed
        ));
    }
    Ok(())
}

/// Wait for a condition to be true, with timeout and polling
pub async fn wait_for_condition<F, Fut>(
    condition_name: &str,
    check_fn: F,
    timeout_secs: u64,
    poll_interval_ms: u64,
) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<bool>>,
{
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let poll_interval = std::time::Duration::from_millis(poll_interval_ms);

    loop {
        match check_fn().await {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(error = %e, "Error checking condition {}", condition_name);
            }
        }

        if start.elapsed() >= timeout {
            return Err(eyre!(
                "Timeout waiting for condition '{}' after {}s",
                condition_name,
                timeout_secs
            ));
        }

        tokio::time::sleep(poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assert_chain_id_eq() {
        let a = ChainId::from_u32(1);
        let b = ChainId::from_u32(1);
        assert!(assert_chain_id_eq(&a, &b).is_ok());

        let c = ChainId::from_u32(2);
        assert!(assert_chain_id_eq(&a, &c).is_err());
    }

    #[test]
    fn test_assert_amount_approx_eq() {
        // Exact match
        assert!(assert_amount_approx_eq(1000, 1000, 0).is_ok());

        // Within tolerance
        assert!(assert_amount_approx_eq(1000, 1010, 100).is_ok());
        assert!(assert_amount_approx_eq(1010, 1000, 100).is_ok());

        // Outside tolerance
        assert!(assert_amount_approx_eq(1000, 1200, 100).is_err());
    }

    #[test]
    fn test_assert_balance_increased() {
        // Exact increase
        assert!(assert_balance_increased(1000, 2000, 1000, 0).is_ok());

        // Increase with fee tolerance
        assert!(assert_balance_increased(1000, 1990, 1000, 100).is_ok());

        // Increase too small
        assert!(assert_balance_increased(1000, 1500, 1000, 100).is_err());
    }

    #[test]
    fn test_assert_withdrawal_states() {
        // Pending
        assert!(assert_withdrawal_pending(false, false, false).is_ok());
        assert!(assert_withdrawal_pending(true, false, false).is_err());

        // Approved
        assert!(assert_withdrawal_approved(true, false, false).is_ok());
        assert!(assert_withdrawal_approved(false, false, false).is_err());

        // Cancelled
        assert!(assert_withdrawal_cancelled(false, true, false).is_ok());
        assert!(assert_withdrawal_cancelled(false, false, false).is_err());

        // Executed
        assert!(assert_withdrawal_executed(true, false, true).is_ok());
        assert!(assert_withdrawal_executed(true, false, false).is_err());
    }
}
