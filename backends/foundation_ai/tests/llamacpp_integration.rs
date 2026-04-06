//! Integration tests for llama.cpp backend.
//!
//! These tests validate that the llama.cpp integration functions correctly
//! with real model loading and generation (when a model is available).

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelParams, ModelProvider, ModelSpec, TextContent,
    UserModelContent,
};
use foundation_core::valtron;
use foundation_testing::huggingface::TestHarness;
use tracing_test::traced_test;

#[test]
#[traced_test]
#[ignore = "requires a local GGUF model file"]
fn test_llama_backend_creation() {
    // Initialize valtron pool for blocking execution
    let _guard = valtron::initialize_pool(42, Some(4));
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

/// Test llama.cpp model loading using the SmolLM2 model from TestHarness.
///
/// This test downloads the model if not present and then verifies
/// the backend can load it.
#[test]
#[traced_test]
#[ignore = "downloads a ~150MB model from HuggingFace"]
fn test_llama_model_loading() {
    // Initialize valtron pool for blocking execution
    let _guard = valtron::initialize_pool(42, Some(4));
    // Get the project root
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let project_root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories");

    // Download model using TestHarness
    let harness = TestHarness::new(project_root);
    let model_path = harness
        .get_smollm_model()
        .expect("Failed to download model");

    // Create backend
    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .build();

    let initialized = backend
        .create(Some(config), None)
        .expect("Failed to create backend");

    // Load model
    let model_spec = ModelSpec {
        name: "smollm2-360m".to_string(),
        id: ModelId::Name("smollm2".to_string(), None),
        devices: None,
        model_location: Some(model_path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let result = initialized.get_model_by_spec(model_spec);
    assert!(result.is_ok(), "Failed to load model: {:?}", result.err());
    println!("Model loaded successfully from: {}", model_path.display());
}

/// Download the SmolLM2 test model from HuggingFace using TestHarness.
///
/// This test downloads a small GGUF model (~150MB Q2_K quantized)
/// from HuggingFace Hub for testing the llama.cpp backend.
/// The model is cached in the `.artifacts` directory.
#[test]
#[traced_test]
#[ignore = "downloads a ~150MB model from HuggingFace"]
fn test_download_smollm_model() {
    // Initialize valtron pool for blocking execution
    let _guard = valtron::initialize_pool(42, Some(4));
    // Get the project root (workspace directory)
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let project_root = std::path::Path::new(&manifest_dir)
        .parent() // go from backends/foundation_ai to backends
        .and_then(|p| p.parent()) // go from backends to project root
        .expect("Should have parent directories");

    let harness = TestHarness::new(project_root);
    let model_path = harness
        .get_smollm_model()
        .expect("Failed to download model");

    assert!(
        model_path.exists(),
        "Model file should exist after download"
    );
    assert!(model_path.ends_with("SmolLM2-360M-Instruct-Q2_K.gguf"));

    println!("Model downloaded to: {}", model_path.display());
}

/// Test llama.cpp backend with the SmolLM2 model.
///
/// This test downloads the model if not present and then verifies
/// the backend can load and use it for generation.
#[test]
#[traced_test]
#[ignore = "downloads a ~150MB model and performs generation"]
fn test_llama_with_smollm_model() {
    // Initialize valtron pool for blocking execution
    let _guard = valtron::initialize_pool(42, Some(4));
    // Get the project root
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let project_root = std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories");

    // Download model using TestHarness
    let harness = TestHarness::new(project_root);
    let model_path = harness
        .get_smollm_model()
        .expect("Failed to download model");

    // Create backend
    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .n_threads(2)
        .build();

    let initialized = backend
        .create(Some(config), None)
        .expect("Failed to create backend");

    // Load model
    let model_spec = ModelSpec {
        name: "smollm2-360m".to_string(),
        id: ModelId::Name("smollm2".to_string(), None),
        devices: None,
        model_location: Some(model_path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = initialized
        .get_model_by_spec(model_spec)
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
    assert!(
        result.is_ok(),
        "Generation should succeed: {:?}",
        result.err()
    );

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
