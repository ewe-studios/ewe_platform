#![cfg(all(feature = "llamacpp", not(target_family = "wasm")))]

use foundation_ai::backends::llamacpp_helpers::*;
use foundation_ai::types::base_types::ModelParams;

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
