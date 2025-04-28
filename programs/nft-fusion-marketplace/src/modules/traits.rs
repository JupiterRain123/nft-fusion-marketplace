use anchor_lang::prelude::*;
use solana_program::hash::hash;
use std::ops::{Deref, DerefMut};

use crate::errors::MarketplaceError;
use crate::state::{
    CollectionTraitConfig, MetadataFormat, TraitType, TraitValue
};

// Helper function to create a new trait type
pub fn create_trait_type(
    collection: &Pubkey,
    name: String,
    is_required: bool,
    trait_values: Vec<TraitValue>,
    bump: u8,
) -> Result<TraitType> {
    if trait_values.is_empty() {
        return Err(MarketplaceError::InvalidTraitConfig.into());
    }

    Ok(TraitType {
        collection: *collection,
        name,
        is_required,
        trait_values,
        bump,
    })
}

// Helper function to find a trait value within a trait type
pub fn find_trait_value<'a, T>(
    trait_type: &'a T, 
    value_name: &str
) -> Result<&'a TraitValue> 
where 
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    trait_type.trait_values
        .iter()
        .find(|v| v.name == value_name)
        .ok_or(MarketplaceError::TraitValueNotFound.into())
}

// Helper function to generate a pseudorandom seed from recent blockhash and other inputs
pub fn generate_random_seed(
    recent_slot: u64,
    collection_key: &Pubkey,
    user_key: &Pubkey,
    additional_entropy: &[u8],
) -> [u8; 32] {
    // Combine all inputs to create entropy
    let mut entropy = Vec::new();
    entropy.extend_from_slice(&recent_slot.to_le_bytes());
    entropy.extend_from_slice(collection_key.as_ref());
    entropy.extend_from_slice(user_key.as_ref());
    entropy.extend_from_slice(additional_entropy);
    
    // Hash the combined entropy
    let hash_result = hash(&entropy);
    hash_result.to_bytes()
}

// Helper function to select a trait value based on weights
pub fn select_weighted_trait_value<'a, T>(
    trait_type: &'a T,
    seed: &[u8; 32],
    offset: usize,
) -> Result<&'a TraitValue> 
where 
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    // Ensure trait type has values
    if trait_type.trait_values.is_empty() {
        return Err(MarketplaceError::InvalidTraitConfig.into());
    }
    
    // Calculate total weight
    let total_weight: u32 = trait_type.trait_values
        .iter()
        .map(|v| v.rarity_weight as u32)
        .sum();
    
    if total_weight == 0 {
        return Err(MarketplaceError::InvalidTraitConfig.into());
    }
    
    // Extract 4 bytes from seed at the given offset (wrapped around if needed)
    let mut rand_bytes = [0u8; 4];
    for i in 0..4 {
        rand_bytes[i] = seed[(offset + i) % 32];
    }
    
    // Convert to a u32 and get a value between 0 and total_weight
    let rand_u32 = u32::from_le_bytes(rand_bytes);
    let rand_value = rand_u32 % total_weight;
    
    // Select trait based on weights
    let mut cumulative_weight = 0;
    for trait_value in &trait_type.trait_values {
        // Skip traits that have reached their supply limit
        if let Some(max_supply) = trait_value.available_supply {
            if trait_value.used_supply >= max_supply {
                continue;
            }
        }
        
        cumulative_weight += trait_value.rarity_weight as u32;
        if rand_value < cumulative_weight {
            return Ok(trait_value);
        }
    }
    
    // Fallback to first trait if no weighted selection was made
    // (should only happen if most traits are supply-limited)
    trait_type.trait_values
        .iter()
        .find(|v| {
            if let Some(max_supply) = v.available_supply {
                v.used_supply < max_supply
            } else {
                true
            }
        })
        .ok_or(MarketplaceError::TraitSupplyExceeded.into())
}

// Helper function to auto-generate traits for an NFT
pub fn auto_generate_traits<'a, T>(
    trait_types: &'a [T],
    _config: &CollectionTraitConfig,
    seed: &[u8; 32],
) -> Result<Vec<(String, String)>> 
where 
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    let mut selected_traits = Vec::new();
    
    // Iterate through each trait type
    for (i, trait_type) in trait_types.iter().enumerate() {
        // Use a different offset for each trait type to ensure variety
        let trait_value = select_weighted_trait_value(trait_type, seed, i * 4)?;
        
        // Add the selected trait to our list
        selected_traits.push((trait_type.name.clone(), trait_value.name.clone()));
    }
    
    // Verify all required traits are present
    for trait_type in trait_types {
        if trait_type.is_required {
            let has_trait = selected_traits.iter().any(|(t_name, _)| t_name == &trait_type.name);
            
            if !has_trait {
                return Err(MarketplaceError::RequiredTraitMissing.into());
            }
        }
    }
    
    Ok(selected_traits)
}

