use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Token, TokenAccount, Mint, Transfer},
    associated_token::AssociatedToken,
};
use solana_program::clock::Clock;

use crate::{
    state::{PlatformConfig, Project, Collection, TokenEscrow, NftData},
    errors::MarketplaceError,
};

#[derive(Accounts)]
#[instruction(nft_mint: Pubkey, token_amount: u64, vesting_period: Option<i64>)]
pub struct CreateTokenEscrow<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"nft_data", nft_mint.as_ref()],
        bump = nft_data.bump,
        constraint = nft_data.owner == owner.key() @ MarketplaceError::NotNftOwner,
    )]
    pub nft_data: Account<'info, NftData>,
    
    #[account(
        constraint = collection.key() == nft_data.collection @ MarketplaceError::CollectionNotFound,
    )]
    pub collection: Account<'info, Collection>,
    
    #[account(
        mut,
        constraint = project.key() == collection.project @ MarketplaceError::ProjectNotFound,
        constraint = project.is_active @ MarketplaceError::ProjectNotFound,
    )]
    pub project: Account<'info, Project>,
    
    // The escrow account to hold tokens
    #[account(
        init,
        payer = owner,
        space = 8 + std::mem::size_of::<TokenEscrow>(),
        seeds = [b"token_escrow", nft_mint.as_ref()],
        bump,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    
    // The token mint
    pub token_mint: Account<'info, Mint>,
    
    // The escrow token account to hold tokens
    #[account(
        init,
        payer = owner,
        seeds = [b"escrow_token_account", nft_mint.as_ref()],
        bump,
        token::mint = token_mint,
        token::authority = token_escrow,
    )]
    pub escrow_token_account: Account<'info, TokenAccount>,
    
    // The owner's token account to transfer tokens from
    #[account(
        mut,
        constraint = owner_token_account.owner == owner.key() @ MarketplaceError::InvalidTokenAccount,
        constraint = owner_token_account.mint == token_mint.key() @ MarketplaceError::InvalidTokenAccount,
    )]
    pub owner_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn create_token_escrow(
    ctx: Context<CreateTokenEscrow>,
    nft_mint: Pubkey,
    token_amount: u64,
    vesting_period: Option<i64>,
) -> Result<()> {
    // Ensure token amount is greater than 0
    if token_amount == 0 {
        return Err(MarketplaceError::TokenPriceTooLow.into());
    }
    
    // Calculate vesting end timestamp if vesting period is provided
    let vesting_end_timestamp = if let Some(period) = vesting_period {
        if period <= 0 {
            None
        } else {
            let current_time = Clock::get()?.unix_timestamp;
            Some(current_time.checked_add(period).ok_or(MarketplaceError::CalculationOverflow)?)
        }
    } else {
        None
    };
    
    // Initialize token escrow account
    let token_escrow = &mut ctx.accounts.token_escrow;
    token_escrow.owner = ctx.accounts.owner.key();
    token_escrow.nft_mint = nft_mint;
    token_escrow.token_mint = ctx.accounts.token_mint.key();
    token_escrow.token_amount = token_amount;
    token_escrow.created_at = Clock::get()?.unix_timestamp;
    token_escrow.vesting_end_timestamp = vesting_end_timestamp;
    token_escrow.escrow_token_account = ctx.accounts.escrow_token_account.key();
    token_escrow.is_active = true;
    token_escrow.bump = *ctx.bumps.get("token_escrow").unwrap();
    
    // Transfer tokens from owner to escrow
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.owner_token_account.to_account_info(),
                to: ctx.accounts.escrow_token_account.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        token_amount,
    )?;
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    msg!("Token escrow created for NFT {}: {} tokens", nft_mint, token_amount);
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(nft_mint: Pubkey)]
pub struct CloseTokenEscrow<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"token_escrow", nft_mint.as_ref()],
        bump = token_escrow.bump,
        constraint = token_escrow.owner == owner.key() @ MarketplaceError::NotNftOwner,
        constraint = token_escrow.is_active @ MarketplaceError::EscrowNotActive,
        close = owner,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    
    #[account(
        mut,
        constraint = escrow_token_account.key() == token_escrow.escrow_token_account @ MarketplaceError::InvalidTokenAccount,
    )]
    pub escrow_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = owner_token_account.owner == owner.key() @ MarketplaceError::InvalidTokenAccount,
        constraint = owner_token_account.mint == token_escrow.token_mint @ MarketplaceError::InvalidTokenAccount,
    )]
    pub owner_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn close_token_escrow(
    ctx: Context<CloseTokenEscrow>,
    nft_mint: Pubkey,
) -> Result<()> {
    // Ensure token escrow nft mint matches the one in context
    if ctx.accounts.token_escrow.nft_mint != nft_mint {
        return Err(MarketplaceError::InvalidTokenEscrow.into());
    }
    
    // Check if vesting period has ended
    if let Some(vesting_end) = ctx.accounts.token_escrow.vesting_end_timestamp {
        let current_time = Clock::get()?.unix_timestamp;
        
        if current_time < vesting_end {
            return Err(MarketplaceError::VestingPeriodActive.into());
        }
    }
    
    // Get amount to return to owner
    let return_amount = ctx.accounts.escrow_token_account.amount;
    
    if return_amount > 0 {
        // Transfer tokens from escrow back to owner
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.owner_token_account.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
                &[&[
                    b"token_escrow",
                    nft_mint.as_ref(),
                    &[ctx.accounts.token_escrow.bump],
                ]],
            ),
            return_amount,
        )?;
    }
    
    // The token_escrow account will be automatically closed by the runtime due to close = owner
    
    msg!("Token escrow closed for NFT {}: {} tokens returned", nft_mint, return_amount);
    
    Ok(())
}