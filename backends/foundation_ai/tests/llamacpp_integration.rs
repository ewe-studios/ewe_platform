//! Integration tests for llama.cpp backend.
//!
//! These tests validate that the llama.cpp integration functions correctly
//! with real model loading and generation (when a model is available).

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{ModelProvider, ModelSpec, ModelId};

#[test]
#[ignore = "requires a local GGUF model file"]
fn test_llama_backend_creation() {
    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .batch_size(256)
        .n_threads(2)
        .build();

    let result = backend.create(Some(config), None);
    assert!(result.is_ok());
}

#[test]
#[ignore = "requires a local GGUF model file"]
fn test_llama_model_loading() {
    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .build();

    let initialized = backend.create(Some(config), None).unwrap();

    // Point to a test model - this path should be set via env var or test fixture
    let model_path = std::env::var("TEST_GGUF_MODEL")
        .unwrap_or_else(|_| "/tmp/test-model.gguf".to_string());

    let model_spec = ModelSpec {
        name: "test-model".to_string(),
        id: ModelId::Name("test".to_string(), None),
        devices: None,
        model_location: Some(model_path.clone().into()),
        lora_location: None,
    };

    let result = initialized.get_model_by_spec(model_spec);

    // Skip if model file doesn't exist
    if !std::path::Path::new(&model_path).exists() {
        println!("Skipping test - model file not found at {}", model_path);
        return;
    }

    assert!(result.is_ok(), "Failed to load model: {:?}", result.err());
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
    let backend = LlamaBackends::LLamaCPU;
    let descriptor = backend.describe().unwrap();

    assert_eq!(descriptor.id, "llamacpp");
    assert!(descriptor.name.contains("llama.cpp"));
    assert_eq!(descriptor.context_window, 4096);
}
