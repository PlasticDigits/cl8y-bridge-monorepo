use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, PendingWithdraw};
use crate::error::BridgeError;

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

    pub admin: Signer<'info>,
}

pub fn handler(ctx: Context<WithdrawReenable>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);

    let pw = &mut ctx.accounts.pending_withdraw;
    require!(pw.cancelled, BridgeError::NotCancelled);
    require!(!pw.executed, BridgeError::AlreadyExecuted);

    pw.cancelled = false;
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
