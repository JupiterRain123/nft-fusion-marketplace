use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Mint, Token, TokenAccount, Transfer},
    associated_token::AssociatedToken,
};
use solana_program::clock::Clock;

use crate::{
    state::{PlatformConfig, Project, Collection, LiquidityPool, NftData, TokenEscrow},
    errors::MarketplaceError,
    modules::oracle::check_oracle_status,
    modules::cooldown::check_cooldown_expired,
};

#[derive(Accounts)]
pub struct RedeemNftForToken<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"nft_data", nft_mint.key().as_ref()],
        bump = nft_data.bump,
        constraint = nft_data.owner == user.key() @ MarketplaceError::NotNftOwner,
    )]
    pub nft_data: Account<'info, NftData>,
    
    #[account(
        mut,
        seeds = [b"collection", collection.collection_id.as_bytes()],
        bump = collection.bump,
    )]
    pub collection: Account<'info, Collection>,
    
    #[account(
        mut,
        seeds = [b"project", project.project_id.as_bytes()],
        bump = project.bump,
        constraint = project.is_active @ MarketplaceError::ProjectNotFound,
    )]
    pub project: Account<'info, Project>,
    
    #[account(
        mut,
        seeds = [b"liquidity_pool", project.key().as_ref()],
        bump = liquidity_pool.bump,
        constraint = !liquidity_pool.redemption_locked @ MarketplaceError::RedemptionLocked,
    )]
    pub liquidity_pool: Account<'info, LiquidityPool>,
    
    /// The NFT mint that will be burned
    #[account(mut)]
    pub nft_mint: Account<'info, Mint>,
    
    /// The user's NFT token account
    #[account(
        mut,
        constraint = user_nft_account.owner == user.key(),
        constraint = user_nft_account.mint == nft_mint.key(),
    )]
    pub user_nft_account: Account<'info, TokenAccount>,
    
    /// The user's token account to receive redeemed tokens
    #[account(
        mut,
        constraint = user_token_account.owner == user.key(),
        constraint = user_token_account.mint == token_mint.key(),
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = lp_token_account.key() == liquidity_pool.lp_token_account,
    )]
    pub lp_token_account: Account<'info, TokenAccount>,
    
    #[account(
        constraint = token_mint.key() == liquidity_pool.token_mint,
    )]
    pub token_mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nft_mint: Pubkey)]
