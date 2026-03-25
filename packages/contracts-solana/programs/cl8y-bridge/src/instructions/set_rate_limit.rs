use crate::error::BridgeError;
use crate::state::{BridgeConfig, WithdrawRateLimit};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct SetRateLimitParams {
    /// Local mint (or [`crate::state::NATIVE_SOL_TOKEN`] for native SOL mapping).
    pub local_mint: Pubkey,
    pub min_per_transaction: u128,
    pub max_per_transaction: u128,
    pub max_per_period: u128,
}

#[derive(Accounts)]
#[instruction(params: SetRateLimitParams)]
pub struct SetRateLimit<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init_if_needed,
        payer = admin,
        space = 8 + WithdrawRateLimit::INIT_SPACE,
        seeds = [WithdrawRateLimit::SEED, params.local_mint.as_ref()],
        bump,
    )]
    pub withdraw_rate_limit: Account<'info, WithdrawRateLimit>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetRateLimit>, params: SetRateLimitParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(
        ctx.accounts.admin.key() == bridge.admin,
        BridgeError::UnauthorizedAdmin
    );

    let wr = &mut ctx.accounts.withdraw_rate_limit;
    wr.explicit_config = true;
    wr.min_per_transaction = params.min_per_transaction;
    wr.max_per_transaction = params.max_per_transaction;
    wr.max_per_period = params.max_per_period;
    wr.bump = ctx.bumps.withdraw_rate_limit;

    Ok(())
}
