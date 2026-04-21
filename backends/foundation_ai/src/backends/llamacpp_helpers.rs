//! Helper functions for `llama.cpp` integration.
//!
//! This module provides utility functions for building sampler chains,
//! converting parameters, and other common operations.

use infrastructure_llama_cpp::sampling::LlamaSampler;

use crate::types::ModelParams;

/// Build a sampler chain from [`ModelParams`] configuration.
///
/// This function converts the high-level [`ModelParams`] into a concrete
/// `LlamaSampler` chain that can be used for token sampling.
///
/// # Parameters
///
/// * `params` - The model parameters containing sampling configuration
///
/// # Returns
///
/// A configured `LlamaSampler` chain ready for use in generation.
///
/// # Sampler Chain Order
///
/// The samplers are applied in the following order:
/// 1. Temperature - scales logits
/// 2. Top-K - keeps only k highest probability tokens
/// 3. Top-P (nucleus) - keeps tokens summing to probability p
/// 4. Penalties - applies repetition penalties
/// 5. Dist - samples from the distribution
#[must_use]
pub fn build_sampler_chain(params: &ModelParams) -> LlamaSampler {
    let mut samplers: Vec<LlamaSampler> = Vec::new();

    // Temperature sampling (always apply if > 0)
    if params.temperature > 0.0 {
        samplers.push(LlamaSampler::temp(params.temperature));
    }

    // Top-K sampling (convert f32 to i32)
    #[allow(clippy::cast_possible_truncation)]
    if params.top_k > 0.0 {
        let k = params.top_k.round() as i32;
        samplers.push(LlamaSampler::top_k(k));
    }

    // Top-P (nucleus) sampling
    if params.top_p > 0.0 && params.top_p < 1.0 {
        // min_keep of 1 ensures at least one token is always available
        samplers.push(LlamaSampler::top_p(params.top_p, 1));
    }

    // Repetition penalty (use epsilon for float comparison)
    if (params.repeat_penalty - 1.0).abs() > f32::EPSILON {
        // penalty_last_n of 64 means we look at last 64 tokens for repetition
        samplers.push(LlamaSampler::penalties(
            64,                    // penalty_last_n
            params.repeat_penalty, // penalty_repeat
            0.0,                   // penalty_freq (not exposed in ModelParams)
            0.0,                   // penalty_present (not exposed in ModelParams)
        ));
    }

    // Final sampling method - use dist for stochastic or greedy for deterministic
    let final_sampler = if params.temperature > 0.0 {
        // Use random sampling with seed if provided
        let seed = params.seed.unwrap_or(0xFFFF_FFFF);
        LlamaSampler::dist(seed)
    } else {
        // Greedy sampling (always picks highest probability)
        LlamaSampler::greedy()
    };
    samplers.push(final_sampler);

    // Build the chain with no_perf=false for optimized performance
    LlamaSampler::chain(samplers, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sampler_chain_default() {
        let params = ModelParams::default();
        let sampler = build_sampler_chain(&params);
        // Sampler should be created without panicking
        assert!(!sampler.sampler.is_null());
    }

    #[test]
    fn test_build_sampler_chain_with_temperature() {
        let params = ModelParams {
            temperature: 0.7,
            ..Default::default()
        };
        let sampler = build_sampler_chain(&params);
        assert!(!sampler.sampler.is_null());
    }

    #[test]
    fn test_build_sampler_chain_greedy() {
        let params = ModelParams {
            temperature: 0.0,
            ..Default::default()
        };
        let sampler = build_sampler_chain(&params);
        assert!(!sampler.sampler.is_null());
    }
}
