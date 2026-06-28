//! Integration tests for llama.cpp backend.
//!
//! These tests validate that the llama.cpp integration functions correctly
//! with real model loading and generation (when a model is available).

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::toolbox::llama_server_harness::start_llama_server;
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelParams, ModelProvider, ModelSpec,
    TextContent, ToolShed, UserModelContent,
};
use foundation_core::valtron::valtron_test;
use foundation_testing::huggingface::TestHarness;
use tracing_test::traced_test;

#[valtron_test]
#[traced_test]
fn test_llama_backend_creation() {
    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(512)
        .batch_size(256)
        .n_threads(2)
        .build();

    let result = backend.create(Some(config));
    assert!(result.is_ok());
}

/// Test llama.cpp model loading using the `SmolLM2` model from `TestHarness`.
///
/// This test downloads the model if not present and then verifies
/// the backend can load it.
#[valtron_test]
#[traced_test]
fn test_llama_model_loading() {
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
        .create(Some(config))
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

/// Download the `SmolLM2` test model from `HuggingFace` using `TestHarness`.
///
/// This test downloads a small GGUF model (~150MB `Q2_K` quantized)
/// from `HuggingFace` Hub for testing the llama.cpp backend.
/// The model is cached in the `.artifacts` directory.
#[valtron_test]
#[traced_test]
fn test_download_smollm_model() {
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

/// Test llama.cpp backend with the `SmolLM2` model.
///
/// This test downloads the model if not present and then verifies
/// the backend can load and use it for generation.
#[valtron_test]
#[traced_test]
fn test_llama_with_smollm_model() {
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
        .create(Some(config))
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
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Hello! How are you?".to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    };

    let result = model.generate(interaction, Some(ModelParams::default()));
    assert!(
        result.is_ok(),
        "Generation should succeed: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert!(!response.is_empty(), "Response should not be empty");
    println!("Generated: {response:?}");
}
