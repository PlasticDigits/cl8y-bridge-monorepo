use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, PendingWithdraw};
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawApproveParams {
    pub transfer_hash: [u8; 32],
}

#[derive(Accounts)]
#[instruction(params: WithdrawApproveParams)]
pub struct WithdrawApprove<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, params.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    pub operator: Signer<'info>,
}

pub fn handler(ctx: Context<WithdrawApprove>, params: WithdrawApproveParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(!bridge.paused, BridgeError::BridgePaused);
    require!(ctx.accounts.operator.key() == bridge.operator, BridgeError::UnauthorizedOperator);

    let pw = &mut ctx.accounts.pending_withdraw;
    require!(!pw.approved, BridgeError::AlreadyApproved);
    require!(!pw.cancelled, BridgeError::WithdrawalCancelled);

    pw.approved = true;
    pw.approved_at = Clock::get()?.unix_timestamp;

    emit!(WithdrawApproveEvent {
        transfer_hash: params.transfer_hash,
        approved_at: pw.approved_at,
    });

    Ok(())
}

#[event]
pub struct WithdrawApproveEvent {
    pub transfer_hash: [u8; 32],
    pub approved_at: i64,
}
