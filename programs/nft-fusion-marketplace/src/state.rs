use anchor_lang::prelude::*;

#[account]
pub struct PlatformConfig {
    pub authority: Pubkey,
    pub platform_fee_basis_points: u16,  // In basis points (100 = 1%)
    pub platform_treasury: Pubkey,
    pub bump: u8,
}

#[account]
pub struct Project {
    pub authority: Pubkey,
    pub project_id: String,       // Unique identifier for the project
    pub project_treasury: Pubkey, // Treasury wallet for the project
    pub royalty_wallet: Option<Pubkey>, // Optional royalty wallet
    pub royalty_basis_points: u16, // Royalty fee in basis points
    pub last_activity_timestamp: i64, // Last activity timestamp for inactivity monitoring
    pub is_active: bool,          // Project active status
    pub bump: u8,
}

#[account]
pub struct Collection {
    pub project: Pubkey,           // Project account this collection belongs to
    pub collection_id: String,     // Unique identifier for the collection
    pub metadata_uri: String,      // Metadata URI for the collection
    pub token_mint: Option<Pubkey>, // Associated token mint (if any)
    pub is_compressed: bool,       // Whether this collection uses compressed NFTs
    pub bump: u8,
}

#[account]
pub struct LiquidityPool {
    pub project: Pubkey,          // Project account this LP belongs to
    pub token_mint: Pubkey,       // Token mint for this liquidity pool
    pub lp_token_account: Pubkey, // Token account holding LP tokens
    pub created_at: i64,          // Creation timestamp
    pub last_activity: i64,       // Last activity timestamp
    pub oracle_price_usd: Option<u64>, // Latest oracle price in USD (scaled by 10^6)
    pub oracle_price_last_update: i64, // Last oracle price update timestamp
    pub redemption_locked: bool,   // Whether redemption is locked due to oracle issues
    pub price_source: crate::modules::oracle::PriceSource, // Source of price data
    pub bump: u8,
}

#[account]
pub struct NftData {
    pub owner: Pubkey,           // Current owner of the NFT
    pub collection: Pubkey,      // Collection account this NFT belongs to
    pub mint: Pubkey,            // NFT mint address
    pub metadata_uri: String,    // Metadata URI for this specific NFT
    pub minted_at: i64,          // Mint timestamp
    pub cooldown_end_timestamp: Option<i64>, // End of cooldown period (if any)
    pub discount_percent: Option<u8>, // Discount percentage applied (if any)
    pub fusion_level: u8,        // Fusion level (0 for base NFTs, higher for fused NFTs)
    pub parent_nfts: Option<Vec<Pubkey>>, // Parent NFTs used in fusion process (if any)
    pub rarity_score: u16,       // Rarity score (higher is rarer)
    pub bump: u8,
}

#[account]
pub struct FusionConfig {
    pub project: Pubkey,         // Project account this fusion config belongs to
    pub collection: Pubkey,      // Collection account this fusion config belongs to
    pub min_nfts_required: u8,   // Minimum number of NFTs required for fusion
    pub max_nfts_allowed: u8,    // Maximum number of NFTs allowed for fusion
    pub base_success_rate: u8,   // Base success rate for fusion (0-100)
    pub token_burn_percent: u8,  // Percentage of input NFT value to burn (0-100)
    pub cooldown_period: i64,    // Cooldown period after fusion (in seconds)
    pub is_active: bool,         // Whether fusion is active for this collection
    pub bump: u8,
}

#[account]
pub struct TokenEscrow {
    pub owner: Pubkey,           // Original token owner
    pub nft_mint: Pubkey,        // Associated NFT mint 
    pub token_mint: Pubkey,      // Token mint address
    pub token_amount: u64,       // Amount of tokens in escrow
    pub escrow_token_account: Pubkey, // Token account holding escrowed tokens
    pub discount_percent: Option<u8>,  // Discount on redemption (if any)
    pub vesting_end_timestamp: Option<i64>, // End of vesting period (if any)
    pub is_active: bool,         // Whether this escrow is active
    pub created_at: i64,         // Creation timestamp
    pub bump: u8,
}

#[account]
pub struct NftListing {
    pub owner: Pubkey,           // NFT owner
    pub nft_mint: Pubkey,        // NFT mint address
    pub token_mint: Pubkey,      // Token mint for payment
    pub asking_price: u64,       // Price in tokens
    pub discount_percent: Option<u8>, // Discount if applicable
    pub cooldown_period: Option<i64>, // Cooldown before redemption
    pub is_active: bool,         // Whether this listing is active
    pub created_at: i64,         // Creation timestamp
    pub collection: Pubkey,      // Collection account the NFT belongs to
    pub bump: u8,
}

// Trait definition structures for NFT attributes
#[account]
pub struct TraitType {
    pub collection: Pubkey,      // Collection this trait type belongs to
    pub name: String,            // Name of trait category (e.g., "Background", "Eyes", "Mouth")
    pub is_required: bool,       // Whether this trait is required for all NFTs
    pub trait_values: Vec<TraitValue>, // List of available values for this trait
    pub bump: u8,
}

// Implement AsRef<TraitType> for TraitType
impl AsRef<TraitType> for TraitType {
    fn as_ref(&self) -> &TraitType {
        self
    }
}

// Implementation for &TraitType is now handled by core's blanket implementation
// for <T: AsRef<U>, U> which already covers this case

// Remove Deref implementation as it's causing recursion issues
// when auto-dereferencing

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct TraitValue {
    pub name: String,            // Name of the trait value (e.g., "Blue" for eye color)
    pub uri_postfix: String,     // Postfix to add to base URI (for asset loading)
    pub rarity_weight: u16,      // Weight for random selection (higher = more common)
    pub available_supply: Option<u32>, // Optional limited supply for this trait
    pub used_supply: u32,        // How many times this trait has been used
}

// Collection traits configuration
#[account]
pub struct CollectionTraitConfig {
    pub collection: Pubkey,      // Collection this config belongs to
    pub base_uri: String,        // Base URI for all NFTs in collection
    pub auto_generation_enabled: bool, // Whether auto-generation is enabled
    pub metadata_format: MetadataFormat, // Format of metadata (JSON, etc.)
    pub trait_types: Vec<Pubkey>, // List of trait type accounts
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Debug)]
pub enum MetadataFormat {
    StandardJson,                // Standard JSON metadata format
    CompressedJson,              // Compressed JSON format for on-chain storage
    Custom,                      // Custom format defined by project
}

// NFT traits record
#[account]
pub struct NftTraits {
    pub nft_mint: Pubkey,        // NFT mint address
    pub collection: Pubkey,      // Collection account the NFT belongs to
    pub trait_values: Vec<(String, String)>, // (trait type name, trait value name) pairs
    pub is_auto_generated: bool, // Whether traits were auto-generated
    pub generation_seed: Option<[u8; 32]>, // Seed used for auto-generation if applicable
    pub bump: u8,
}
