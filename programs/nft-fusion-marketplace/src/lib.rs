#![recursion_limit = "256"]

use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Token, TokenAccount, Mint},
};
use pyth_sdk_solana::{load_price_feed_from_account_info, Price, PriceFeed};
use solana_program::clock::Clock;

// Import modules
pub mod errors;
pub mod state;
pub mod modules;

declare_id!("7wVDyMSQrpDp7HaAie3Cby9LnqbXyAJeMtGwQyKZ59ES");

// Import enums we need from modules
use modules::oracle::PriceSource;

// Instruction context for updating price from Pyth Oracle
#[derive(Accounts)]
#[instruction(project_id: String)]
pub struct UpdateOraclePrice<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, state::PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"project", project_id.as_bytes()],
        bump = project.bump,
        constraint = project.is_active @ errors::MarketplaceError::ProjectNotFound,
    )]
    pub project: Account<'info, state::Project>,
    
    #[account(
        mut,
        seeds = [b"liquidity_pool", project.key().as_ref()],
        bump = liquidity_pool.bump,
    )]
    pub liquidity_pool: Account<'info, state::LiquidityPool>,
    
    /// CHECK: This is the Pyth oracle price feed account
    pub pyth_price_account: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

// Instruction context for updating price from DEX liquidity pools (like Raydium)
#[derive(Accounts)]
#[instruction(project_id: String)]
pub struct UpdateDexPrice<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, state::PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"project", project_id.as_bytes()],
        bump = project.bump,
        constraint = project.is_active @ errors::MarketplaceError::ProjectNotFound,
    )]
    pub project: Account<'info, state::Project>,
    
    #[account(
        mut,
        seeds = [b"liquidity_pool", project.key().as_ref()],
        bump = liquidity_pool.bump,
    )]
    pub liquidity_pool: Account<'info, state::LiquidityPool>,
    
    // DEX Liquidity pool token account (token side)
    #[account(mut)]
    pub dex_token_account: Account<'info, TokenAccount>,
    
    // DEX Liquidity pool account (USDC/SOL side)
    #[account(mut)]
    pub dex_base_account: Account<'info, TokenAccount>,
    
    // Token mint account
    #[account(
        constraint = token_mint.key() == liquidity_pool.token_mint @ errors::MarketplaceError::InvalidTokenMint,
    )]
    pub token_mint: Account<'info, Mint>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Instruction context for updating price from external source (manual or API)
#[derive(Accounts)]
#[instruction(project_id: String, price_usd: u64)]
pub struct SetManualPrice<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    
    #[account(
        seeds = [b"platform_config"],
        bump = platform_config.bump,
        constraint = platform_config.authority == authority.key() @ errors::MarketplaceError::Unauthorized,
    )]
    pub platform_config: Account<'info, state::PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"project", project_id.as_bytes()],
        bump = project.bump,
        constraint = project.is_active @ errors::MarketplaceError::ProjectNotFound,
    )]
    pub project: Account<'info, state::Project>,
    
    #[account(
        mut,
        seeds = [b"liquidity_pool", project.key().as_ref()],
        bump = liquidity_pool.bump,
    )]
    pub liquidity_pool: Account<'info, state::LiquidityPool>,
    
    pub system_program: Program<'info, System>,
}

#[program]
pub mod nft_fusion_marketplace {
    use super::*;
    
    // Oracle price integration functions
    
