// No anchor_lang prelude needed here
use std::collections::HashMap;
use std::ops::Deref;

use crate::state::TraitType;

// Calculate rarity score based on trait values
pub fn calculate_rarity_score<'a, T>(
    trait_types: &'a [T],
    trait_values: &[(String, String)],
) -> u16 
where
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    let mut base_score: u16 = 10; // Start with a base score
    
    // Create a map for faster lookups
    let mut trait_map: HashMap<String, &T> = HashMap::new();
    for trait_type in trait_types {
        trait_map.insert(trait_type.name.clone(), trait_type);
    }
    
    for (trait_name, trait_value_name) in trait_values {
        if let Some(trait_type) = trait_map.get(trait_name) {
            // Try to find the trait value
            if let Some(value) = trait_type.trait_values.iter().find(|v| v.name == *trait_value_name) {
                // Calculate rarity contribution
                // Traits with lower weights are rarer, so invert the weight for score calculation
                let max_weight: u16 = trait_type.trait_values.iter().map(|v| v.rarity_weight).max().unwrap_or(100);
                let rarity_bonus = if value.rarity_weight > 0 {
                    // Invert the weight and scale
                    (max_weight as f32 / value.rarity_weight as f32 * 5.0) as u16
                } else {
                    // If weight is 0, assign a high rarity bonus
                    50
                };
                
                // Add to total score
                base_score = base_score.saturating_add(rarity_bonus);
                
                // Bonus for limited supply traits
                if let Some(max_supply) = value.available_supply {
                    if max_supply < 100 && value.used_supply > 0 {
                        // More bonus for more scarce traits
                        let scarcity_bonus = (100u16.saturating_sub(max_supply as u16)) / 10;
                        base_score = base_score.saturating_add(scarcity_bonus);
                    }
                }
            }
        }
    }
    
    // Ensure score is within a reasonable range
    base_score.min(1000)
}

// Calculate fusion boost based on parent NFT rarity scores
pub fn calculate_fusion_boost(parent_scores: &[u16]) -> u16 {
    if parent_scores.is_empty() {
        return 0;
    }
    
    // Get highest parent score
    let max_score = *parent_scores.iter().max().unwrap_or(&0);
    
    // Calculate average of all scores
    let avg_score = parent_scores.iter().sum::<u16>() as f32 / parent_scores.len() as f32;
    
    // Boost is a combination of max score and average
    let raw_boost = (max_score as f32 * 0.6 + avg_score * 0.4) as u16;
    
    // Cap the boost to prevent excessive inflation
    raw_boost.min(200)
}

// Calculate overall rarity score for a fused NFT
pub fn calculate_fused_nft_rarity<'a, T>(
    trait_types: &'a [T],
    trait_values: &[(String, String)],
    parent_scores: &[u16],
    fusion_level: u8,
) -> u16 
where
    T: AsRef<TraitType> + Deref<Target = TraitType>
{
    // Base score from traits
    let base_score = calculate_rarity_score(trait_types, trait_values);
    
    // Fusion boost from parents
    let fusion_boost = calculate_fusion_boost(parent_scores);
    
    // Level multiplier - higher fusion levels get more boost
    let level_multiplier = match fusion_level {
        0 => 1.0,     // Base NFT, no boost
        1 => 1.1,     // First fusion level
        2 => 1.2,     // Second fusion level
        3 => 1.35,    // Third fusion level
        _ => 1.5,     // Max boost for higher levels
    };
    
    // Apply multiplier and add fusion boost
    let final_score = (base_score as f32 * level_multiplier) as u16 + fusion_boost;
    
    // Cap at maximum score to prevent inflation
    final_score.min(2000)
}