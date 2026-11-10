use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::backends::llamacpp_helpers::*;
use foundation_ai::types::base_types::{ModelParams, ModelProvider};

/// The explicit real clock backend for tests.
fn test_clock() -> foundation_context::Ctx {
    foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    ))
}

#[test]
fn test_build_sampler_chain_default() {
    let params = ModelParams::default();
    let sampler = build_sampler_chain(&params, 32_000);
    // Sampler should be created without panicking
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_build_sampler_chain_with_temperature() {
    let params = ModelParams {
        temperature: 0.7,
        ..Default::default()
    };
    let sampler = build_sampler_chain(&params, 32_000);
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_build_sampler_chain_greedy() {
    let params = ModelParams {
        temperature: 0.0,
        ..Default::default()
    };
    let sampler = build_sampler_chain(&params, 32_000);
    assert!(!sampler.sampler.is_null());
}

#[test]
fn test_backend_config_builder() {
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(32)
        .context_length(4096)
        .batch_size(512)
        .n_threads(8)
        .use_mmap(true)
        .use_mlock(false)
        .build();

    assert_eq!(config.n_gpu_layers, 32);
    assert_eq!(config.context_length, 4096);
    assert_eq!(config.batch_size, 512);
    assert_eq!(config.n_threads, 8);
    assert!(config.use_mmap);
    assert!(!config.use_mlock);
}

#[test]
fn test_backend_config_default() {
    let config = LlamaBackendConfig::default();

    assert_eq!(config.n_gpu_layers, 0);
    assert_eq!(config.context_length, 4096);
    assert_eq!(config.batch_size, 512);
    assert!(config.n_threads > 0);
}

#[test]
fn test_backend_describe() {
    let backend = LlamaBackends::cpu(test_clock());
    let descriptor = backend.describe().unwrap();

    assert_eq!(descriptor.id, "llamacpp");
    assert!(descriptor.name.contains("llama.cpp"));
    assert_eq!(descriptor.context_window, 4096);
}