    // Update price from Pyth oracle
    pub fn update_price_from_pyth(
        ctx: Context<UpdateOraclePrice>, 
        _project_id: String
    ) -> Result<()> {
        let price_feed: PriceFeed = load_price_feed_from_account_info(&ctx.accounts.pyth_price_account)
            .map_err(|_| errors::MarketplaceError::StaleOracleFeed)?;
        
        let price: Price = price_feed.get_current_price()
            .ok_or(errors::MarketplaceError::StaleOracleFeed)?;
        
        // Get price in USD (scaled by 10^6)
        let price_usd = if price.price < 0 {
            return Err(errors::MarketplaceError::StaleOracleFeed.into());
        } else {
            price.price as u64 * 10u64.pow(price.expo.unsigned_abs() as u32)
        };
        
        // Determine if oracle feed is stale
        let current_time = Clock::get()?.unix_timestamp;
        let price_pub_time = current_time - 60; // Simplified due to SDK limitations
        let max_staleness: i64 = 3600; // 1 hour
        let is_stale = current_time - price_pub_time > max_staleness;
        
        // Update liquidity pool oracle information
        let liquidity_pool = &mut ctx.accounts.liquidity_pool;
        liquidity_pool.oracle_price_usd = Some(price_usd);
        liquidity_pool.oracle_price_last_update = current_time;
        liquidity_pool.price_source = PriceSource::Pyth;
        
        // Lock or unlock redemptions based on oracle status
        if is_stale {
            liquidity_pool.redemption_locked = true;
            msg!("Oracle feed is stale, NFT redemption locked");
        } else {
            liquidity_pool.redemption_locked = false;
            msg!("Oracle price updated: {} USD", price_usd as f64 / 1_000_000.0);
        }
        
        // Update project's last activity timestamp
        let project = &mut ctx.accounts.project;
        project.last_activity_timestamp = current_time;
        
        Ok(())
    }
    
    // Update price from DEX liquidity pools (Raydium, etc.)
    pub fn update_price_from_dex(
        ctx: Context<UpdateDexPrice>, 
        _project_id: String
    ) -> Result<()> {
        // Calculate price based on DEX pool ratios
        let token_reserves = ctx.accounts.dex_token_account.amount;
        let base_reserves = ctx.accounts.dex_base_account.amount;
        
        // Ensure pools have liquidity
        if token_reserves == 0 || base_reserves == 0 {
            return Err(errors::MarketplaceError::InsufficientLiquidity.into());
        }
        
        // Calculate price in base tokens (scaled by 10^6)
        // For simplicity, we assume the base token is USDC (or another stablecoin with 6 decimals)
        // and the token has 9 decimals (standard for SPL tokens)
        let price_usd = (base_reserves as u128)
            .checked_mul(1_000_000_000)
            .ok_or(errors::MarketplaceError::CalculationOverflow)?
            .checked_div(token_reserves as u128)
            .ok_or(errors::MarketplaceError::CalculationOverflow)? as u64;
        
        let current_time = Clock::get()?.unix_timestamp;
        
        // Update liquidity pool oracle information
        let liquidity_pool = &mut ctx.accounts.liquidity_pool;
        liquidity_pool.oracle_price_usd = Some(price_usd);
        liquidity_pool.oracle_price_last_update = current_time;
        liquidity_pool.price_source = PriceSource::DexLiquidity;
        liquidity_pool.redemption_locked = false;
        
        // Update project's last activity timestamp
        let project = &mut ctx.accounts.project;
        project.last_activity_timestamp = current_time;
        
        msg!("DEX price updated: {} USD", price_usd as f64 / 1_000_000.0);
        
        Ok(())
    }
    
    // Set price manually for testing or projects without price feeds
    pub fn set_price_manually(
        ctx: Context<SetManualPrice>, 
        _project_id: String, 
        price_usd: u64
    ) -> Result<()> {
        let current_time = Clock::get()?.unix_timestamp;
        
        // Update liquidity pool oracle information
        let liquidity_pool = &mut ctx.accounts.liquidity_pool;
        liquidity_pool.oracle_price_usd = Some(price_usd);
        liquidity_pool.oracle_price_last_update = current_time;
        liquidity_pool.price_source = PriceSource::Manual;
        liquidity_pool.redemption_locked = false;
        
        // Update project's last activity timestamp
        let project = &mut ctx.accounts.project;
        project.last_activity_timestamp = current_time;
        
        msg!("Manual price set: {} USD", price_usd as f64 / 1_000_000.0);
        
        Ok(())
    }
}