pub struct TokenEscrowRedemption<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    
    // NFT to be redeemed
    #[account(
        mut,
        seeds = [b"nft_data", nft_mint.as_ref()],
        bump = nft_data.bump,
        constraint = nft_data.owner == user.key() @ MarketplaceError::NotNftOwner,
        constraint = nft_data.mint == nft_mint @ MarketplaceError::InvalidNftForFusion,
    )]
    pub nft_data: Account<'info, NftData>,
    
    // Token escrow account for this NFT
    #[account(
        mut,
        seeds = [b"token_escrow", nft_mint.as_ref()],
        bump = token_escrow.bump,
        constraint = token_escrow.nft_mint == nft_mint @ MarketplaceError::InvalidTokenEscrow,
        constraint = token_escrow.owner == user.key() @ MarketplaceError::NotNftOwner,
        constraint = token_escrow.is_active @ MarketplaceError::EscrowNotActive,
    )]
    pub token_escrow: Account<'info, TokenEscrow>,
    
    // Collection this NFT belongs to
    #[account(
        constraint = collection.key() == nft_data.collection @ MarketplaceError::CollectionNotFound,
    )]
    pub collection: Account<'info, Collection>,
    
    // Project this collection belongs to
    #[account(
        mut,
        constraint = project.key() == collection.project @ MarketplaceError::ProjectNotFound,
    )]
    pub project: Account<'info, Project>,
    
    // User's token account to receive redeemed tokens
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ MarketplaceError::InvalidTokenAccount,
        constraint = user_token_account.mint == token_escrow.token_mint @ MarketplaceError::InvalidTokenAccount,
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
    // Escrow token account
    #[account(
        mut,
        constraint = escrow_token_account.key() == token_escrow.escrow_token_account @ MarketplaceError::InvalidTokenAccount,
    )]
    pub escrow_token_account: Account<'info, TokenAccount>,
    
    // Platform fee destination
    #[account(
        mut,
        constraint = platform_treasury.owner == platform_config.platform_treasury @ MarketplaceError::InvalidTokenAccount,
        constraint = platform_treasury.mint == token_escrow.token_mint @ MarketplaceError::InvalidTokenAccount,
    )]
    pub platform_treasury: Account<'info, TokenAccount>,
    
    // Project fee destination
    #[account(
        mut,
        constraint = project_treasury.owner == project.project_treasury @ MarketplaceError::InvalidTokenAccount,
        constraint = project_treasury.mint == token_escrow.token_mint @ MarketplaceError::InvalidTokenAccount,
    )]
    pub project_treasury: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn redeem_escrow_token(
    ctx: Context<TokenEscrowRedemption>,
    nft_mint: Pubkey,
) -> Result<()> {
    // Check if vesting period has ended
    if let Some(vesting_end) = ctx.accounts.token_escrow.vesting_end_timestamp {
        let current_time = Clock::get()?.unix_timestamp;
        
        if current_time < vesting_end {
            return Err(MarketplaceError::VestingPeriodActive.into());
        }
    }
    
    // Get amount to transfer
    let redemption_amount = ctx.accounts.token_escrow.token_amount;
    
    // Calculate redemption fee (small fee to prevent abuse)
    let platform_fee_bps = ctx.accounts.platform_config.platform_fee_basis_points as u64;
    let redemption_fee = redemption_amount
        .checked_mul(platform_fee_bps as u64)
        .ok_or(MarketplaceError::CalculationOverflow)?
        .checked_div(10000)
        .ok_or(MarketplaceError::CalculationOverflow)?;
        
    let project_fee_bps = ctx.accounts.project.royalty_basis_points as u64;
    let project_redemption_fee = redemption_amount
        .checked_mul(project_fee_bps as u64)
        .ok_or(MarketplaceError::CalculationOverflow)?
        .checked_div(10000)
        .ok_or(MarketplaceError::CalculationOverflow)?;
        
    // Calculate final amount to transfer to user
    let final_amount = redemption_amount
        .checked_sub(redemption_fee)
        .ok_or(MarketplaceError::CalculationOverflow)?
        .checked_sub(project_redemption_fee)
        .ok_or(MarketplaceError::CalculationOverflow)?;
    
    // Transfer tokens from escrow to user
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.escrow_token_account.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.token_escrow.to_account_info(),
            },
            &[&[
                b"token_escrow", 
                nft_mint.as_ref(), 
                &[ctx.accounts.token_escrow.bump]
            ]],
        ),
        final_amount,
    )?;
    
    // Transfer redemption fee to platform treasury
    if redemption_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.platform_treasury.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
                &[&[
                    b"token_escrow", 
                    nft_mint.as_ref(), 
                    &[ctx.accounts.token_escrow.bump]
                ]],
            ),
            redemption_fee,
        )?;
    }
    
    // Transfer project redemption fee to project treasury
    if project_redemption_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.escrow_token_account.to_account_info(),
                    to: ctx.accounts.project_treasury.to_account_info(),
                    authority: ctx.accounts.token_escrow.to_account_info(),
                },
                &[&[
                    b"token_escrow", 
                    nft_mint.as_ref(), 
                    &[ctx.accounts.token_escrow.bump]
                ]],
            ),
            project_redemption_fee,
        )?;
    }
    
    // Mark escrow as inactive
    let token_escrow = &mut ctx.accounts.token_escrow;
    token_escrow.is_active = false;
    
    // Burn or close the NFT (in a real implementation, you would burn the NFT)
    // For now, we'll just mark it as redeemed by updating the NFT data
    let nft_data = &mut ctx.accounts.nft_data;
    nft_data.owner = ctx.accounts.project.key(); // Transfer ownership to project
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    msg!("NFT redeemed for tokens from escrow: {}", nft_mint);
    
    Ok(())
}

pub fn redeem_nft_for_token(
    ctx: Context<RedeemNftForToken>,
    nft_mint: Pubkey,
) -> Result<()> {
    // Ensure NFT mint matches the one in context
    if ctx.accounts.nft_mint.key() != nft_mint {
        return Err(MarketplaceError::NotNftOwner.into());
    }
    
    // Check oracle status to ensure price feed is valid
    check_oracle_status(&ctx.accounts.liquidity_pool)?;
    
    // Check if the NFT is still in cooldown period
    check_cooldown_expired(&ctx.accounts.nft_data)?;
    
    // Calculate token amount to redeem
    // For simplicity in this MVP, we'll use a 1:1 ratio
    // In a production system, you would calculate based on oracle price
    let token_amount: u64 = 1_000_000_000; // 1 token with 9 decimals
    
    // Check if liquidity pool has enough tokens
    if ctx.accounts.lp_token_account.amount < token_amount {
        return Err(MarketplaceError::InsufficientLiquidity.into());
    }
    
    // Transfer tokens from LP account to user
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.lp_token_account.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.liquidity_pool.to_account_info(),
            },
            &[&[
                b"liquidity_pool",
                ctx.accounts.project.key().as_ref(),
                &[ctx.accounts.liquidity_pool.bump],
            ]],
        ),
        token_amount,
    )?;
    
    // Update NFT data to mark as redeemed
    // In a real implementation, you would burn the NFT or transfer it to a null account
    // For this MVP, we'll just close the NFT data account
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    // Update liquidity pool's last activity timestamp
    let liquidity_pool = &mut ctx.accounts.liquidity_pool;
    liquidity_pool.last_activity = Clock::get()?.unix_timestamp;
    
    // Close the NFT data account and refund rent to user
    let nft_data_account_info = ctx.accounts.nft_data.to_account_info();
    let destination_account_info = ctx.accounts.user.to_account_info();
    let rent_balance = nft_data_account_info.lamports();
    
    **nft_data_account_info.try_borrow_mut_lamports()? = 0;
    **destination_account_info.try_borrow_mut_lamports()? = destination_account_info
        .lamports()
        .checked_add(rent_balance)
        .ok_or(MarketplaceError::CalculationOverflow)?;
    
    msg!("NFT redeemed for tokens: {}", nft_mint);
    
    Ok(())
}
