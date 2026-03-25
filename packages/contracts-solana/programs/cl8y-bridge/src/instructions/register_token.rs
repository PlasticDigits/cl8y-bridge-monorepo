use crate::error::BridgeError;
use crate::state::{BridgeConfig, TokenMapping, TokenMode};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RegisterTokenParams {
    pub local_mint: Pubkey,
    pub dest_chain: [u8; 4],
    pub dest_token: [u8; 32],
    pub mode: TokenMode,
    /// Decimals of `local_mint` on Solana (or 9 for native SOL sentinel).
    pub decimals: u8,
    /// Decimals of `dest_token` on the remote chain.
    pub src_decimals: u8,
}

#[derive(Accounts)]
#[instruction(params: RegisterTokenParams)]
pub struct RegisterToken<'info> {
    #[account(
        seeds = [BridgeConfig::SEED],
        bump = bridge.bump,
    )]
    pub bridge: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = admin,
        space = 8 + TokenMapping::INIT_SPACE,
        seeds = [TokenMapping::SEED, params.dest_chain.as_ref(), params.dest_token.as_ref()],
        bump,
    )]
    pub token_mapping: Account<'info, TokenMapping>,

    /// SPL mint account; omit when `local_mint` is `Pubkey::default()` (native SOL mapping).
    pub mint: Option<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterToken>, params: RegisterTokenParams) -> Result<()> {
    let bridge = &ctx.accounts.bridge;
    require!(
        ctx.accounts.admin.key() == bridge.admin,
        BridgeError::UnauthorizedAdmin
    );

    if params.local_mint == Pubkey::default() {
        require!(ctx.accounts.mint.is_none(), BridgeError::InvalidDecimals);
        require!(
            matches!(params.mode, TokenMode::LockUnlock),
            BridgeError::InvalidNativeTokenMode
        );
    } else {
        let mint = ctx
            .accounts
            .mint
            .as_ref()
            .ok_or(BridgeError::TokenNotRegistered)?;
        require!(
            mint.key() == params.local_mint,
            BridgeError::TokenNotRegistered
        );
        require!(
            params.decimals == mint.decimals,
            BridgeError::InvalidDecimals
        );

        if params.mode == TokenMode::MintBurn {
            let bridge_pda = ctx.accounts.bridge.key();
            require!(
                mint.mint_authority.contains(&bridge_pda),
                BridgeError::MintAuthorityNotBridge
            );
        }
    }

    let mapping = &mut ctx.accounts.token_mapping;
    mapping.local_mint = params.local_mint;
    mapping.dest_chain = params.dest_chain;
    mapping.dest_token = params.dest_token;
    mapping.mode = params.mode;
    mapping.decimals = params.decimals;
    mapping.src_decimals = params.src_decimals;
    mapping.accrued_fees = 0;
    mapping.bump = ctx.bumps.token_mapping;

    Ok(())
}