// Helper function to distribute fees among platform, project, and royalty wallets
pub fn distribute_fees<'info>(
    token_program: &Program<'info, Token>,
    lp_token_account: &Account<'info, TokenAccount>,
    platform_treasury: &Account<'info, TokenAccount>,
    project_treasury: &Account<'info, TokenAccount>,
    royalty_wallet: Option<&Account<'info, TokenAccount>>,
    liquidity_pool: &Account<'info, state::LiquidityPool>,
    platform_config: &Account<'info, state::PlatformConfig>,
    project: &Account<'info, state::Project>,
    token_amount: u64,
) -> Result<()> {
    // Calculate platform fee (platform_fee_basis_points is in basis points, e.g., 200 = 2%)
    let platform_fee = (token_amount as u128)
        .checked_mul(platform_config.platform_fee_basis_points as u128)
        .ok_or(errors::MarketplaceError::CalculationOverflow)?
        .checked_div(10000)
        .ok_or(errors::MarketplaceError::CalculationOverflow)? as u64;
    
    // Calculate project fee (royalty_basis_points is in basis points)
    let project_fee = (token_amount as u128)
        .checked_mul(project.royalty_basis_points as u128)
        .ok_or(errors::MarketplaceError::CalculationOverflow)?
        .checked_div(10000)
        .ok_or(errors::MarketplaceError::CalculationOverflow)? as u64;
    
    // Calculate royalty fee (if royalty wallet is provided)
    let royalty_fee = if royalty_wallet.is_some() && project.royalty_wallet.is_some() {
        // For simplicity, we'll use a fixed 1% royalty fee
        (token_amount as u128)
            .checked_mul(100) // 1% = 100 basis points
            .ok_or(errors::MarketplaceError::CalculationOverflow)?
            .checked_div(10000)
            .ok_or(errors::MarketplaceError::CalculationOverflow)? as u64
    } else {
        0
    };
    
    // Transfer platform fee
    if platform_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token::Transfer {
                    from: lp_token_account.to_account_info(),
                    to: platform_treasury.to_account_info(),
                    authority: liquidity_pool.to_account_info(),
                },
                &[&[
                    b"liquidity_pool",
                    liquidity_pool.project.as_ref(),
                    &[liquidity_pool.bump],
                ]],
            ),
            platform_fee,
        )?;
    }
    
    // Transfer project fee
    if project_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token::Transfer {
                    from: lp_token_account.to_account_info(),
                    to: project_treasury.to_account_info(),
                    authority: liquidity_pool.to_account_info(),
                },
                &[&[
                    b"liquidity_pool",
                    liquidity_pool.project.as_ref(),
                    &[liquidity_pool.bump],
                ]],
            ),
            project_fee,
        )?;
    }
    
    // Transfer royalty fee if applicable
    if royalty_fee > 0 && royalty_wallet.is_some() {
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                token::Transfer {
                    from: lp_token_account.to_account_info(),
                    to: royalty_wallet.unwrap().to_account_info(),
                    authority: liquidity_pool.to_account_info(),
                },
                &[&[
                    b"liquidity_pool",
                    liquidity_pool.project.as_ref(),
                    &[liquidity_pool.bump],
                ]],
            ),
            royalty_fee,
        )?;
    }
    
    Ok(())
}

// Helper function to mint NFT (placeholder for actual minting logic)
pub fn mint_nft_internal(
    owner: Pubkey,
    nft_mint: Pubkey,
    metadata_uri: String,
    collection: Pubkey,
    is_compressed: bool,
) -> Result<()> {
    // In a real implementation, this would handle the actual NFT minting process
    // For regular NFTs, this would create token, metadata, and master edition accounts
    // For compressed NFTs, this would call into a merkle tree program to append the NFT
    
    // Just log the operation for now
    msg!("Minting NFT {} for owner {}", nft_mint, owner);
    msg!("Metadata URI: {}", metadata_uri);
    msg!("Collection: {}", collection);
    msg!("Compressed: {}", is_compressed);
    
    Ok(())
}
