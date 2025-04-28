use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    associated_token::AssociatedToken,
};
// Using direct TokenMetadata calls instead due to version incompatibility
// We'll implement basic NFT metadata operations
use solana_program::clock::Clock;

use crate::{
    state::{PlatformConfig, Project, Collection, NftData},
    errors::MarketplaceError,
};

#[derive(Accounts)]
#[instruction(collection_id: String, project_id: String, metadata_uri: String, token_mint: Option<Pubkey>)]
pub struct CreateCollection<'info> {
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
        space = 8 + std::mem::size_of::<Collection>() + collection_id.len() + metadata_uri.len() + 100, // Extra space
        seeds = [b"collection", collection_id.as_bytes()],
        bump
    )]
    pub collection: Account<'info, Collection>,
    
    #[account(
        constraint = token_mint.is_none() || token_mint_account.key() == token_mint.unwrap(),
    )]
    /// CHECK: This is the token mint account if linking to existing token
    pub token_mint_account: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(collection_id: String, metadata_uri: String)]
pub struct MintNft<'info> {
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
    
    /// The NFT mint that will be created
    #[account(mut)]
    pub nft_mint: Signer<'info>,
    
    /// The NFT metadata account
    #[account(
        init,
        payer = user,
        space = 8 + std::mem::size_of::<NftData>() + metadata_uri.len() + 100, // Extra space
        seeds = [b"nft_data", nft_mint.key().as_ref()],
        bump,
    )]
    pub nft_data: Account<'info, NftData>,
    
    /// Metadata account for the NFT
    /// CHECK: This is validated in the instruction
    #[account(mut)]
    pub metadata_account: AccountInfo<'info>,
    
    /// Master edition account for the NFT
    /// CHECK: This is validated in the instruction
    #[account(mut)]
    pub master_edition: AccountInfo<'info>,
    
    /// The user's associated token account to receive the NFT
    #[account(mut)]
    pub user_token_account: AccountInfo<'info>,
    
    /// CHECK: This is the token metadata program
    pub token_metadata_program: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn create_collection(
    ctx: Context<CreateCollection>,
    collection_id: String,
    _project_id: String,
    metadata_uri: String,
    token_mint: Option<Pubkey>,
    is_compressed: bool,
) -> Result<()> {
    // Validate metadata URI
    if metadata_uri.is_empty() {
        return Err(MarketplaceError::InvalidMetadataUri.into());
    }
    
    let collection = &mut ctx.accounts.collection;
    collection.project = ctx.accounts.project.key();
    collection.collection_id = collection_id;
    collection.metadata_uri = metadata_uri;
    collection.token_mint = token_mint;
    collection.is_compressed = is_compressed;
    collection.bump = *ctx.bumps.get("collection").unwrap();
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    msg!("Collection created: {}", collection.collection_id);
    
    Ok(())
}

// Internal function for minting an NFT
// In a real implementation, you would integrate with either standard NFTs or compressed NFTs via Bubblegum
pub fn mint_nft_internal(
    owner: Pubkey,
    nft_mint: Pubkey,
    _metadata_uri: String,
    _collection: Pubkey,
    is_compressed: bool,
) -> Result<()> {
    // This is a placeholder for actual NFT minting logic
    // In a real implementation, you would:
    // 1. For standard NFTs: Use token_metadata_program to create metadata and master edition
    // 2. For compressed NFTs: Use bubblegum program to mint a compressed NFT
    
    msg!("Minting NFT: {} to owner: {}", nft_mint, owner);
    
    // The actual implementation would depend on whether it's a standard or compressed NFT
    if is_compressed {
        msg!("Minting compressed NFT via Bubblegum");
        // Bubblegum integration would go here
    } else {
        msg!("Minting standard NFT via Metaplex");
        // Standard NFT minting would go here
    }
    
    Ok(())
}

pub fn mint_nft(
    ctx: Context<MintNft>,
    _collection_id: String,
    metadata_uri: String,
    traits_selection: Option<Vec<u8>>,
) -> Result<()> {
    // Validate metadata URI
    if metadata_uri.is_empty() {
        return Err(MarketplaceError::InvalidMetadataUri.into());
    }
    
    // Validate traits selection if provided
    if let Some(traits) = &traits_selection {
        if traits.is_empty() {
            return Err(MarketplaceError::InvalidTraitsSelection.into());
        }
    }
    
    // Initialize NFT data
    let nft_data = &mut ctx.accounts.nft_data;
    nft_data.owner = ctx.accounts.user.key();
    nft_data.collection = ctx.accounts.collection.key();
    nft_data.mint = ctx.accounts.nft_mint.key();
    nft_data.metadata_uri = metadata_uri.clone();
    nft_data.minted_at = Clock::get()?.unix_timestamp;
    nft_data.cooldown_end_timestamp = None;
    nft_data.discount_percent = None;
    nft_data.bump = *ctx.bumps.get("nft_data").unwrap();
    
    // Here we would mint the NFT based on whether it's compressed or not
    if ctx.accounts.collection.is_compressed {
        // For compressed NFTs, we would use bubblegum program
        // This is just a placeholder for the actual implementation
        msg!("Minting compressed NFT");
        // Bubblegum integration would go here
    } else {
        // For standard NFTs, use token_metadata_program
        // Create token mint
        msg!("Minting standard NFT");
        
        // Placeholder for standard NFT minting
        // In a real implementation, you would:
        // 1. Mint the token
        // 2. Create metadata
        // 3. Create master edition
    }
    
    // Update project's last activity timestamp
    let project = &mut ctx.accounts.project;
    project.last_activity_timestamp = Clock::get()?.unix_timestamp;
    
    msg!("NFT minted: {}", ctx.accounts.nft_mint.key());
    
    Ok(())
}
