use anchor_lang::prelude::*;

/// Per local-mint withdraw rate limit state (TerraClassic `RATE_LIMITS` + `RATE_WINDOWS` + EVM min-per-tx).
#[account]
#[derive(InitSpace)]
pub struct WithdrawRateLimit {
    /// When true, `min_*` / `max_*` are used as set by admin (EVM `TokenRegistry` semantics).
    /// When false, Terra-style implicit limits apply: no min, no per-tx cap, period cap = supply/1000 or default.
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
