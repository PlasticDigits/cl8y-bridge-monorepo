use anchor_lang::prelude::*;
use anchor_lang::system_program;
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

    pub mint: Option<InterfaceAccount<'info, Mint>>,

    /// Bridge-owned token account (for SPL fee withdrawal)
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = bridge,
    )]
    pub bridge_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    /// Admin's token account (for SPL fee withdrawal)
    #[account(mut)]
    pub admin_token_account: Option<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub token_program: Option<Interface<'info, TokenInterface>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawFees>, params: WithdrawFeesParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(ctx.accounts.admin.key() == bridge.admin, BridgeError::UnauthorizedAdmin);
    require!(params.amount > 0, BridgeError::ZeroAmount);

    let bridge_seeds: &[&[u8]] = &[BridgeConfig::SEED, &[bridge.bump]];

    if params.native {
        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.bridge.to_account_info(),
                    to: ctx.accounts.admin.to_account_info(),
                },
                &[bridge_seeds],
            ),
            params.amount,
        )?;
    } else {
        let mint = ctx.accounts.mint.as_ref()
            .ok_or(error!(BridgeError::TokenNotRegistered))?;
        let bridge_ta = ctx.accounts.bridge_token_account.as_ref()
            .ok_or(error!(BridgeError::TokenNotRegistered))?;
        let admin_ta = ctx.accounts.admin_token_account.as_ref()
            .ok_or(error!(BridgeError::TokenNotRegistered))?;
        let token_prog = ctx.accounts.token_program.as_ref()
            .ok_or(error!(BridgeError::TokenNotRegistered))?;

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                token_prog.to_account_info(),
                TransferChecked {
                    from: bridge_ta.to_account_info(),
                    mint: mint.to_account_info(),
                    to: admin_ta.to_account_info(),
                    authority: ctx.accounts.bridge.to_account_info(),
                },
                &[bridge_seeds],
            ),
            params.amount,
            mint.decimals,
        )?;
    }

    Ok(())
}
