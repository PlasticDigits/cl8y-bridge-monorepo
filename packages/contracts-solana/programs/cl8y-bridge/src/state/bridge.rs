use anchor_lang::prelude::*;

/// Canonical token identifier for native SOL in transfer hashes.
/// All-zeros matches the EVM convention of address(0) for native/ETH tokens.
pub const NATIVE_SOL_TOKEN: Pubkey = Pubkey::new_from_array([0u8; 32]);

#[account]
#[derive(InitSpace)]
pub struct BridgeConfig {
    pub admin: Pubkey,
    pub operator: Pubkey,
    pub fee_bps: u16,
    pub withdraw_delay: i64,
    pub deposit_nonce: u64,
    pub accrued_native_fees: u64,
    pub paused: bool,
    pub chain_id: [u8; 4],
    pub bump: u8,
}

impl BridgeConfig {
    pub const SEED: &'static [u8] = b"bridge";
}