// Helper function to validate manually provided traits
pub fn validate_traits<'a, T>(
    trait_types: &'a [T],
    provided_traits: &[(String, String)],
) -> Result<()> 
where
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    // Check all required traits are present
    for trait_type in trait_types {
        if trait_type.is_required {
            let has_trait = provided_traits.iter().any(|(t_name, _)| t_name == &trait_type.name);
            
            if !has_trait {
                return Err(MarketplaceError::RequiredTraitMissing.into());
            }
        }
    }
    
    // Validate each provided trait
    for (trait_name, trait_value) in provided_traits {
        // Find the corresponding trait type
        let trait_type = trait_types
            .iter()
            .find(|t| &t.name == trait_name)
            .ok_or(MarketplaceError::TraitTypeNotFound)?;
        
        // Find the trait value in the trait type
        let value = find_trait_value(trait_type, trait_value)?;
        
        // Check if trait is within supply limits
        if let Some(max_supply) = value.available_supply {
            if value.used_supply >= max_supply {
                return Err(MarketplaceError::TraitSupplyExceeded.into());
            }
        }
    }
    
    Ok(())
}

// Helper function to generate metadata URI with traits
pub fn generate_metadata_uri<'a, T>(
    config: &CollectionTraitConfig,
    trait_values: &[(String, String)],
    trait_types: &'a [T],
) -> Result<String> 
where
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    // Base URI from config
    let mut uri = config.base_uri.clone();
    
    // Process based on metadata format
    match config.metadata_format {
        MetadataFormat::StandardJson => {
            // Just append postfixes for each trait to the base URI
            for (trait_name, trait_value_name) in trait_values {
                // Find the trait type
                let trait_type = trait_types
                    .iter()
                    .find(|t| &t.name == trait_name)
                    .ok_or(MarketplaceError::TraitTypeNotFound)?;
                
                // Find the trait value
                let value = find_trait_value(trait_type, trait_value_name)?;
                
                // Append the postfix
                if !value.uri_postfix.is_empty() {
                    if !uri.ends_with('/') {
                        uri.push('/');
                    }
                    uri.push_str(&value.uri_postfix);
                }
            }
        },
        MetadataFormat::CompressedJson => {
            // For compressed format, we'll create a compact identifier
            // representing the traits (implementation depends on specific needs)
            let mut trait_identifiers = Vec::new();
            
            for (trait_name, trait_value_name) in trait_values {
                let trait_type = trait_types
                    .iter()
                    .find(|t| &t.name == trait_name)
                    .ok_or(MarketplaceError::TraitTypeNotFound)?;
                
                let value = find_trait_value(trait_type, trait_value_name)?;
                
                // Add compact identifier
                trait_identifiers.push(format!("{}:{}", trait_type.name, value.name));
            }
            
            // Join all identifiers and append to base URI
            if !uri.ends_with('/') {
                uri.push('/');
            }
            uri.push_str(&trait_identifiers.join("_"));
        },
        MetadataFormat::Custom => {
            // Custom handling would be implemented based on project needs
            // For now, just keep the base URI
        }
    }
    
    Ok(uri)
}

// Helper function to update used supply for a trait value
pub fn update_trait_supply<T>(
    trait_type: &mut T,
    value_name: &str,
) -> Result<()> 
where
    T: AsMut<TraitType> + DerefMut<Target = TraitType>
{
    // Find the trait value and increment its used_supply
    let value = trait_type.trait_values
        .iter_mut()
        .find(|v| v.name == value_name)
        .ok_or(MarketplaceError::TraitValueNotFound)?;
    
    value.used_supply += 1;
    
    // Check if we've exceeded available supply
    if let Some(max_supply) = value.available_supply {
        if value.used_supply > max_supply {
            return Err(MarketplaceError::TraitSupplyExceeded.into());
        }
    }
    
    Ok(())
}