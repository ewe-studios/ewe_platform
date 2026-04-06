//! Integration tests for llama.cpp backend.
//!
//! These tests validate that the llama.cpp integration functions correctly
//! with real model loading and generation (when a model is available).

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{Model, ModelProvider, ModelSpec, ModelId, ModelInteraction, Messages, ModelParams, TextContent, UserModelContent};
use foundation_testing::huggingface::TestHarness;

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

/// Download the SmolLM2 test model from HuggingFace using TestHarness.
///
/// This test downloads a small GGUF model (~150MB Q2_K quantized)
/// from HuggingFace Hub for testing the llama.cpp backend.
/// The model is cached in the `.artifacts` directory.
#[tokio::test]
#[ignore = "downloads a ~150MB model from HuggingFace"]
async fn test_download_smollm_model() {
    // Get the project root (workspace directory)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR should be set");
    let project_root = std::path::Path::new(&manifest_dir)
        .parent() // go from backends/foundation_ai to backends
        .and_then(|p| p.parent()) // go from backends to project root
        .expect("Should have parent directories");

    let harness = TestHarness::new(project_root);
    let model_path = harness.get_smollm_model()
        .await
        .expect("Failed to download model");

    assert!(model_path.exists(), "Model file should exist after download");
    assert!(model_path.ends_with("SmolLM2-360M-Instruct-Q2_K.gguf"));

    println!("Model downloaded to: {}", model_path.display());
}

/// Test llama.cpp backend with the SmolLM2 model.
///
/// This test downloads the model if not present and then verifies
/// the backend can load and use it for generation.
#[tokio::test]
#[ignore = "downloads a ~150MB model and performs generation"]
async fn test_llama_with_smollm_model() {
    // Get the project root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR should be set");
    let project_root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories");

    // Download model using TestHarness
    let harness = TestHarness::new(project_root);
    let model_path = harness.get_smollm_model()
        .await
        .expect("Failed to download model");

    // Create backend
    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .n_threads(2)
        .build();

    let initialized = backend.create(Some(config), None)
        .expect("Failed to create backend");

    // Load model
    let model_spec = ModelSpec {
        name: "smollm2-360m".to_string(),
        id: ModelId::Name("smollm2".to_string(), None),
        devices: None,
        model_location: Some(model_path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = initialized.get_model_by_spec(model_spec)
        .expect("Failed to load model");

    // Test generation with chat messages
    let interaction = ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        messages: vec![Messages::User {
            role: "user".to_string(),
            content: UserModelContent::Text(TextContent {
                content: "Hello! How are you?".to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools: vec![],
        chat_template: None,
    };

    let result = model.generate(interaction, Some(ModelParams::default()));
    assert!(result.is_ok(), "Generation should succeed: {:?}", result.err());

    let response = result.unwrap();
    assert!(!response.is_empty(), "Response should not be empty");
    println!("Generated: {:?}", response);
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
