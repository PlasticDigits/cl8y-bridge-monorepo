use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TokenInterface, TokenAccount, Mint, TransferChecked, MintTo};
use crate::state::{BridgeConfig, PendingWithdraw, TokenMapping, TokenMode, ExecutedHash};
use crate::error::BridgeError;

#[derive(Accounts)]
pub struct WithdrawExecute<'info> {
    #[account(
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

    #[account(constraint = mint.key() == token_mapping.local_mint @ BridgeError::TokenNotRegistered)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = recipient,
    )]
    pub recipient_token_account: InterfaceAccount<'info, TokenAccount>,

    /// Bridge-owned token account for lock/unlock mode
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bridge,
    )]
    pub bridge_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        seeds = [TokenMapping::SEED, pending_withdraw.src_chain.as_ref(), token_mapping.dest_token.as_ref()],
        bump = token_mapping.bump,
    )]
    pub token_mapping: Account<'info, TokenMapping>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawExecute>) -> Result<()> {
    let bridge = &ctx.accounts.bridge;

    require!(!bridge.paused, BridgeError::BridgePaused);

    {
        let pw = &ctx.accounts.pending_withdraw;
        require!(pw.approved, BridgeError::NotApproved);
        require!(!pw.cancelled, BridgeError::WithdrawalCancelled);
        require!(!pw.executed, BridgeError::AlreadyExecuted);
        require!(pw.dest_account == ctx.accounts.recipient.key(), BridgeError::WrongRecipient);
        require!(pw.token == ctx.accounts.mint.key(), BridgeError::TokenMintMismatch);

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
