use crate::error::BridgeError;
use crate::state::{BridgeConfig, TokenMapping};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface, TransferChecked};

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

    #[account(mut)]
    pub token_mapping: Option<Box<Account<'info, TokenMapping>>>,

    pub token_program: Option<Interface<'info, TokenInterface>>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawFees>, params: WithdrawFeesParams) -> Result<()> {
    let bridge_admin = ctx.accounts.bridge.admin;
    let bridge_bump = ctx.accounts.bridge.bump;
    let bridge_key = ctx.accounts.bridge.key();
    require!(
        ctx.accounts.admin.key() == bridge_admin,
        BridgeError::UnauthorizedAdmin
    );
    require!(params.amount > 0, BridgeError::ZeroAmount);

    if params.native {
        require!(
            ctx.accounts.bridge.accrued_native_fees >= params.amount,
            BridgeError::InsufficientAccruedFees
        );

        // Withdraw native SOL fees from bridge PDA
        let bridge_info = ctx.accounts.bridge.to_account_info();
        let admin_info = ctx.accounts.admin.to_account_info();
        let rent_exempt = Rent::get()?.minimum_balance(8 + BridgeConfig::INIT_SPACE);
        let available = bridge_info.lamports().saturating_sub(rent_exempt);
        require!(
            available >= params.amount,
            BridgeError::InsufficientBridgeBalance
        );
        **bridge_info.try_borrow_mut_lamports()? = bridge_info
            .lamports()
            .checked_sub(params.amount)
            .ok_or(BridgeError::ArithmeticOverflow)?;
        **admin_info.try_borrow_mut_lamports()? = admin_info
            .lamports()
            .checked_add(params.amount)
            .ok_or(BridgeError::ArithmeticOverflow)?;
        ctx.accounts.bridge.accrued_native_fees = ctx
            .accounts
            .bridge
            .accrued_native_fees
            .checked_sub(params.amount)
            .ok_or(BridgeError::ArithmeticOverflow)?;
    } else {
        // Withdraw SPL token fees from bridge token account
        let bridge_token_account = ctx
            .accounts
            .bridge_token_account
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let admin_token_account = ctx
            .accounts
            .admin_token_account
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let mint = ctx
            .accounts
            .mint
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let token_mapping = ctx
            .accounts
            .token_mapping
            .as_mut()
            .ok_or(BridgeError::TokenNotRegistered)?;
        let token_program = ctx
            .accounts
            .token_program
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;

        require!(
            token_mapping.local_mint == mint.key(),
            BridgeError::TokenNotRegistered
        );
        let (expected_mapping, _) = Pubkey::find_program_address(
            &[
                TokenMapping::SEED,
                token_mapping.dest_chain.as_ref(),
                token_mapping.dest_token.as_ref(),
            ],
            ctx.program_id,
        );
        require!(
            token_mapping.key() == expected_mapping,
            BridgeError::TokenNotRegistered
        );
        require!(
            bridge_token_account.owner == bridge_key,
            BridgeError::TokenNotRegistered
        );
        require!(
            bridge_token_account.mint == mint.key(),
            BridgeError::TokenNotRegistered
        );
        require!(
            admin_token_account.mint == mint.key(),
            BridgeError::TokenNotRegistered
        );
        require!(
            token_mapping.accrued_fees >= params.amount,
            BridgeError::InsufficientAccruedFees
        );

        let bridge_seeds: &[&[u8]] = &[BridgeConfig::SEED, &[bridge_bump]];
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

        token_mapping.accrued_fees = token_mapping
            .accrued_fees
            .checked_sub(params.amount)
            .ok_or(BridgeError::ArithmeticOverflow)?;
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
