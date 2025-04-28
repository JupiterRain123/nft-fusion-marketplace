# NFT Fusion Marketplace - Deployment Guide

This document provides step-by-step instructions for building and deploying the NFT Fusion Marketplace Solana program.

## Prerequisites

- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools) (v1.9.29 recommended)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Anchor Framework](https://www.anchor-lang.com/docs/installation) (v0.24.2)
- [Node.js](https://nodejs.org/) (v16 or later)
- [Git](https://git-scm.com/downloads)

## Development Environment Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/nft-fusion-marketplace.git
   cd nft-fusion-marketplace
   ```

2. Install dependencies:
   ```bash
   npm install
   ```

3. Build the program:
   ```bash
   anchor build
   ```

## Local Deployment (Localnet)

1. Start a local validator:
   ```bash
   solana-test-validator
   ```

2. Deploy the program:
   ```bash
   anchor deploy
   ```

3. Run tests to verify deployment:
   ```bash
   anchor test
   ```

## Devnet Deployment

1. Configure Solana CLI to use devnet:
   ```bash
   solana config set --url https://api.devnet.solana.com
   ```

2. Create or import a keypair for deployment:
   ```bash
   solana-keygen new -o deploy/keypairs/wallet-keypair.json
   # Or import existing
   # solana-keygen recover -o deploy/keypairs/wallet-keypair.json
   ```

3. Airdrop SOL to your wallet (for devnet):
   ```bash
   solana airdrop 2 $(solana address -k deploy/keypairs/wallet-keypair.json)
   ```

4. Build and deploy:
   ```bash
   anchor build
   anchor deploy --program-keypair deploy/keypairs/nft_fusion_marketplace-keypair.json --provider.wallet deploy/keypairs/wallet-keypair.json
   ```

## Mainnet Deployment

1. Configure Solana CLI to use mainnet:
   ```bash
   solana config set --url https://api.mainnet-beta.solana.com
   ```

2. Use a secure keypair with sufficient SOL:
   ```bash
   # Do NOT generate a new keypair for mainnet - use a hardware wallet or other secure solution
   # Example with hardware wallet like Ledger:
   solana-keygen pubkey usb://ledger
   ```

3. Deploy with Anchor:
   ```bash
   anchor build
   anchor deploy --provider.cluster mainnet --provider.wallet /path/to/secure/wallet.json
   ```

## Solana Playground Deployment (Alternative)

If you prefer a web-based deployment, you can use Solana Playground:

1. Visit [Solana Playground](https://beta.solpg.io/)
2. Create a new project with the Anchor framework
3. Set SDK version to 1.9.29 and Anchor version to 0.24.2
4. Create the project file structure to match this repository
5. Copy the source files from the `/programs` directory
6. Set the program ID to match your desired deployment address
7. Build and deploy directly from the Playground interface

## After Deployment

After deployment, you'll need to:

1. Note your program ID (address)
2. Update the program ID in Anchor.toml and lib.rs if necessary
3. Initialize the platform configuration using the provided JavaScript/TypeScript client

## Required Programs for Testing

For testing on devnet, ensure these programs are available:
- Token Metadata Program: `metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`
- Bubblegum Program: `BGUMAp9Gq7iTEuizy4pqaxsTyUCBK68MDfK752saRPUY`
- State Compression Program: `gSbePebfvPy7tRqimPoVecS2UsBvYv46ynrzWocc92s`
- Associated Token Account Program: `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`
- Token Program: `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`
- Pyth Oracle Program: `FsJ3A3u2vn5cTVofAjvy6y5kwABJAqYWpe4975bi2epH`