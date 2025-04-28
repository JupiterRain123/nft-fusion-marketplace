use anchor_lang::prelude::*;
use anchor_spl::{
    token::{self, Mint, Token, TokenAccount, Transfer},
    associated_token::AssociatedToken,
};
use solana_program::clock::Clock;

use crate::{
    state::{PlatformConfig, Project, LiquidityPool},
    errors::MarketplaceError,
    modules::oracle::PriceSource,
};

// Make struct explicitly implement Accounts trait
#[derive(Accounts)]
#[instruction(project_id: String, token_mint: Pubkey)]
pub struct SetupLiquidityPool<'info> {
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
        constraint = project.authority == authority.key() @ MarketplaceError::Unauthorized,
    )]
    pub project: Account<'info, Project>,
    
    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<LiquidityPool>(),
        seeds = [b"liquidity_pool", project.key().as_ref()],
        bump
    )]
    pub liquidity_pool: Account<'info, LiquidityPool>,
    
    #[account(
        constraint = token_mint_account.key() == token_mint,
    )]
    pub token_mint_account: Account<'info, Mint>,
    
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = token_mint_account,
        associated_token::authority = liquidity_pool,
    )]
    pub lp_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = authority_token_account.owner == authority.key(),
        constraint = authority_token_account.mint == token_mint,
    )]
    pub authority_token_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(project_id: String)]
pub struct CheckLpInactivity<'info> {
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
        constraint = lp_token_account.key() == liquidity_pool.lp_token_account,
    )]
    pub lp_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        address = platform_config.platform_treasury,
    )]
    /// CHECK: This is the platform treasury account
    pub platform_treasury: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// Setup a liquidity pool for a project
pub fn setup_liquidity_pool(
    ctx: Context<SetupLiquidityPool>,
    project_id: String,
    token_mint: Pubkey,
    initial_liquidity: u64,
) -> Result<()> {
    // Initialize liquidity pool
    let liquidity_pool = &mut ctx.accounts.liquidity_pool;
    liquidity_pool.project = ctx.accounts.project.key();
    liquidity_pool.token_mint = token_mint;
    liquidity_pool.lp_token_account = ctx.accounts.lp_token_account.key();
    liquidity_pool.created_at = Clock::get()?.unix_timestamp;
    liquidity_pool.last_activity = Clock::get()?.unix_timestamp;
    liquidity_pool.oracle_price_usd = None; // Will be updated by oracle module
    liquidity_pool.oracle_price_last_update = 0;
    liquidity_pool.redemption_locked = false;
    liquidity_pool.price_source = PriceSource::None; // No price source set yet
    liquidity_pool.bump = *ctx.bumps.get("liquidity_pool").unwrap();
    
    // Transfer initial liquidity if provided
    if initial_liquidity > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.authority_token_account.to_account_info(),
                    to: ctx.accounts.lp_token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            initial_liquidity,
        )?;
    }
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    msg!("Liquidity pool created for project: {}", project_id);
    
    Ok(())
}

// Check if liquidity pool is inactive and reclaim if needed
pub fn check_lp_inactivity(
    ctx: Context<CheckLpInactivity>,
    project_id: String,
) -> Result<()> {
    // Check if liquidity pool is inactive (6 months = 15,768,000 seconds)
    let current_time = Clock::get()?.unix_timestamp;
    let inactivity_period: i64 = 15_768_000;
    let last_activity = ctx.accounts.liquidity_pool.last_activity;
    
    if current_time - last_activity < inactivity_period {
        return Err(MarketplaceError::LiquidityPoolNotInactive.into());
    }
    
    // If inactive, reclaim liquidity to platform treasury
    let liquidity_amount = ctx.accounts.lp_token_account.amount;
    
    if liquidity_amount > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.lp_token_account.to_account_info(),
                    to: ctx.accounts.platform_treasury.to_account_info(),
                    authority: ctx.accounts.liquidity_pool.to_account_info(),
                },
                &[&[
                    b"liquidity_pool",
                    ctx.accounts.project.key().as_ref(),
                    &[ctx.accounts.liquidity_pool.bump],
                ]],
            ),
            liquidity_amount,
        )?;
    }
    
    // Mark project as inactive
    let project = &mut ctx.accounts.project;
    project.is_active = false;
    
    msg!("Inactive liquidity pool reclaimed for project: {}", project_id);
    
    Ok(())
}
