use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, TokenMapping, TokenMode};
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RegisterTokenParams {
    pub local_mint: Pubkey,
    pub dest_chain: [u8; 4],
    pub dest_token: [u8; 32],
    pub mode: TokenMode,
    pub decimals: u8,
}

#[derive(Accounts)]
#[instruction(params: RegisterTokenParams)]
pub struct RegisterToken<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + TokenMapping::INIT_SPACE,
        seeds = [TokenMapping::SEED, params.dest_chain.as_ref(), params.dest_token.as_ref()],
        bump,
    )]
    pub token_mapping: Account<'info, TokenMapping>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterToken>, params: RegisterTokenParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);

    let mapping = &mut ctx.accounts.token_mapping;
    mapping.local_mint = params.local_mint;
    mapping.dest_chain = params.dest_chain;
    mapping.dest_token = params.dest_token;
    mapping.mode = params.mode;
    mapping.decimals = params.decimals;
    mapping.bump = ctx.bumps.token_mapping;

    Ok(())
}
