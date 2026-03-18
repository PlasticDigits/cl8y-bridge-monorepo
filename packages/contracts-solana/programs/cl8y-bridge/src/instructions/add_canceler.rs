use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, CancelerEntry};
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddCancelerParams {
    pub canceler: Pubkey,
    pub active: bool,
}

#[derive(Accounts)]
#[instruction(params: AddCancelerParams)]
pub struct AddCanceler<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + CancelerEntry::INIT_SPACE,
        seeds = [CancelerEntry::SEED, params.canceler.as_ref()],
        bump,
    )]
    pub canceler_entry: Account<'info, CancelerEntry>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<AddCanceler>, params: AddCancelerParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);

    let entry = &mut ctx.accounts.canceler_entry;
    entry.pubkey = params.canceler;
    entry.active = params.active;
    entry.bump = ctx.bumps.canceler_entry;

    Ok(())
}
