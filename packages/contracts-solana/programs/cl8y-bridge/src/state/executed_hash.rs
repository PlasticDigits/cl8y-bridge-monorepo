use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct ExecutedHash {
    pub bump: u8,
}

impl ExecutedHash {
    pub const SEED: &'static [u8] = b"executed";
}
