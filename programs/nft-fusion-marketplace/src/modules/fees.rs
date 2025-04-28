use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::{
    state::{PlatformConfig, Project, LiquidityPool},
    errors::MarketplaceError,
};

// Distribute fees from a swap transaction
pub fn distribute_fees<'info>(
    token_program: &Program<'info, Token>,
    lp_token_account: &Account<'info, TokenAccount>,
    platform_treasury: &AccountInfo<'info>,
    project_treasury: &AccountInfo<'info>,
    royalty_wallet: Option<&AccountInfo<'info>>,
    liquidity_pool: &Account<'info, LiquidityPool>,
    platform_config: &Account<'info, PlatformConfig>,
    project: &Account<'info, Project>,
    amount: u64,
) -> Result<()> {
    // Calculate platform fee
    let platform_fee = amount
        .checked_mul(platform_config.platform_fee_basis_points as u64)
        .and_then(|v| v.checked_div(10000))
        .ok_or(MarketplaceError::CalculationOverflow)?;
    
    // Calculate project fee
    let project_fee = amount
        .checked_mul(((10000 - platform_config.platform_fee_basis_points - project.royalty_basis_points) / 2) as u64)
        .and_then(|v| v.checked_div(10000))
        .ok_or(MarketplaceError::CalculationOverflow)?;
    
    // Calculate royalty fee
    let royalty_fee = amount
        .checked_mul(project.royalty_basis_points as u64)
        .and_then(|v| v.checked_div(10000))
        .ok_or(MarketplaceError::CalculationOverflow)?;
    
    // Transfer platform fee
    if platform_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: lp_token_account.to_account_info(),
                    to: platform_treasury.clone(),
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
                Transfer {
                    from: lp_token_account.to_account_info(),
                    to: project_treasury.clone(),
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
    
    // Transfer royalty fee
    if royalty_fee > 0 && royalty_wallet.is_some() {
        token::transfer(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                Transfer {
                    from: lp_token_account.to_account_info(),
                    to: royalty_wallet.unwrap().clone(),
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
