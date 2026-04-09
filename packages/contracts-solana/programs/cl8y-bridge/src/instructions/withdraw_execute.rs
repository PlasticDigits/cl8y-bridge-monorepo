use crate::decimal::normalize_decimals;
use crate::error::BridgeError;
use crate::hash::compute_transfer_hash;
use crate::state::{
    BridgeConfig, ExecutedHash, PendingWithdraw, TokenMapping, TokenMode, WithdrawRateLimit,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Mint, MintTo, TokenAccount, TokenInterface, TransferChecked,
};

/// SPL (or Token-2022) withdrawal execution: lock/unlock transfer or mint.
///
/// **Permissionless (after `withdraw_delay`):** any signer may invoke this as `executor` and pay
/// account-creation rent, matching Terra `WithdrawExecuteUnlock` / `WithdrawExecuteMint` and EVM
/// `withdrawExecuteUnlock` / `withdrawExecuteMint` (any caller after the cancel window).
/// The token recipient does **not** sign; `recipient` is only the pending `dest_account` pubkey.
/// Lamports from closing `pending_withdraw` go to **`bridge.operator`**, not the recipient.
///
/// See `docs/SOLANA_BRIDGE_INVARIANTS.md` (INV-W2, INV-D1).
#[derive(Accounts)]
pub struct WithdrawExecute<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Box<Account<'info, BridgeConfig>>,

    /// CHECK: receives rent when `pending_withdraw` is closed — must be `bridge.operator`.
    #[account(mut, address = bridge.operator @ BridgeError::UnauthorizedOperator)]
    pub operator_rent: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [PendingWithdraw::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump = pending_withdraw.bump,
        close = operator_rent,
    )]
    pub pending_withdraw: Box<Account<'info, PendingWithdraw>>,

    #[account(
        init,
        payer = executor,
        space = 8 + ExecutedHash::INIT_SPACE,
        seeds = [ExecutedHash::SEED, pending_withdraw.transfer_hash.as_ref()],
        bump,
    )]
    pub executed_hash: Box<Account<'info, ExecutedHash>>,

    #[account(
        mut,
        constraint = mint.key() == token_mapping.local_mint @ BridgeError::TokenNotRegistered
    )]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = recipient,
        associated_token::token_program = token_program,
    )]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Bridge-owned token account for lock/unlock mode
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bridge,
        associated_token::token_program = token_program,
    )]
    pub bridge_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        seeds = [TokenMapping::SEED, pending_withdraw.src_chain.as_ref(), token_mapping.dest_token.as_ref()],
        bump = token_mapping.bump,
    )]
    pub token_mapping: Box<Account<'info, TokenMapping>>,

    #[account(
        init_if_needed,
        payer = executor,
        space = 8 + WithdrawRateLimit::INIT_SPACE,
        seeds = [WithdrawRateLimit::SEED, mint.key().as_ref()],
        bump,
    )]
    pub withdraw_rate_limit: Account<'info, WithdrawRateLimit>,

    #[account(mut)]
    pub executor: Signer<'info>,

    /// CHECK: withdrawal destination pubkey (must match pending); does not sign.
    #[account(
        constraint = recipient.key() == pending_withdraw.dest_account @ BridgeError::WrongRecipient
    )]
    pub recipient: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawExecute>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;

    require!(!bridge.paused, BridgeError::BridgePaused);

    {
        let pw = &ctx.accounts.pending_withdraw;
        require!(!pw.cancelled, BridgeError::WithdrawalCancelled);
        require!(pw.approved, BridgeError::NotApproved);
        require!(!pw.executed, BridgeError::AlreadyExecuted);
        require!(
            pw.token == ctx.accounts.mint.key(),
            BridgeError::TokenMintMismatch
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
        let supply = ctx.accounts.mint.supply as u128;
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

    let bridge_seeds: &[&[u8]] = &[BridgeConfig::SEED, &[bridge.bump]];
    let decimals = ctx.accounts.mint.decimals;

    match ctx.accounts.token_mapping.mode {
        TokenMode::LockUnlock => {
            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.bridge_token_account.to_account_info(),
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.recipient_token_account.to_account_info(),
                        authority: bridge.to_account_info(),
                    },
                    &[bridge_seeds],
                ),
                amount,
                decimals,
            )?;
        }
        TokenMode::MintBurn => {
            token_interface::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    MintTo {
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.recipient_token_account.to_account_info(),
                        authority: bridge.to_account_info(),
                    },
                    &[bridge_seeds],
                ),
                amount,
            )?;
        }
    }

    emit!(WithdrawExecuteEvent {
        transfer_hash,
        recipient: dest_account,
        amount: amount_u128,
    });

    Ok(())
}

#[event]
pub struct WithdrawExecuteEvent {
    pub transfer_hash: [u8; 32],
    pub recipient: Pubkey,
    pub amount: u128,
}
