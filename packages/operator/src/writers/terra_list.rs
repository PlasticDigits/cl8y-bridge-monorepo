//! Terra withdrawal list pagination helpers (GL-139 / INV-TC-AW5).
//!
//! The CosmWasm query caps `pending_withdrawals` / `active_withdrawals` at
//! [`WITHDRAW_LIST_MAX_PAGE`] rows. Clients that request 50 and treat
//! `len < 50` as EOF drop later history. Prefer `next_start_after` and clamp
//! the requested page size to the contract cap.

/// Must match `bridge::active_withdraw::WITHDRAW_LIST_MAX_LIMIT`.
pub const WITHDRAW_LIST_MAX_PAGE: u32 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageAdvance {
    Continue(String),
    Exhausted,
}

/// Clamp a configured/requested page size to the contract cap.
pub fn clamp_page_size(requested: u32) -> u32 {
    requested.clamp(1, WITHDRAW_LIST_MAX_PAGE)
}

/// Decide whether to fetch another Terra list page.
///
/// `next_start_after` from the contract wins (including skip-capped short
/// active pages). An explicit JSON null (`has_next_field` with no cursor)
/// means the range is exhausted. Legacy contracts without the field fall
/// back to "full page of `effective_limit` → continue from last hash".
pub fn advance_withdraw_list_page(
    page_len: usize,
    effective_limit: u32,
    next_start_after: Option<String>,
    has_next_field: bool,
    last_hash: Option<String>,
) -> PageAdvance {
    if let Some(cursor) = next_start_after {
        if !cursor.is_empty() {
            return PageAdvance::Continue(cursor);
        }
    }
    if has_next_field {
        return PageAdvance::Exhausted;
    }
    if page_len == 0 || page_len < effective_limit as usize {
        return PageAdvance::Exhausted;
    }
    match last_hash {
        Some(h) if !h.is_empty() => PageAdvance::Continue(h),
        _ => PageAdvance::Exhausted,
    }
}

pub fn is_operator_approval_candidate(approved: bool, cancelled: bool, executed: bool) -> bool {
    !approved && !cancelled && !executed
}

/// Approved, still in-flight: operator must `withdrawExecute*` after the cancel window (INV-OP-W11).
pub fn is_operator_execution_candidate(approved: bool, cancelled: bool, executed: bool) -> bool {
    approved && !cancelled && !executed
}

/// Walk synthetic pages the way the Terra writer does and count candidates.
#[cfg(test)]
pub fn soak_operator_pages(
    pages: &[Vec<(bool, bool, bool, String)>],
    effective_limit: u32,
    use_cursor_field: bool,
) -> SoakStats {
    let mut processed = 0u32;
    let mut candidates = 0u32;
    let mut skipped_terminal = 0u32;
    let mut pages_fetched = 0u32;
    let mut idx = 0usize;
    loop {
        if idx >= pages.len() {
            break;
        }
        let page = &pages[idx];
        pages_fetched += 1;
        let last_hash = page.last().map(|e| e.3.clone());
        let is_last_synthetic = idx + 1 == pages.len();
        let next_start_after = if use_cursor_field && !is_last_synthetic {
            last_hash.clone()
        } else {
            None
        };
        let has_next_field = use_cursor_field;
        for (approved, cancelled, executed, _) in page {
            processed += 1;
            if is_operator_approval_candidate(*approved, *cancelled, *executed) {
                candidates += 1;
            } else {
                skipped_terminal += 1;
            }
        }
        match advance_withdraw_list_page(
            page.len(),
            effective_limit,
            next_start_after,
            has_next_field,
            last_hash,
        ) {
            PageAdvance::Continue(_) => idx += 1,
            PageAdvance::Exhausted => break,
        }
    }
    SoakStats {
        processed,
        candidates,
        skipped_terminal,
        pages_fetched,
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoakStats {
    pub processed: u32,
    pub candidates: u32,
    pub skipped_terminal: u32,
    pub pages_fetched: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u32) -> String {
        format!("h{n}")
    }

    /// Production LCD mix: 95 executed + 11 approved-not-executed, page cap 30.
    fn prod_legacy_pages() -> Vec<Vec<(bool, bool, bool, String)>> {
        let mut rows = Vec::new();
        for i in 0..95u32 {
            rows.push((true, false, true, hash(i)));
        }
        for i in 95..106u32 {
            rows.push((true, false, false, hash(i)));
        }
        rows.chunks(30).map(|c| c.to_vec()).collect()
    }

    #[test]
    fn execution_candidate_is_approved_not_terminal() {
        assert!(is_operator_execution_candidate(true, false, false));
        assert!(!is_operator_execution_candidate(false, false, false));
        assert!(!is_operator_execution_candidate(true, true, false));
        assert!(!is_operator_execution_candidate(true, false, true));
    }

    #[test]
    fn clamp_never_exceeds_contract_cap() {
        assert_eq!(clamp_page_size(50), 30);
        assert_eq!(clamp_page_size(30), 30);
        assert_eq!(clamp_page_size(0), 1);
    }

    #[test]
    fn oversized_request_without_cursor_stops_early() {
        // Bug: request 50, contract returns 30, treat len<50 as EOF.
        let pages = prod_legacy_pages();
        assert_eq!(pages.len(), 4);
        let stats = soak_operator_pages(&pages, 50, false);
        assert_eq!(stats.pages_fetched, 1);
        assert_eq!(stats.processed, 30);
        assert_eq!(stats.candidates, 0, "first 30 production rows are executed");
    }

    #[test]
    fn clamped_page_size_walks_full_history() {
        let pages = prod_legacy_pages();
        let stats = soak_operator_pages(&pages, 30, false);
        assert_eq!(stats.pages_fetched, 4);
        assert_eq!(stats.processed, 106);
        // Operator candidates are unapproved; production "active" rows are approved.
        assert_eq!(stats.candidates, 0);
        assert_eq!(stats.skipped_terminal, 106);
    }

    #[test]
    fn cursor_field_repairs_oversized_request() {
        let pages = prod_legacy_pages();
        let stats = soak_operator_pages(&pages, 50, true);
        assert_eq!(stats.pages_fetched, 4);
        assert_eq!(stats.processed, 106);
    }

    #[test]
    fn active_index_soak_is_one_short_page() {
        let active: Vec<(bool, bool, bool, String)> =
            (0..11u32).map(|i| (true, false, false, hash(i))).collect();
        let stats = soak_operator_pages(&[active], 30, true);
        assert_eq!(stats.pages_fetched, 1);
        assert_eq!(stats.processed, 11);
        assert_eq!(stats.skipped_terminal, 11); // approved → not operator candidates
        assert_eq!(stats.candidates, 0);
    }

    #[test]
    fn terminal_history_growth_does_not_add_active_pages() {
        let active: Vec<(bool, bool, bool, String)> =
            (0..11u32).map(|i| (false, false, false, hash(i))).collect();
        let small = soak_operator_pages(&[active.clone()], 30, true);
        // Same 11 unapproved actives; terminal rows are not in the active index.
        let still = soak_operator_pages(&[active], 30, true);
        assert_eq!(small, still);
        assert_eq!(still.pages_fetched, 1);
        assert_eq!(still.candidates, 11);
        assert_eq!(still.processed, 11);
    }
}
