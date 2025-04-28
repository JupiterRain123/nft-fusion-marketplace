use anchor_lang::prelude::*;
use solana_program::clock::Clock;

use crate::{
    state::NftData,
    errors::MarketplaceError,
};

// Check if NFT cooldown period has expired
pub fn check_cooldown_expired(nft_data: &NftData) -> Result<()> {
    if let Some(cooldown_end) = nft_data.cooldown_end_timestamp {
        let current_time = Clock::get()?.unix_timestamp;
        if current_time < cooldown_end {
            return Err(MarketplaceError::NftInCooldown.into());
        }
    }
    
    Ok(())
}

// Calculate remaining cooldown time in seconds
pub fn get_remaining_cooldown(nft_data: &NftData) -> Result<Option<i64>> {
    if let Some(cooldown_end) = nft_data.cooldown_end_timestamp {
        let current_time = Clock::get()?.unix_timestamp;
        if current_time < cooldown_end {
            return Ok(Some(cooldown_end - current_time));
        }
    }
    
    Ok(None)
}
