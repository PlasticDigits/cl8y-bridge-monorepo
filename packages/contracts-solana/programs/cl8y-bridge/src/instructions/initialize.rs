use anchor_lang::prelude::*;
use crate::state::BridgeConfig;
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeParams {
    pub operator: Pubkey,
    pub fee_bps: u16,
    pub withdraw_delay: i64,
    pub chain_id: [u8; 4],
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + BridgeConfig::INIT_SPACE,
        seeds = [BridgeConfig::SEED],
        bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
    require!(params.fee_bps <= 10000, BridgeError::InvalidFeeBps);
    require!(params.withdraw_delay >= 15 && params.withdraw_delay <= 86400, BridgeError::InvalidWithdrawDelay);
    require!(params.chain_id != [0u8; 4], BridgeError::InvalidChainId);

    let bridge = &mut ctx.accounts.bridge;
    bridge.admin = ctx.accounts.admin.key();
    bridge.operator = params.operator;
    bridge.fee_bps = params.fee_bps;
    bridge.withdraw_delay = params.withdraw_delay;
    bridge.deposit_nonce = 0;
    bridge.paused = false;
    bridge.chain_id = params.chain_id;
    bridge.bump = ctx.bumps.bridge;

    msg!("Bridge initialized with operator: {}", params.operator);
    Ok(())
}
