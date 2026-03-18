use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, TokenInterface, TokenAccount, Mint, TransferChecked};
use crate::state::BridgeConfig;
use crate::error::BridgeError;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawFeesParams {
    pub amount: u64,
    pub native: bool,
}

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(
        mut,
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// Admin's token account to receive SPL fees (optional for native withdrawals)
    /// CHECK: Only used for SPL fee withdrawal; validated by token_program CPI
    #[account(mut)]
    pub admin_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// Bridge-owned token account holding SPL fees (optional for native withdrawals)
    #[account(mut)]
    pub bridge_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// Mint of the SPL token (optional for native withdrawals)
    pub mint: Option<InterfaceAccount<'info, Mint>>,

    pub token_program: Option<Interface<'info, TokenInterface>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawFees>, params: WithdrawFeesParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);
    require!(params.amount > 0, BridgeError::ZeroAmount);

    if params.native {
        // Withdraw native SOL fees from bridge PDA
        let bridge_info = ctx.accounts.bridge.to_account_info();
        let admin_info = ctx.accounts.admin.to_account_info();
        **bridge_info.try_borrow_mut_lamports()? = bridge_info
            .lamports()
            .checked_sub(params.amount)
            .ok_or(BridgeError::ArithmeticOverflow)?;
        **admin_info.try_borrow_mut_lamports()? = admin_info
            .lamports()
            .checked_add(params.amount)
            .ok_or(BridgeError::ArithmeticOverflow)?;
    } else {
        // Withdraw SPL token fees from bridge token account
        let bridge_token_account = ctx.accounts.bridge_token_account
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let admin_token_account = ctx.accounts.admin_token_account
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let mint = ctx.accounts.mint
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let token_program = ctx.accounts.token_program
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;

        let bridge_seeds: &[&[u8]] = &[BridgeConfig::SEED, &[bridge.bump]];
        let decimals = mint.decimals;

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                TransferChecked {
                    from: bridge_token_account.to_account_info(),
                    mint: mint.to_account_info(),
                    to: admin_token_account.to_account_info(),
                    authority: ctx.accounts.bridge.to_account_info(),
                },
                &[bridge_seeds],
            ),
            params.amount,
            decimals,
        )?;
    }

    emit!(WithdrawFeesEvent {
        admin: ctx.accounts.admin.key(),
        amount: params.amount,
        native: params.native,
    });

    Ok(())
}

#[event]
pub struct WithdrawFeesEvent {
    pub admin: Pubkey,
    pub amount: u64,
    pub native: bool,
}
