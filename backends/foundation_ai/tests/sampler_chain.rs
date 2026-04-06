//! Tests for sampler chain helpers.

use foundation_ai::backends::llamacpp_helpers::build_sampler_chain;
use foundation_ai::types::ModelParams;

#[test]
fn test_sampler_chain_default() {
    let params = ModelParams::default();
    let sampler = build_sampler_chain(&params);

    // Sampler should be created without panicking
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_sampler_chain_with_all_options() {
    let params = ModelParams {
        temperature: 0.8,
        top_p: 0.9,
        top_k: 50.0,
        repeat_penalty: 1.2,
        seed: Some(42),
        max_tokens: 100,
        stop_tokens: vec![],
        thinking_level: foundation_ai::types::ThinkingLevels::Medium,
        cache_retention: foundation_ai::types::CacheRetention::None,
        thinking_budget: None,
    };

    let sampler = build_sampler_chain(&params);
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_sampler_chain_greedy() {
    let params = ModelParams {
        temperature: 0.0,
        ..Default::default()
    };

    let sampler = build_sampler_chain(&params);
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_sampler_chain_high_temperature() {
    let params = ModelParams {
        temperature: 1.5,
        ..Default::default()
    };

    let sampler = build_sampler_chain(&params);
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_sampler_chain_no_top_k() {
    let params = ModelParams {
        top_k: 0.0,
        ..Default::default()
    };

    let sampler = build_sampler_chain(&params);
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_sampler_chain_no_repetition_penalty() {
    let params = ModelParams {
        repeat_penalty: 1.0,
        ..Default::default()
    };

    let sampler = build_sampler_chain(&params);
    assert!(!sampler.sampler.is_null());
}
