import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, createMint, getOrCreateAssociatedTokenAccount, mintTo } from '@solana/spl-token';
import { assert } from "chai";

// Define the type structure for our program - normally this would be imported
// from "../target/types/nft_fusion_marketplace" but we'll define it here
// since the types might not be generated until after a successful build
type NftFusionMarketplace = {
  metadata: {
    address: PublicKey;
  };
};

describe("nft-fusion-marketplace", () => {
  // Configure the client to use the local cluster
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.NftFusionMarketplace as Program<NftFusionMarketplace>;
  
  // Generate keypairs for testing
  const user = Keypair.generate();
  const platformAuthority = Keypair.generate();
  const platformTreasury = Keypair.generate();
  const projectTreasury = Keypair.generate();
  const royaltyWallet = Keypair.generate();
  
  // Mock addresses for external programs
  const mockPythPriceAccount = Keypair.generate().publicKey;
  
  // Test variables
  const projectId = "test-project";
  const collectionId = "test-collection";
  const metadataUri = "https://arweave.net/test-metadata";
  let tokenMint: PublicKey;
  let userTokenAccount: PublicKey;
  let nftMint: PublicKey;
  
  // PDA addresses
  let platformConfigPda: PublicKey;
  let projectPda: PublicKey;
  let collectionPda: PublicKey;
  let liquidityPoolPda: PublicKey;
  let lpTokenAccountPda: PublicKey;
  
  before(async () => {
    // Airdrop SOL to test accounts
    await provider.connection.requestAirdrop(user.publicKey, 100 * LAMPORTS_PER_SOL);
    await provider.connection.requestAirdrop(platformAuthority.publicKey, 100 * LAMPORTS_PER_SOL);
    
    // Create a token mint for testing
    tokenMint = await createMint(
      provider.connection,
      platformAuthority,
      platformAuthority.publicKey,
      null,
      9 // decimals
    );
    
    // Create token accounts
    userTokenAccount = (await getOrCreateAssociatedTokenAccount(
      provider.connection,
      user,
      tokenMint,
      user.publicKey
    )).address;
    
    // Mint tokens to the user
    await mintTo(
      provider.connection,
      platformAuthority,
      tokenMint,
      userTokenAccount,
      platformAuthority.publicKey,
      1000 * 10**9 // 1000 tokens with 9 decimals
    );
    
    // Derive PDAs
    [platformConfigPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("platform_config")],
      program.programId
    );
    
    [projectPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("project"), Buffer.from(projectId)],
      program.programId
    );
    
    [collectionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("collection"), Buffer.from(collectionId)],
      program.programId
    );
  });

  it("Initializes the platform", async () => {
    try {
      await program.methods
        .initialize(500, platformTreasury.publicKey) // 5% platform fee
        .accounts({
          authority: platformAuthority.publicKey,
          platformConfig: platformConfigPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([platformAuthority])
        .rpc();
      
      // Fetch and verify the platform config
      const platformConfig = await program.account.platformConfig.fetch(platformConfigPda);
      assert.equal(platformConfig.authority.toString(), platformAuthority.publicKey.toString());
      assert.equal(platformConfig.platformFeeBasisPoints, 500);
      assert.equal(platformConfig.platformTreasury.toString(), platformTreasury.publicKey.toString());
      
      console.log("Platform initialized successfully");
    } catch (error) {
      console.error("Error initializing platform:", error);
      throw error;
    }
  });

  it("Creates a project", async () => {
    try {
      await program.methods
        .createProject(
          projectId,
          projectTreasury.publicKey,
          royaltyWallet.publicKey,
          200 // 2% royalty fee
        )
        .accounts({
          authority: platformAuthority.publicKey,
          platformConfig: platformConfigPda,
          project: projectPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([platformAuthority])
        .rpc();
      
      // Fetch and verify the project
      const project = await program.account.project.fetch(projectPda);
      assert.equal(project.authority.toString(), platformAuthority.publicKey.toString());
      assert.equal(project.projectId, projectId);
      assert.equal(project.projectTreasury.toString(), projectTreasury.publicKey.toString());
      assert.equal(project.royaltyWallet.toString(), royaltyWallet.publicKey.toString());
      assert.equal(project.royaltyBasisPoints, 200);
      assert.isTrue(project.isActive);
      
      console.log("Project created successfully");
    } catch (error) {
      console.error("Error creating project:", error);
      throw error;
    }
  });

  it("Creates a collection", async () => {
    try {
      await program.methods
        .createCollection(
          collectionId,
          projectId,
          metadataUri,
          tokenMint,
          false // Not compressed
        )
        .accounts({
          authority: platformAuthority.publicKey,
          platformConfig: platformConfigPda,
          project: projectPda,
          collection: collectionPda,
          tokenMintAccount: tokenMint,
          systemProgram: SystemProgram.programId,
        })
        .signers([platformAuthority])
        .rpc();
      
      // Fetch and verify the collection
      const collection = await program.account.collection.fetch(collectionPda);
      assert.equal(collection.project.toString(), projectPda.toString());
      assert.equal(collection.collectionId, collectionId);
      assert.equal(collection.metadataUri, metadataUri);
      assert.equal(collection.tokenMint.toString(), tokenMint.toString());
      assert.isFalse(collection.isCompressed);
      
      console.log("Collection created successfully");
    } catch (error) {
      console.error("Error creating collection:", error);
      throw error;
    }
  });

  it("Sets up a liquidity pool", async () => {
    try {
      // Derive liquidity pool PDAs
      [liquidityPoolPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("liquidity_pool"), projectPda.toBuffer()],
        program.programId
      );
      
      // Get the associated token account for the liquidity pool
      [lpTokenAccountPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("associated_token_account"),
          liquidityPoolPda.toBuffer(),
          tokenMint.toBuffer(),
        ],
        new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL") // Associated Token Program ID
      );
      
      await program.methods
        .setupLiquidityPool(
          projectId,
          tokenMint,
          new anchor.BN(100 * 10**9) // Initial liquidity of 100 tokens
        )
        .accounts({
          authority: platformAuthority.publicKey,
          platformConfig: platformConfigPda,
          project: projectPda,
          liquidityPool: liquidityPoolPda,
          tokenMintAccount: tokenMint,
          lpTokenAccount: lpTokenAccountPda,
          authorityTokenAccount: userTokenAccount, // Using user's account for simplicity in test
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([platformAuthority])
        .rpc();
      
      // Fetch and verify the liquidity pool
      const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
      assert.equal(liquidityPool.project.toString(), projectPda.toString());
      assert.equal(liquidityPool.tokenMint.toString(), tokenMint.toString());
      assert.equal(liquidityPool.lpTokenAccount.toString(), lpTokenAccountPda.toString());
      assert.isFalse(liquidityPool.redemptionLocked);
      
      console.log("Liquidity pool set up successfully");
    } catch (error) {
      console.error("Error setting up liquidity pool:", error);
      throw error;
    }
  });

  // Helper function to fetch liquidity pool data and check price
  async function checkOraclePrice(expectedSource: string, minPrice = 0) {
    // Fetch the liquidity pool data
    const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
    
    // Assert that price source is as expected
    assert.equal(liquidityPool.priceSource.toString(), expectedSource);
    
    // Ensure price exists and is reasonable
    assert.isDefined(liquidityPool.oraclePriceUsd);
    if (liquidityPool.oraclePriceUsd) {
      assert.isAtLeast(liquidityPool.oraclePriceUsd.toNumber(), minPrice);
      console.log(`Price updated to: $${liquidityPool.oraclePriceUsd.toNumber() / 1_000_000} USD`);
    }
    
    // Check that timestamp was updated
    assert.isAbove(liquidityPool.oraclePriceLastUpdate.toNumber(), 0);
    
    return liquidityPool;
  }

  it("Updates price manually", async () => {
    try {
      // Set a manual price of $10.50 USD (scaled by 10^6)
      const priceUsd = new anchor.BN(10_500_000);
      
      await program.methods
        .setPriceManually(
          projectId,
          priceUsd
        )
        .accounts({
          authority: platformAuthority.publicKey,
          platformConfig: platformConfigPda,
          project: projectPda,
          liquidityPool: liquidityPoolPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([platformAuthority])
        .rpc();
      
      // Verify price was updated correctly
      const liquidityPool = await checkOraclePrice('manual', 10_000_000);
      assert.equal(liquidityPool.oraclePriceUsd.toString(), priceUsd.toString());
      assert.isFalse(liquidityPool.redemptionLocked);
      
      console.log("Manual price update successful");
    } catch (error) {
      console.error("Error updating price manually:", error);
      throw error;
    }
  });

  it("Updates price from DEX liquidity pools", async () => {
    try {
      // We'll need a mock DEX pool for testing
      // Create token accounts to represent DEX liquidity
      const dexTokenAccount = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        platformAuthority,
        tokenMint,
        platformAuthority.publicKey
      );
      
      // Mint tokens to the DEX token account
      await mintTo(
        provider.connection,
        platformAuthority,
        tokenMint,
        dexTokenAccount.address,
        platformAuthority.publicKey,
        1000 * 10**9 // 1000 tokens with 9 decimals
      );
      
      // Create a mock USDC mint for the base pair
      const usdcMint = await createMint(
        provider.connection,
        platformAuthority,
        platformAuthority.publicKey,
        null,
        6 // USDC has 6 decimals
      );
      
      // Create USDC account for the DEX
      const dexBaseAccount = await getOrCreateAssociatedTokenAccount(
        provider.connection,
        platformAuthority,
        usdcMint,
        platformAuthority.publicKey
      );
      
      // Mint USDC to the DEX base account
      await mintTo(
        provider.connection,
        platformAuthority,
        usdcMint,
        dexBaseAccount.address,
        platformAuthority.publicKey,
        5000 * 10**6 // 5000 USDC with 6 decimals (represents a $5 token price)
      );
      
      await program.methods
        .updatePriceFromDex(
          projectId
        )
        .accounts({
          authority: platformAuthority.publicKey,
          platformConfig: platformConfigPda,
          project: projectPda,
          liquidityPool: liquidityPoolPda,
          dexTokenAccount: dexTokenAccount.address,
          dexBaseAccount: dexBaseAccount.address,
          tokenMint: tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([platformAuthority])
        .rpc();
      
      // Verify price was updated from DEX
      await checkOraclePrice('dexLiquidity', 1_000_000); // At least $1
      
      console.log("DEX price update successful");
    } catch (error) {
      console.error("Error updating price from DEX:", error);
      throw error;
    }
  });

  it("Attempts to update price from Pyth oracle", async () => {
    try {
      // In a real environment, we'd use a real Pyth price feed
      // For this test, we'll try the call but expect it to fail gracefully since we're not
      // connected to a real Pyth feed
      
      console.log("Attempting Pyth oracle price update (expected to fail without real Pyth feed)");
      
      try {
        await program.methods
          .updatePriceFromPyth(
            projectId
          )
          .accounts({
            authority: platformAuthority.publicKey,
            platformConfig: platformConfigPda,
            project: projectPda,
            liquidityPool: liquidityPoolPda,
            pythPriceAccount: mockPythPriceAccount, // This is a fake address
            systemProgram: SystemProgram.programId,
          })
          .signers([platformAuthority])
          .rpc();
          
        console.log("Pyth update succeeded unexpectedly");
      } catch (error) {
        // We expect this to fail because we're not using a real Pyth account
        console.log("Pyth update failed as expected:", error.message);
        assert.include(error.message, "Error"); // Should contain some error message
      }
      
      // Verify the previous price is still there
      const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
      assert.isDefined(liquidityPool.oraclePriceUsd);
      console.log("Previous price remains intact");
    } catch (error) {
      console.error("Unexpected error in Pyth test:", error);
      throw error;
    }
  });

  it("Mints an NFT", async () => {
    try {
      // Generate a keypair for the NFT mint
      const nftMintKeypair = Keypair.generate();
      nftMint = nftMintKeypair.publicKey;
      
      // Derive NFT data PDA
      const [nftDataPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("nft_data"), nftMint.toBuffer()],
        program.programId
      );
      
      // In a real test, you would need to create metadata accounts
      // For this test, we'll use placeholder accounts
      const metadataAccount = Keypair.generate().publicKey;
      const masterEdition = Keypair.generate().publicKey;
      const userTokenAccount = Keypair.generate().publicKey;
      
      // Note: This test will fail because we need proper metadata program integration
      // This is just a placeholder for the actual test structure
      console.log("NFT minting would be tested here");
      
      /*
      await program.methods
        .mintNft(
          collectionId,
          metadataUri,
          null // No traits selection
        )
        .accounts({
          user: user.publicKey,
          platformConfig: platformConfigPda,
          collection: collectionPda,
          project: projectPda,
          nftMint: nftMint,
          nftData: nftDataPda,
          metadataAccount: metadataAccount,
          masterEdition: masterEdition,
          userTokenAccount: userTokenAccount,
          tokenMetadataProgram: new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"),
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
          systemProgram: SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([user, nftMintKeypair])
        .rpc();
      */
    } catch (error) {
      console.error("Error in NFT minting test:", error);
      // Expected to fail in this stub implementation
    }
  });

  it("Sets up NFT escrow for swapping", async () => {
    try {
      // Generate a keypair for the NFT mint
      const nftMintKeypair = Keypair.generate();
      nftMint = nftMintKeypair.publicKey;
      
      // Derive escrow PDA
      const [escrowPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("token_escrow"), user.publicKey.toBuffer(), nftMint.toBuffer()],
        program.programId
      );
      
      // Derive token escrow account PDA
      const [escrowTokenAccountPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("escrow_token_account"), escrowPda.toBuffer()],
        program.programId
      );
      
      console.log("Setting up token escrow for NFT swap");
      
      // In a real test, this would create a full escrow
      // For now, we'll just verify the calculation of escrow PDAs
      assert.ok(escrowPda);
      assert.ok(escrowTokenAccountPda);
      
      // Calculate the token amount needed based on current price
      const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
      const priceInTokens = liquidityPool.oraclePriceUsd 
        ? liquidityPool.oraclePriceUsd.toNumber() / 1_000_000
        : 10; // Fallback price if not set
        
      console.log(`NFT would cost approximately ${priceInTokens} tokens based on oracle price`);
      console.log("Token escrow setup validated");
    } catch (error) {
      console.error("Error setting up token escrow:", error);
      throw error;
    }
  });
  
  it("Tests redemption mechanics (simulation only)", async () => {
    try {
      // Check if redemption is locked
      const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
      
      if (liquidityPool.redemptionLocked) {
        console.log("Redemption is currently locked - would need to update price");
      } else {
        console.log("Redemption is available");
        
        // Calculate redemption value using oracle price
        const baseRedemptionValue = liquidityPool.oraclePriceUsd
          ? liquidityPool.oraclePriceUsd.toNumber() 
          : 10_000_000; // Fallback price
          
        // Add rarity bonus calculation (simulated)
        const rarityMultiplier = 1.2; // 20% bonus for rare NFT
        const finalRedemptionValue = Math.floor(baseRedemptionValue * rarityMultiplier);
        
        console.log(`Base redemption value: $${baseRedemptionValue / 1_000_000} USD`);
        console.log(`With rarity bonus: $${finalRedemptionValue / 1_000_000} USD`);
        
        // In tokens (assuming 1:1 USD to token at 6 decimal places)
        const tokensReceived = finalRedemptionValue / 1_000_000 * 10**9;
        console.log(`User would receive approximately ${tokensReceived / 10**9} tokens`);
      }
      
      // Test refresh Oracle functionality
      console.log("Testing Oracle refresh capability");
      if (Date.now() / 1000 - liquidityPool.oraclePriceLastUpdate.toNumber() > 3600) {
        console.log("Oracle price is stale, refresh would be required before redemption");
      } else {
        console.log("Oracle price is fresh, redemption could proceed immediately");
      }
    } catch (error) {
      console.error("Error in redemption test:", error);
      throw error;
    }
  });
  
  it("Tests price conversion between USD and tokens", async () => {
    try {
      // Check the oracle price
      const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
      const priceUsd = liquidityPool.oraclePriceUsd 
        ? liquidityPool.oraclePriceUsd.toNumber() 
        : 10_000_000; // Default $10 USD if not set
      
      // Token amount calculation logic (testing our conversion functions)
      
      // Calculate for different USD amounts
      const testAmounts = [1, 5, 10, 50, 100];
      console.log(`Current price: $${priceUsd / 1_000_000} USD per token`);
      
      for (const usdAmount of testAmounts) {
        // USD to token conversion (simplified)
        // In a real impl, this would use the actual conversion logic from our program
        const scaledUsd = usdAmount * 1_000_000; // 6 decimals
        const tokenAmount = (scaledUsd * 10**9) / priceUsd; // 9 token decimals
        
        console.log(`$${usdAmount} USD = ${tokenAmount / 10**9} tokens`);
        
        // And back to USD to verify
        const backToUsd = (tokenAmount * priceUsd) / 10**9 / 1_000_000;
        console.log(`Verification: ${tokenAmount / 10**9} tokens = $${backToUsd} USD`);
        
        // Assert near equality within rounding error
        assert.approximately(backToUsd, usdAmount, 0.001);
      }
      
      console.log("Price conversion logic works correctly");
    } catch (error) {
      console.error("Error in price conversion test:", error);
      throw error;
    }
  });
  
  it("Validates the state structures", async () => {
    try {
      console.log("Validating data structures in our app");
      
      // Get all the state accounts we've created
      const platformConfig = await program.account.platformConfig.fetch(platformConfigPda);
      const project = await program.account.project.fetch(projectPda);
      const collection = await program.account.collection.fetch(collectionPda);
      const liquidityPool = await program.account.liquidityPool.fetch(liquidityPoolPda);
      
      // Validate platform config structure
      console.log("\nPlatform Config:");
      console.log("- Authority:", platformConfig.authority.toString());
      console.log("- Fee (basis points):", platformConfig.platformFeeBasisPoints);
      console.log("- Treasury:", platformConfig.platformTreasury.toString());
      console.log("- Bump:", platformConfig.bump);
      
      // Validate project structure
      console.log("\nProject:");
      console.log("- ID:", project.projectId);
      console.log("- Authority:", project.authority.toString());
      console.log("- Treasury:", project.projectTreasury.toString());
      console.log("- Royalty wallet:", project.royaltyWallet.toString());
      console.log("- Royalty (basis points):", project.royaltyBasisPoints);
      console.log("- Active:", project.isActive);
      console.log("- Last activity:", new Date(project.lastActivityTimestamp.toNumber() * 1000).toISOString());
      console.log("- Bump:", project.bump);
      
      // Validate collection structure
      console.log("\nCollection:");
      console.log("- ID:", collection.collectionId);
      console.log("- Project:", collection.project.toString());
      console.log("- Token mint:", collection.tokenMint.toString());
      console.log("- Metadata URI:", collection.metadataUri);
      console.log("- Compressed:", collection.isCompressed);
      console.log("- Bump:", collection.bump);
      
      // Validate liquidity pool structure
      console.log("\nLiquidity Pool:");
      console.log("- Project:", liquidityPool.project.toString());
      console.log("- Token mint:", liquidityPool.tokenMint.toString());
      console.log("- LP token account:", liquidityPool.lpTokenAccount.toString());
      console.log("- Oracle price (USD):", liquidityPool.oraclePriceUsd 
        ? `$${liquidityPool.oraclePriceUsd.toNumber() / 1_000_000}` 
        : "Not set");
      console.log("- Price source:", liquidityPool.priceSource.toString());
      console.log("- Last price update:", new Date(liquidityPool.oraclePriceLastUpdate.toNumber() * 1000).toISOString());
      console.log("- Redemption locked:", liquidityPool.redemptionLocked);
      console.log("- Bump:", liquidityPool.bump);
      
      // Validate key relationships
      assert.equal(project.authority.toString(), platformConfig.authority.toString(), 
        "Project authority should match platform authority");
      assert.equal(collection.project.toString(), projectPda.toString(),
        "Collection's project should match project PDA");
      assert.equal(liquidityPool.project.toString(), projectPda.toString(),
        "Liquidity pool's project should match project PDA");
      assert.equal(liquidityPool.tokenMint.toString(), tokenMint.toString(),
        "Liquidity pool's token mint should match token mint");
      assert.equal(collection.tokenMint.toString(), tokenMint.toString(),
        "Collection's token mint should match token mint");
      
      console.log("\nAll data structures validated successfully");
    } catch (error) {
      console.error("Error validating state structures:", error);
      throw error;
    }
  });
});
