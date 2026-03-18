use anchor_lang::prelude::*;
use crate::state::{BridgeConfig, PendingWithdraw, ExecutedHash};
use crate::error::BridgeError;

#[derive(Accounts)]
pub struct WithdrawExecuteNative<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
        close = recipient,
    )]
    pub pending_withdraw: Account<'info, PendingWithdraw>,

    #[account(
        init,
        payer = recipient,
        space = 8 + ExecutedHash::INIT_SPACE,
        seeds = [ExecutedHash::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump,
    )]
    pub executed_hash: Account<'info, ExecutedHash>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawExecuteNative>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;

    require!(!bridge.paused, BridgeError::BridgePaused);

    {
        let pw = &ctx.accounts.pending_withdraw;
        require!(pw.approved, BridgeError::NotApproved);
        require!(!pw.cancelled, BridgeError::WithdrawalCancelled);
        require!(!pw.executed, BridgeError::AlreadyExecuted);
        require!(pw.dest_account == ctx.accounts.recipient.key(), BridgeError::WrongRecipient);

        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= pw.approved_at + bridge.withdraw_delay,
            BridgeError::DelayNotElapsed
        );
    }

    let pw = &mut ctx.accounts.pending_withdraw;
    pw.executed = true;

    let amount_u128 = pw.amount;
    let amount: u64 = amount_u128
        .try_into()
        .map_err(|_| BridgeError::AmountExceedsU64)?;
    let transfer_hash = pw.transfer_hash;
    let dest_account = pw.dest_account;

    ctx.accounts.executed_hash.bump = ctx.bumps.executed_hash;

    // Transfer native SOL from bridge PDA to recipient
    let bridge_info = ctx.accounts.bridge.to_account_info();
    let recipient_info = ctx.accounts.recipient.to_account_info();
    **bridge_info.try_borrow_mut_lamports()? = bridge_info
        .lamports()
        .checked_sub(amount)
        .ok_or(BridgeError::ArithmeticOverflow)?;
    **recipient_info.try_borrow_mut_lamports()? = recipient_info
        .lamports()
        .checked_add(amount)
        .ok_or(BridgeError::ArithmeticOverflow)?;

    emit!(WithdrawExecuteNativeEvent {
        transfer_hash,
        recipient: dest_account,
        amount: amount_u128,
    });

    Ok(())
}

#[event]
pub struct WithdrawExecuteNativeEvent {
    pub transfer_hash: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u128,
}
