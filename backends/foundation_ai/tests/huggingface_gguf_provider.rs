//! Integration tests for `HuggingFaceGGUFProvider`.
//!
//! These tests verify model downloading and loading from HuggingFace Hub.
//! Tests are ignored by default as they require network access and download files.
//!
//! Run with: `cargo test --package foundation_ai --test huggingface_gguf_provider -- --ignored --nocapture`

use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFProvider,
};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelParams, ModelProvider, Quantization,
    TextContent, UserModelContent,
};
use foundation_core::valtron;
use tracing_test::traced_test;

fn init_valtron() -> valtron::PoolGuard {
    valtron::initialize_pool(42, Some(4))
}

fn get_token() -> Option<String> {
    std::env::var("HF_TOKEN").ok()
}

fn get_project_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories")
        .to_path_buf()
}

fn get_artefacts_dir(project_root: &std::path::Path) -> std::path::PathBuf {
    project_root.join("artefacts").join("models")
}

#[test]
#[ignore = "requires network access and downloads a model"]
fn test_huggingface_gguf_provider_parsing() {
    let _guard = init_valtron();

    let config = HuggingFaceGGUFConfig::default();
    let provider = HuggingFaceGGUFProvider::new(config).unwrap();

    // Test basic parsing
    let parsed = provider.parse_model_id(&ModelId::Name(
        "TheBloke/Llama-2-7B-GGUF:q4_k_m".to_string(),
        None,
    ));
    assert!(parsed.is_some());
    let parsed = parsed.unwrap();
    assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
    assert_eq!(parsed.quantization, Some("q4_k_m".to_string()));
    assert_eq!(parsed.revision, "main");

    // Test with revision
    let parsed = provider.parse_model_id(&ModelId::Name(
        "TheBloke/Llama-2-7B-GGUF:main:q5_k_m".to_string(),
        None,
    ));
    assert!(parsed.is_some());
    let parsed = parsed.unwrap();
    assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
    assert_eq!(parsed.quantization, Some("q5_k_m".to_string()));
    assert_eq!(parsed.revision, "main");

    // Test without quantization (should use default)
    let parsed =
        provider.parse_model_id(&ModelId::Name("TheBloke/Llama-2-7B-GGUF".to_string(), None));
    assert!(parsed.is_some());
    let parsed = parsed.unwrap();
    assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
    assert_eq!(parsed.revision, "main");
    // Should have default quantization
    assert!(parsed.quantization.is_some());
}

#[test]
#[traced_test]
#[ignore = "requires HF_TOKEN and downloads SmolLM2 model"]
fn test_huggingface_gguf_provider_download_smollm() {
    let _guard = init_valtron();

    let token = get_token().expect("HF_TOKEN must be set for integration tests");
    let project_root = get_project_root();
    let cache_dir = get_artefacts_dir(project_root.as_path());

    // Configure HuggingFaceGGUFProvider to download to artefacts/models
    let config = HuggingFaceGGUFConfig::builder()
        .token(token)
        .cache_dir(&cache_dir)
        .build();

    let provider = HuggingFaceGGUFProvider::new(config).unwrap();

    // Load model using ModelId with explicit quantization
    let model_id = ModelId::Name(
        "unsloth/SmolLM2-360M-Instruct-GGUF".to_string(),
        Some(Quantization::Q2K),
    );

    let result = provider.get_model(model_id);

    match result {
        Ok(_model) => {
            let expected_path = cache_dir
                .join("unsloth--SmolLM2-360M-Instruct-GGUF/SmolLM2-360M-Instruct-Q2_K.gguf");
            assert!(expected_path.exists());
        }
        Err(e) => {
            panic!("Failed to load model: {e:?}");
        }
    }
}

#[test]
#[ignore = "requires HF_TOKEN and GPU"]
fn test_huggingface_gguf_provider_with_gpu() {
    let _guard = init_valtron();

    let token = get_token().expect("HF_TOKEN must be set for integration tests");
    let project_root = get_project_root();
    let cache_dir = get_artefacts_dir(project_root.as_path());

    // Configure with GPU backend
    let config = HuggingFaceGGUFConfig::builder()
        .token(token)
        .cache_dir(&cache_dir)
        .llama_backend(foundation_ai::backends::llamacpp::LlamaBackends::LLamaGPU)
        .n_gpu_layers(32) // Offload 32 layers to GPU
        .build();

    let provider = HuggingFaceGGUFProvider::new(config).unwrap();

    // Load model using ModelId with explicit quantization
    let model_id = ModelId::Name(
        "unsloth/SmolLM2-360M-Instruct-GGUF".to_string(),
        Some(Quantization::Q2K),
    );

    let result = provider.get_model(model_id);
    assert!(result.is_ok(), "Should load model with GPU backend");
}

#[test]
#[ignore = "requires HF_TOKEN"]
fn test_huggingface_gguf_provider_describe() {
    let _guard = init_valtron();

    let config = HuggingFaceGGUFConfig::default();
    let provider = HuggingFaceGGUFProvider::new(config).unwrap();

    let descriptor = provider.describe().unwrap();

    assert_eq!(descriptor.id, "huggingface");
    assert_eq!(
        descriptor.provider,
        foundation_ai::types::ModelProviders::HUGGINGFACE
    );
    assert!(descriptor.base_url.is_some());
}

/// Test HuggingFaceGGUFProvider with SmolLM2 model download and inference.
///
/// This test downloads the SmolLM2 model (Q2_K quantization) to artefacts/models,
/// then verifies the provider can load it and perform inference.
#[test]
#[traced_test]
#[ignore = "requires HF_TOKEN and downloads a ~150MB model for inference"]
fn test_huggingface_gguf_provider_with_smollm_inference() {
    // Initialize valtron pool for blocking execution
    let _guard = init_valtron();

    let token = get_token().expect("HF_TOKEN must be set for integration tests");
    let project_root = get_project_root();

    // Use artefacts/models as the cache directory (same as TestHarness)
    let cache_dir = project_root.join("artefacts").join("models");

    // Configure HuggingFaceGGUFProvider to use artefacts/models
    let config = HuggingFaceGGUFConfig::builder()
        .token(token)
        .cache_dir(&cache_dir)
        .llama_backend(foundation_ai::backends::llamacpp::LlamaBackends::LLamaCPU)
        .n_gpu_layers(0)
        .n_threads(2usize)
        .context_length(512usize)
        .build();

    let provider = HuggingFaceGGUFProvider::new(config).unwrap();

    // Use ModelId with explicit quantization
    let model_id = ModelId::Name(
        "unsloth/SmolLM2-360M-Instruct-GGUF".to_string(),
        Some(Quantization::Q2K),
    );

    // Load model - downloads to artefacts/models if not cached
    let model = provider.get_model(model_id).expect("Failed to load model");

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
    println!("Generated: {response:?}");
}
