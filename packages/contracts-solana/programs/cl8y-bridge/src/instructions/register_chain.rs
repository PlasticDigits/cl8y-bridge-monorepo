use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, ChainEntry};
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RegisterChainParams {
    pub chain_id: [u8; 4],
    pub identifier: String,
}

#[derive(Accounts)]
#[instruction(params: RegisterChainParams)]
pub struct RegisterChain<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + ChainEntry::INIT_SPACE,
        seeds = [ChainEntry::SEED, params.chain_id.as_ref()],
        bump,
    )]
    pub chain_entry: Account<'info, ChainEntry>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterChain>, params: RegisterChainParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);
    require!(params.identifier.len() <= 64, BridgeError::ArithmeticOverflow);

    let entry = &mut ctx.accounts.chain_entry;
    entry.chain_id = params.chain_id;
    entry.identifier = params.identifier;
    entry.bump = ctx.bumps.chain_entry;

    Ok(())
}
