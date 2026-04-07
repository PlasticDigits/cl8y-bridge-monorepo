use anchor_lang::prelude::*;

/// Marks `(src_chain, nonce)` as consumed by an approval (parity with EVM `withdrawNonceUsed`).
#[account]
#[derive(InitSpace)]
pub struct NonceUsed {
    pub bump: u8,
}

impl NonceUsed {
    pub const SEED: &'static [u8] = b"nonce_used";
}
