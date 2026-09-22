//! Exit policy for `cl8y-e2e run` / `full`.
//!
//! Security-critical skips fail the process. Two allowlists are named here
//! (not only in comments):
//!
//! - [`SuiteMode::Quick`] — explicit `--quick` connectivity-only mode.
//! - [`OPTIONAL_SOLANA_LIVE_ALLOWLIST`] — optional Solana live flows when
//!   `SOLANA_ENABLED` is unset. Entries must not be security-critical names.
//!   The live destination fraud case matches `canceler_` and is omitted from
//!   the default required set instead ([`SOLANA_LIVE_OMITTED_UNLESS_ENABLED`]);
//!   a `Skip` of that name still fails.

use super::TestResult;

/// How a suite treats skips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuiteMode {
    /// Default `run` / `full`. Security-critical skips fail the process.
    Required,
    /// Explicit `--quick`. Skips are allowlisted connectivity probes.
    Quick,
}

/// Prefixes that are never allowlisted. A `Skip` whose name starts with one
/// of these is security-critical in [`SuiteMode::Required`].
pub const SECURITY_CRITICAL_SKIP_PREFIXES: &[&str] = &[
    "fraud_detection",
    "canceler_",
    "operator_rejects_unverified",
    "xchain_hash_",
];

/// Exact security-critical names: operator no-approve Solana (issue alias and
/// the suite name) plus watchtower / cancel-window cases.
pub const SECURITY_CRITICAL_SKIP_NAMES: &[&str] = &[
    "operator_no_approve_solana",
    "operator_does_not_approve_unverified_solana_source",
    "evm_time_skip",
    "watchtower_delay_mechanism",
    "withdraw_delay_enforcement",
    "approval_cancellation_blocks_withdraw",
    "withdraw_cancel_during_window",
    "cancelled_approval_blocks_withdrawal",
];

/// Optional Solana live flows. A skip is allowlisted only when the name is
/// in this list, the name is not security-critical, and the reason records
/// that `SOLANA_ENABLED` is unset (GitHub `e2e.yml` does not enable them).
///
/// Security-critical Solana cases are not members. See
/// [`SOLANA_LIVE_OMITTED_UNLESS_ENABLED`].
pub const OPTIONAL_SOLANA_LIVE_ALLOWLIST: &[&str] = &[];

/// Security-critical live Solana cases that the default required set includes
/// only when `SOLANA_ENABLED` is set. When it is unset they are omitted, so
/// they are not a passing skip and are not coverage.
pub const SOLANA_LIVE_OMITTED_UNLESS_ENABLED: &[&str] =
    &["canceler_detects_fraud_on_solana_destination"];

/// `SOLANA_ENABLED=1` or `true` (any case).
pub fn solana_live_enabled(raw: Option<&str>) -> bool {
    match raw.map(str::trim) {
        Some(v) => {
            let l = v.to_ascii_lowercase();
            l == "1" || l == "true"
        }
        None => false,
    }
}

pub fn solana_live_enabled_from_env() -> bool {
    solana_live_enabled(std::env::var("SOLANA_ENABLED").ok().as_deref())
}

/// Whether `name` is part of the default required set.
pub fn default_required_includes_test(name: &str, solana_enabled: bool) -> bool {
    if SOLANA_LIVE_OMITTED_UNLESS_ENABLED.contains(&name) {
        return solana_enabled;
    }
    true
}

pub fn is_security_critical_skip_name(name: &str) -> bool {
    SECURITY_CRITICAL_SKIP_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || SECURITY_CRITICAL_SKIP_NAMES.contains(&name)
}

fn reason_is_solana_disabled(reason: &str) -> bool {
    let r = reason.to_ascii_lowercase();
    r.contains("solana_enabled")
        && (r.contains("not true")
            || r.contains("unset")
            || r.contains("not set")
            || r.contains("is not")
            || r.contains("disabled"))
}

/// Named allowlist: optional Solana live flow whose `SOLANA_ENABLED` gate is unset.
/// Security-critical names always return false.
pub fn is_optional_solana_live_allowlisted(name: &str, reason: &str) -> bool {
    if is_security_critical_skip_name(name) {
        return false;
    }
    OPTIONAL_SOLANA_LIVE_ALLOWLIST.contains(&name) && reason_is_solana_disabled(reason)
}

