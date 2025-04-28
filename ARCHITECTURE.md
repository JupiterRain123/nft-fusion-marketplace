# NFT Fusion Marketplace Architecture

This document outlines the architecture and key components of the NFT Fusion Marketplace Solana program.

## Core Concepts

### Escrow-Based Swapping
The marketplace uses an escrow-based approach for secure token/NFT swapping, rather than a liquidity pool model. This provides:
- Individual ownership tracking
- Support for discounts and premiums
- Vesting and cooldown capabilities
- Isolated security (failures affect only individual escrows)

### Triple-Source Oracle
Price information is obtained from three possible sources to ensure reliability:
1. **Pyth Network** - For established tokens with Pyth price feeds
2. **DEX Liquidity Pools** - For tokens with active trading on DEXes like Raydium
3. **Manual Price Setting** - For new projects or testing environments

### NFT Fusion Mechanics
The program supports combining NFTs to create higher-tier NFTs with:
- Rarity scoring system
- Trait inheritance and bonuses
- Cooldown periods for controlling fusion frequency
- Burn mechanisms to manage token supply

## Account Structure

| Account Type | Purpose |
|--------------|---------|
| `PlatformConfig` | Global configuration for the marketplace |
| `Project` | Individual projects using the marketplace |
| `Collection` | NFT collection within a project |
| `LiquidityPool` | Manages token/NFT exchange pricing and oracle data |
| `NftData` | Metadata about individual NFTs |
| `FusionConfig` | Configuration for NFT fusion mechanics |
| `TokenEscrow` | Holds tokens in escrow for NFT redemption |
| `NftListing` | Lists NFTs available for purchase with tokens |
| `TraitType` | Defines NFT trait categories (e.g., "Background", "Eyes") |
| `CollectionTraitConfig` | Configuration for NFT traits generation |
| `NftTraits` | Records traits associated with a specific NFT |

## System Workflow

### Token to NFT Flow
1. User deposits tokens into an escrow account
2. Tokens are locked with appropriate vesting parameters
3. NFT is minted to the user or transferred from a collection
4. Fees are distributed among platform, project, and royalty recipients

### NFT Fusion Flow
1. User provides multiple NFTs for fusion
2. Rarity of input NFTs is evaluated
3. Fusion success is calculated based on configuration
4. If successful, input NFTs are burned and a new NFT is created
5. The new NFT receives traits based on parents and rarity bonuses

### Price Oracle Flow
1. Oracle price is updated from one of three sources (Pyth, DEX, Manual)
2. Price staleness is checked to ensure fresh data
3. If oracle data is stale, redemptions can be locked
4. Price data is used for token/NFT exchange rate calculations

## Module Organization

The codebase is organized into functional modules:

- `oracle.rs` - Manages price feeds and oracle integration
- `escrow.rs` - Handles token escrow functionality
- `swap.rs` - Implements token/NFT swap logic
- `mint.rs` - Controls NFT minting process
- `rarity.rs` - Calculates NFT rarity scores
- `traits.rs` - Manages NFT traits and attributes
- `redeem.rs` - Processes token redemption from escrows
- `cooldown.rs` - Implements cooldown period logic
- `fees.rs` - Calculates and distributes fees
- `lp.rs` - Manages liquidty pool operations

## Security Considerations

The program implements several security features:

1. **Isolated Escrows**: Individual escrows prevent systemic vulnerabilities
2. **Authority Checks**: All sensitive operations require proper authority signatures
3. **Oracle Validation**: Multiple price sources with staleness checking
4. **Supply Limits**: Controls on trait usage and NFT creation
5. **Fee Distribution**: Automatic fee distribution prevents missed payments

## Integration Points

### External Program Dependencies

- **Token Program**: Standard SPL token operations
- **Associated Token Account Program**: Managing token accounts
- **Metadata Program**: Creating and updating NFT metadata
- **Pyth Oracle Program**: Getting external price feeds
- **State Compression Program** (optional): Supporting compressed NFTs

### Client Integration

Clients can interact with the program through:
1. Anchor framework client libraries (TypeScript/JavaScript)
2. Direct Solana transaction construction
3. Custom SDKs built on top of the base functionality