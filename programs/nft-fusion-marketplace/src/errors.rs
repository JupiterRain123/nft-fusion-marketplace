use anchor_lang::prelude::*;

#[error_code]
pub enum MarketplaceError {
    #[msg("Invalid platform fee value. Must be less than 10000.")]
    InvalidPlatformFee,
    
    #[msg("Invalid royalty fee value. Must be less than 10000.")]
    InvalidRoyaltyFee,
    
    #[msg("Project ID already exists.")]
    ProjectAlreadyExists,
    
    #[msg("Project ID does not exist.")]
    ProjectNotFound,
    
    #[msg("Project is not active.")]
    ProjectNotActive,
    
    #[msg("Collection ID already exists for this project.")]
    CollectionAlreadyExists,
    
    #[msg("Collection ID does not exist.")]
    CollectionNotFound,
    
    #[msg("No token mint specified for collection.")]
    NoTokenMintSpecified,
    
    #[msg("Oracle price feed is stale or unavailable.")]
    StaleOracleFeed,
    
    #[msg("NFT is still in cooldown period.")]
    NftInCooldown,
    
    #[msg("Insufficient token amount for the swap.")]
    InsufficientTokenAmount,
    
    #[msg("Invalid discount percentage. Must be between 0 and 100.")]
    InvalidDiscountPercentage,
    
    #[msg("Insufficient liquidity in the pool.")]
    InsufficientLiquidity,
    
    #[msg("Invalid cooldown period. Must be greater than 0.")]
    InvalidCooldownPeriod,
    
    #[msg("NFT redemption is locked due to oracle issues.")]
    RedemptionLocked,
    
    #[msg("Liquidity pool already exists for this project.")]
    LiquidityPoolAlreadyExists,
    
    #[msg("Liquidity pool does not exist for this project.")]
    LiquidityPoolNotFound,
    
    #[msg("NFT metadata URI is invalid or empty.")]
    InvalidMetadataUri,
    
    #[msg("Invalid traits selection for minting.")]
    InvalidTraitsSelection,
    
    #[msg("NFT is not owned by the user.")]
    NotNftOwner,
    
    #[msg("Liquidity pool is not inactive.")]
    LiquidityPoolNotInactive,
    
    #[msg("Operation not permitted for the current user.")]
    Unauthorized,
    
    #[msg("Calculation overflow occurred.")]
    CalculationOverflow,
    
    #[msg("Invalid token mint provided.")]
    InvalidTokenMint,
    
    #[msg("Invalid token account provided.")]
    InvalidTokenAccount,
    
    #[msg("Invalid token amount for operation.")]
    InvalidTokenAmount,
    
    #[msg("Token account has insufficient balance.")]
    InsufficientTokenBalance,
    
    #[msg("Invalid NFT for fusion.")]
    InvalidNftForFusion,
    
    #[msg("Not enough NFTs provided for fusion.")]
    NotEnoughNftsForFusion,
    
    #[msg("NFTs must belong to the same collection for fusion.")]
    MixedCollections,
    
    #[msg("Fusion algorithm error occurred.")]
    FusionAlgorithmError,
    
    #[msg("Escrow account is not active.")]
    EscrowNotActive,
    
    #[msg("Vesting period has not ended.")]
    VestingPeriodActive,
    
    #[msg("Not authorized to redeem from this escrow.")]
    UnauthorizedEscrowRedemption,
    
    #[msg("NFT listing is not active.")]
    ListingNotActive,
    
    #[msg("Not authorized to manage this listing.")]
    UnauthorizedListingOperation,
    
    #[msg("Token price is too low.")]
    TokenPriceTooLow,
    
    #[msg("Escrow already exists for this NFT.")]
    EscrowAlreadyExists,
    
    #[msg("Listing already exists for this NFT.")]
    ListingAlreadyExists,
    
    #[msg("Invalid token escrow for this NFT.")]
    InvalidTokenEscrow,
    
    #[msg("Not the owner of this token escrow.")]
    NotTokenEscrowOwner,
    
    #[msg("Token escrow is not active.")]
    TokenEscrowNotActive,
    
    #[msg("Fee calculation error.")]
    FeeCalculationError,
    
    // Trait-related errors
    #[msg("Trait type not found in collection.")]
    TraitTypeNotFound,
    
    #[msg("Trait value not found in trait type.")]
    TraitValueNotFound,
    
    #[msg("Required trait type missing in provided traits.")]
    RequiredTraitMissing,
    
    #[msg("Trait auto-generation failed.")]
    TraitGenerationFailed,
    
    #[msg("Trait value exceeds available supply.")]
    TraitSupplyExceeded,
    
    #[msg("Invalid trait configuration.")]
    InvalidTraitConfig,
    
    #[msg("Auto-generation not enabled for this collection.")]
    AutoGenerationDisabled,
    
    #[msg("Trait validation failed.")]
    TraitValidationFailed,
}
