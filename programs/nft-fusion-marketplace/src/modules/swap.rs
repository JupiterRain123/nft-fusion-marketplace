use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Token, TokenAccount, Mint, Transfer},
    associated_token::AssociatedToken,
};
use solana_program::clock::Clock;

use crate::{
    state::{PlatformConfig, Project, Collection, LiquidityPool, NftData},
    errors::MarketplaceError,
    modules::{mint::mint_nft_internal, fees::distribute_fees, oracle::check_oracle_status},
};

#[derive(Accounts)]
#[instruction(collection_id: String, token_amount: u64)]
pub struct SwapTokenForNft<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        mut,
        seeds = [b"collection", collection_id.as_bytes()],
        bump,
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
    )]
    pub liquidity_pool: Account<'info, LiquidityPool>,

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

    #[account(
        mut,
        address = platform_config.platform_treasury,
    )]
    /// CHECK: This is the platform treasury account
    pub platform_treasury: AccountInfo<'info>,

    #[account(
        mut,
        address = project.project_treasury,
    )]
    /// CHECK: This is the project treasury account
    pub project_treasury: AccountInfo<'info>,

    #[account(
        mut,
        address = project.royalty_wallet.unwrap_or(project.project_treasury),
    )]
    /// CHECK: This is the royalty wallet account
    pub royalty_wallet: AccountInfo<'info>,

    /// The NFT mint that will be created
    #[account(mut)]
    pub nft_mint: Signer<'info>,

    /// The NFT metadata account
    #[account(
        init,
        payer = user,
        space = 8 + std::mem::size_of::<NftData>() + 256, // Extra space for metadata_uri
        seeds = [b"nft_data", nft_mint.key().as_ref()],
        bump,
    )]
    pub nft_data: Account<'info, NftData>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn swap_token_for_nft(
    ctx: Context<SwapTokenForNft>,
    collection_id: String,
    token_amount: u64,
    discount_percent: Option<u8>,
    cooldown_period: Option<i64>,
) -> Result<()> {
    // Check if collection exists and belongs to the right project
    if ctx.accounts.collection.collection_id != collection_id {
        return Err(MarketplaceError::CollectionNotFound.into());
    }

    // Check if the token mint matches the collection's associated token
    if ctx.accounts.collection.token_mint.is_none() || 
       ctx.accounts.collection.token_mint.unwrap() != ctx.accounts.token_mint.key() {
        return Err(MarketplaceError::NoTokenMintSpecified.into());
    }

    // Check oracle status to ensure price feed is valid
    check_oracle_status(&ctx.accounts.liquidity_pool)?;

    // Calculate token amount required based on oracle price
    // For simplicity in this MVP we assume a 1:1 ratio
    // In a production system, you would calculate based on oracle price
    let required_token_amount = token_amount;
    
    // Apply discount if provided
    let discounted_amount = if let Some(discount) = discount_percent {
        if discount > 100 {
            return Err(MarketplaceError::InvalidDiscountPercentage.into());
        }
        
        required_token_amount
            .checked_mul((100 - discount) as u64)
            .and_then(|v| v.checked_div(100))
            .ok_or(MarketplaceError::CalculationOverflow)?
    } else {
        required_token_amount
    };
    
    // Check if user has enough tokens
    if ctx.accounts.user_token_account.amount < discounted_amount {
        return Err(MarketplaceError::InsufficientTokenAmount.into());
    }

    // Transfer tokens from user to LP account
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.lp_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        discounted_amount,
    )?;

    // Distribute fees
    distribute_fees(
        &ctx.accounts.token_program,
        &ctx.accounts.lp_token_account,
        &ctx.accounts.platform_treasury,
        &ctx.accounts.project_treasury,
        Some(&ctx.accounts.royalty_wallet),
        &ctx.accounts.liquidity_pool,
        &ctx.accounts.platform_config,
        &ctx.accounts.project,
        discounted_amount,
    )?;

    // Set cooldown if discount was applied
    let cooldown_end_timestamp = if discount_percent.is_some() && cooldown_period.is_some() {
        let cooldown = cooldown_period.unwrap();
        if cooldown <= 0 {
            return Err(MarketplaceError::InvalidCooldownPeriod.into());
        }
        
        let current_time = Clock::get()?.unix_timestamp;
        Some(current_time + cooldown)
    } else {
        None
    };

    // Initialize NFT data
    let nft_data = &mut ctx.accounts.nft_data;
    nft_data.owner = ctx.accounts.user.key();
    nft_data.collection = ctx.accounts.collection.key();
    nft_data.mint = ctx.accounts.nft_mint.key();
    nft_data.minted_at = Clock::get()?.unix_timestamp;
    nft_data.cooldown_end_timestamp = cooldown_end_timestamp;
    nft_data.discount_percent = discount_percent;
    nft_data.bump = *ctx.bumps.get("nft_data").unwrap();
    
    // Mint the NFT to the user
    // In a real implementation, you'd call the appropriate NFT minting logic here
    // For this MVP, we'll use a placeholder that would be replaced with actual minting
    mint_nft_internal(
        ctx.accounts.user.key(),
        ctx.accounts.nft_mint.key(),
        String::from("metadata_uri_placeholder"), // Replace with actual metadata URI
        ctx.accounts.collection.key(),
        ctx.accounts.collection.is_compressed,
    )?;
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    // Update liquidity pool's last activity timestamp
    let liquidity_pool = &mut ctx.accounts.liquidity_pool;
    liquidity_pool.last_activity = Clock::get()?.unix_timestamp;
    
    msg!("Token swapped for NFT: {}", ctx.accounts.nft_mint.key());
    
    Ok(())
}