pub fn is_security_critical_skip(result: &TestResult, mode: SuiteMode) -> bool {
    let TestResult::Skip { name, reason } = result else {
        return false;
    };
    if mode == SuiteMode::Quick {
        return false;
    }
    if is_optional_solana_live_allowlisted(name, reason) {
        return false;
    }
    is_security_critical_skip_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TestResult, TestSuite};
    use std::time::Duration;

    fn suite_with(results: Vec<TestResult>) -> TestSuite {
        let mut suite = TestSuite::new("fixture");
        for result in results {
            suite.add_result(result);
        }
        suite
    }

    #[test]
    fn security_skip_with_zero_fails_exits_nonzero() {
        let suite = suite_with(vec![TestResult::skip("fraud_detection", "env is empty")]);
        assert_eq!(suite.failed(), 0);
        assert!(suite.security_skips(SuiteMode::Required) > 0);
        assert!(suite.should_fail(SuiteMode::Required));
    }

    #[test]
    fn all_pass_exits_zero() {
        let suite = suite_with(vec![TestResult::pass(
            "evm_connectivity",
            Duration::from_millis(1),
        )]);
        assert!(!suite.should_fail(SuiteMode::Required));
        assert!(!suite.should_fail(SuiteMode::Quick));
    }

    #[test]
    fn quick_mode_allowlists_security_skip() {
        let suite = suite_with(vec![TestResult::skip(
            "fraud_detection",
            "not part of explicit --quick",
        )]);
        assert_eq!(suite.failed(), 0);
        assert!(!suite.should_fail(SuiteMode::Quick));
        assert!(suite.should_fail(SuiteMode::Required));
    }

    #[test]
    fn benign_skip_does_not_fail_required_suite() {
        let suite = suite_with(vec![
            TestResult::pass("evm_connectivity", Duration::from_millis(1)),
            TestResult::skip("real_evm1_to_evm2_transfer", "evm2 not configured"),
        ]);
        assert!(!suite.should_fail(SuiteMode::Required));
    }

    #[test]
    fn canceler_solana_source_fraud_skip_is_security_critical() {
        let name = "canceler_solana_source_fraud_detection";
        assert!(is_security_critical_skip_name(name));
        assert!(!is_optional_solana_live_allowlisted(
            name,
            "SOLANA_ENABLED is not true"
        ));
        let suite = suite_with(vec![TestResult::skip(name, "SOLANA_ENABLED is not true")]);
        assert!(suite.should_fail(SuiteMode::Required));
        assert!(default_required_includes_test(name, false));
    }

    #[test]
    fn solana_destination_fraud_omitted_unless_enabled_and_skip_fails() {
        let name = "canceler_detects_fraud_on_solana_destination";
        assert!(is_security_critical_skip_name(name));
        assert!(!default_required_includes_test(name, false));
        assert!(default_required_includes_test(name, true));
        assert!(!is_optional_solana_live_allowlisted(
            name,
            "SOLANA_ENABLED is not true — start canceler with Solana watcher to run this test"
        ));
        let skipped = suite_with(vec![TestResult::skip(
            name,
            "SOLANA_ENABLED is not true — start canceler with Solana watcher to run this test",
        )]);
        assert!(skipped.should_fail(SuiteMode::Required));

        let passing_without_it = suite_with(vec![TestResult::pass(
            "evm_connectivity",
            Duration::from_millis(1),
        )]);
        assert!(!passing_without_it.should_fail(SuiteMode::Required));
    }

    #[test]
    fn allowlist_rejects_security_prefixes_and_names_watchtower() {
        for name in OPTIONAL_SOLANA_LIVE_ALLOWLIST {
            assert!(
                !is_security_critical_skip_name(name),
                "{name} must not be both allowlisted and security-critical"
            );
        }
        assert!(is_security_critical_skip_name("operator_no_approve_solana"));
        assert!(is_security_critical_skip_name(
            "operator_does_not_approve_unverified_solana_source"
        ));
        assert!(is_security_critical_skip_name(
            "operator_rejects_unverified_solana_source"
        ));
        assert!(is_security_critical_skip_name(
            "xchain_hash_id_validity_solana_source"
        ));
        assert!(is_security_critical_skip_name("watchtower_delay_mechanism"));
        assert!(is_security_critical_skip_name(
            "withdraw_cancel_during_window"
        ));
        assert!(is_security_critical_skip_name(
            "cancelled_approval_blocks_withdrawal"
        ));
        assert!(!is_security_critical_skip(
            &TestResult::pass("fraud_detection", Duration::from_millis(1)),
            SuiteMode::Required
        ));
        assert!(!is_security_critical_skip(
            &TestResult::fail("fraud_detection", "boom", Duration::from_millis(1)),
            SuiteMode::Required
        ));
    }

    #[test]
    fn failed_result_still_fails_when_no_skips() {
        let suite = suite_with(vec![TestResult::fail(
            "evm_connectivity",
            "down",
            Duration::from_millis(1),
        )]);
        assert!(suite.should_fail(SuiteMode::Required));
        assert!(suite.should_fail(SuiteMode::Quick));
    }

    #[test]
    fn solana_enabled_parser() {
        assert!(!solana_live_enabled(None));
        assert!(!solana_live_enabled(Some("false")));
        assert!(!solana_live_enabled(Some("")));
        assert!(solana_live_enabled(Some("true")));
        assert!(solana_live_enabled(Some("TRUE")));
        assert!(solana_live_enabled(Some("1")));
    }
}
