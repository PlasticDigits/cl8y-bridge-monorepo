use crate::decimal::normalize_decimals;
use crate::error::BridgeError;
use crate::hash::compute_transfer_hash;
use crate::state::{
    BridgeConfig, ExecutedHash, PendingWithdraw, WithdrawRateLimit, NATIVE_SOL_TOKEN,
};
use anchor_lang::prelude::*;

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

    #[account(
        init_if_needed,
        payer = recipient,
        space = 8 + WithdrawRateLimit::INIT_SPACE,
        seeds = [WithdrawRateLimit::SEED, NATIVE_SOL_TOKEN.as_ref()],
        bump,
    )]
    pub withdraw_rate_limit: Account<'info, WithdrawRateLimit>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawExecuteNative>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;

    require!(!bridge.paused, BridgeError::BridgePaused);

    {
        let pw = &ctx.accounts.pending_withdraw;
        require!(!pw.cancelled, BridgeError::WithdrawalCancelled);
        require!(pw.approved, BridgeError::NotApproved);
        require!(!pw.executed, BridgeError::AlreadyExecuted);
        require!(
            pw.dest_account == ctx.accounts.recipient.key(),
            BridgeError::WrongRecipient
        );
        require!(
            pw.token == NATIVE_SOL_TOKEN,
            BridgeError::NotNativeToken
        );

        let recomputed = compute_transfer_hash(
            &pw.src_chain,
            &bridge.chain_id,
            &pw.src_account,
            &pw.dest_account.to_bytes(),
            &pw.token.to_bytes(),
            pw.amount,
            pw.nonce,
        );
        require!(recomputed == pw.transfer_hash, BridgeError::HashMismatch);

        let clock = Clock::get()?;
        let window_end = pw
            .approved_at
            .checked_add(bridge.withdraw_delay)
            .ok_or(BridgeError::ArithmeticOverflow)?;
        require!(
            clock.unix_timestamp > window_end,
            BridgeError::DelayNotElapsed
        );
    }

    let amount_u128 = normalize_decimals(
        ctx.accounts.pending_withdraw.amount,
        ctx.accounts.pending_withdraw.src_decimals,
        ctx.accounts.pending_withdraw.dest_decimals,
    )?;
    let amount: u64 = amount_u128
        .try_into()
        .map_err(|_| BridgeError::AmountExceedsU64)?;

    {
        let wr = &mut ctx.accounts.withdraw_rate_limit;
        wr.bump = ctx.bumps.withdraw_rate_limit;
        // No on-chain total supply for native SOL; Terra uses bank supply — match implicit-default path (zero supply).
        let supply = 0u128;
        let (min_tx, max_tx, max_period) = crate::rate_limit::resolve_effective_limits(
            wr.explicit_config,
            wr.min_per_transaction,
            wr.max_per_transaction,
            wr.max_per_period,
            supply,
        );
        let mut window_start = wr.window_start;
        let mut used = wr.used;
        crate::rate_limit::check_and_update_withdraw_rate_limit(
            Clock::get()?.unix_timestamp,
            amount_u128,
            min_tx,
            max_tx,
            max_period,
            &mut window_start,
            &mut used,
        )?;
        wr.window_start = window_start;
        wr.used = used;
    }

    let pw = &mut ctx.accounts.pending_withdraw;
    pw.executed = true;
    let transfer_hash = pw.transfer_hash;
    let dest_account = pw.dest_account;

    ctx.accounts.executed_hash.bump = ctx.bumps.executed_hash;

    // Transfer native SOL from bridge PDA to recipient
    let bridge_info = ctx.accounts.bridge.to_account_info();
    let recipient_info = ctx.accounts.recipient.to_account_info();
    let rent_exempt = Rent::get()?.minimum_balance(8 + BridgeConfig::INIT_SPACE);
    let available = bridge_info.lamports().saturating_sub(rent_exempt);
    require!(available >= amount, BridgeError::InsufficientBridgeBalance);
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
