use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct CancelerEntry {
    pub pubkey: Pubkey,
    pub active: bool,
    pub bump: u8,
}

impl CancelerEntry {
    pub const SEED: &'static [u8] = b"canceler";
}
