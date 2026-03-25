use crate::error::BridgeError;
use crate::state::{BridgeConfig, PendingWithdraw};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct WithdrawReenable<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    /// Operator or admin (EVM: operator-only uncancel; we allow admin too for ops).
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<WithdrawReenable>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(!bridge.paused, BridgeError::BridgePaused);
    let k = ctx.accounts.authority.key();
    require!(
        k == bridge.admin || k == bridge.operator,
        BridgeError::UnauthorizedOperator
    );

    let pw = &mut ctx.accounts.pending_withdraw;
    require!(pw.cancelled, BridgeError::NotCancelled);
    require!(!pw.executed, BridgeError::AlreadyExecuted);

    pw.cancelled = false;
    // Restart cancel window; keep approval valid (EVM/Terra uncancel semantics).
    pw.approved = true;
    pw.approved_at = Clock::get()?.unix_timestamp;

    emit!(WithdrawReenableEvent {
        transfer_hash: pw.transfer_hash,
    });

    Ok(())
}

#[event]
pub struct WithdrawReenableEvent {
    pub transfer_hash: [u8; 32],
}
