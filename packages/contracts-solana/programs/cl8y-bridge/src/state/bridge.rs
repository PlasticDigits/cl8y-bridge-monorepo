use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct BridgeConfig {
    pub admin: Pubkey,
    pub operator: Pubkey,
    pub fee_bps: u16,
    pub withdraw_delay: i64,
    pub deposit_nonce: u64,
    pub paused: bool,
    pub chain_id: [u8; 4],
    pub bump: u8,
}

impl BridgeConfig {
    pub const SEED: &'static [u8] = b"bridge";
}
