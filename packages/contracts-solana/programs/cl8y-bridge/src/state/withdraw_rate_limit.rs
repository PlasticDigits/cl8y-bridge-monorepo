use anchor_lang::prelude::*;

/// Per local-mint withdraw rate limit state (TerraClassic `RATE_LIMITS` + `RATE_WINDOWS` + EVM min-per-tx).
#[account]
#[derive(InitSpace)]
pub struct WithdrawRateLimit {
    /// When true, stored fields are used as set by admin (`set_rate_limit`, EVM `setRateLimit` parity).
    /// When false, implicit defaults match EVM `_setDefaultRateLimits` / Terra `add_token` auto limits:
    /// min = supply/1e6, max per tx = supply/1e4, max per period = max per tx; zero SPL supply uses a
    /// fixed period floor (native SOL path) — see `rate_limit::resolve_effective_limits`.
    pub explicit_config: bool,
    pub min_per_transaction: u128,
    pub max_per_transaction: u128,
    pub max_per_period: u128,
    pub window_start: i64,
    pub used: u128,
    pub bump: u8,
}

impl WithdrawRateLimit {
    pub const SEED: &'static [u8] = b"w_rate_lim";
}
