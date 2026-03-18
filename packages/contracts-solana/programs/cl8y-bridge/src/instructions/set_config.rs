use anchor_lang::prelude::*;
use crate::state::BridgeConfig;
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetConfigParams {
    pub new_admin: Option<Pubkey>,
    pub operator: Option<Pubkey>,
    pub fee_bps: Option<u16>,
    pub withdraw_delay: Option<i64>,
    pub paused: Option<bool>,
}

#[derive(Accounts)]
pub struct SetConfig<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    pub admin: Signer<'info>,
}

pub fn handler(ctx: Context<SetConfig>, params: SetConfigParams) -> Result<()> {
    let bridge = &mut ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);

    if let Some(new_admin) = params.new_admin {
        bridge.admin = new_admin;
    }
    if let Some(operator) = params.operator {
        bridge.operator = operator;
    }
    if let Some(fee_bps) = params.fee_bps {
        require!(fee_bps <= 10000, BridgeError::InvalidFeeBps);
        bridge.fee_bps = fee_bps;
    }
    if let Some(delay) = params.withdraw_delay {
        require!(delay >= 15 && delay <= 86400, BridgeError::InvalidWithdrawDelay);
        bridge.withdraw_delay = delay;
    }
    if let Some(paused) = params.paused {
        bridge.paused = paused;
    }

    Ok(())
}
