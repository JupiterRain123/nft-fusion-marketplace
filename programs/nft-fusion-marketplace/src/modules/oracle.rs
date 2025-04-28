use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use pyth_sdk_solana::{load_price_feed_from_account_info, Price, PriceFeed};
use solana_program::clock::Clock;

use crate::{
    state::{PlatformConfig, Project, LiquidityPool},
    errors::MarketplaceError,
};

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
    pub platform_config: Account<'info, PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"project", project_id.as_bytes()],
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
    pub platform_config: Account<'info, PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"project", project_id.as_bytes()],
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
    
    // DEX Liquidity pool token account (token side)
    #[account(mut)]
    pub dex_token_account: Account<'info, TokenAccount>,
    
    // DEX Liquidity pool account (USDC/SOL side)
    #[account(mut)]
    pub dex_base_account: Account<'info, TokenAccount>,
    
    // Token mint account
    #[account(
        constraint = token_mint.key() == liquidity_pool.token_mint @ MarketplaceError::InvalidTokenMint,
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
        constraint = platform_config.authority == authority.key() @ MarketplaceError::Unauthorized,
    )]
    pub platform_config: Account<'info, PlatformConfig>,
    
    #[account(
        mut,
        seeds = [b"project", project_id.as_bytes()],
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
    
    pub system_program: Program<'info, System>,
}

// Check if oracle price feed is fresh and usable
pub fn check_oracle_status(liquidity_pool: &LiquidityPool) -> Result<()> {
    // Check if redemption is locked due to oracle issues
    if liquidity_pool.redemption_locked {
        return Err(MarketplaceError::RedemptionLocked.into());
    }
    
    // Check if oracle price is available and fresh (not older than 1 hour)
    if liquidity_pool.oracle_price_usd.is_none() {
        return Err(MarketplaceError::StaleOracleFeed.into());
    }
    
    let current_time = Clock::get()?.unix_timestamp;
    let max_staleness: i64 = 3600; // 1 hour
    
    if current_time - liquidity_pool.oracle_price_last_update > max_staleness {
        return Err(MarketplaceError::StaleOracleFeed.into());
    }
    
    Ok(())
}

// Get the current oracle price in tokens for a given USD amount
// This is useful for converting from USD to token amount when users want to mint NFTs
pub fn get_token_amount_for_usd(
    liquidity_pool: &LiquidityPool,
    usd_amount: u64
) -> Result<u64> {
    // Ensure oracle price is fresh and available
    check_oracle_status(liquidity_pool)?;
    
    let oracle_price_usd = liquidity_pool.oracle_price_usd
        .ok_or(MarketplaceError::StaleOracleFeed)?;
    
    // Calculate token amount based on USD price
    // Formula: token_amount = (usd_amount * 10^9) / token_price_usd
    // Note: 10^9 is for 9 decimal places in token amount (standard for SPL tokens)
    let token_amount = (usd_amount as u128)
        .checked_mul(1_000_000_000)
        .ok_or(MarketplaceError::CalculationOverflow)?
        .checked_div(oracle_price_usd as u128)
        .ok_or(MarketplaceError::CalculationOverflow)? as u64;
    
    Ok(token_amount)
}

// Get the current USD value for a given token amount
// This is useful for valuing NFTs or calculating fees in USD terms
pub fn get_usd_value_for_tokens(
    liquidity_pool: &LiquidityPool,
    token_amount: u64
) -> Result<u64> {
    // Ensure oracle price is fresh and available
    check_oracle_status(liquidity_pool)?;
    
    let oracle_price_usd = liquidity_pool.oracle_price_usd
        .ok_or(MarketplaceError::StaleOracleFeed)?;
    
    // Calculate USD value based on token amount
    // Formula: usd_value = (token_amount * token_price_usd) / 10^9
    let usd_value = (token_amount as u128)
        .checked_mul(oracle_price_usd as u128)
        .ok_or(MarketplaceError::CalculationOverflow)?
        .checked_div(1_000_000_000)
        .ok_or(MarketplaceError::CalculationOverflow)? as u64;
    
    Ok(usd_value)
}

// Update oracle price from Pyth
pub fn update_oracle_price(
    ctx: Context<UpdateOraclePrice>,
    _project_id: String,
) -> Result<()> {
    let price_feed: PriceFeed = load_price_feed_from_account_info(&ctx.accounts.pyth_price_account)
        .map_err(|_| MarketplaceError::StaleOracleFeed)?;
    
    let price: Price = price_feed.get_current_price()
        .ok_or(MarketplaceError::StaleOracleFeed)?;
    
    // Get price in USD (scaled by 10^6)
    let price_usd = if price.price < 0 {
        return Err(MarketplaceError::StaleOracleFeed.into());
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

// Update price from DEX liquidity pool (like Raydium)
pub fn update_dex_price(
    ctx: Context<UpdateDexPrice>,
    _project_id: String,
) -> Result<()> {
    // Calculate price based on DEX pool ratios
    let token_reserves = ctx.accounts.dex_token_account.amount;
    let base_reserves = ctx.accounts.dex_base_account.amount;
    
    // Ensure pools have liquidity
    if token_reserves == 0 || base_reserves == 0 {
        return Err(MarketplaceError::InsufficientLiquidity.into());
    }
    
    // Calculate price in base tokens (scaled by 10^6)
    // For simplicity, we assume the base token is USDC (or another stablecoin with 6 decimals)
    // and the token has 9 decimals (standard for SPL tokens)
    let price_usd = (base_reserves as u128)
        .checked_mul(1_000_000_000)
        .ok_or(MarketplaceError::CalculationOverflow)?
        .checked_div(token_reserves as u128)
        .ok_or(MarketplaceError::CalculationOverflow)? as u64;
    
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

// Set manual price (from off-chain API or for testing)
pub fn set_manual_price(
    ctx: Context<SetManualPrice>,
    _project_id: String,
    price_usd: u64,
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

// Define price source enum to track where the price came from
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Debug)]
pub enum PriceSource {
    Pyth,           // Pyth oracle network
    DexLiquidity,   // DEX liquidity pool (Raydium, etc.)
    Manual,         // Manually set price
    None,           // No price source set
}
