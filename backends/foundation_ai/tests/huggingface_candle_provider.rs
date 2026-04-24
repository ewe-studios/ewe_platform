//! Integration tests for `HuggingFaceCandleProvider`.
//!
//! These tests verify safetensors model downloading from HuggingFace Hub
//! and Candle inference. Tests are ignored by default as they require network
//! access and download model files.
//!
//! Run with: `cargo test --package foundation_ai --test huggingface_candle_provider --features candle -- --ignored --nocapture`

#![cfg(feature = "candle")]

use foundation_ai::backends::candle::CandleDType;
use foundation_ai::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelProvider, TextContent,
    UserModelContent,
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

/// Test HuggingFaceCandleProvider repo_id parsing.
///
/// Does not require network or model files.
#[test]
fn test_candle_provider_repo_parsing() {
    // parse_repo_id is a static method — no provider needed
    let parsed = HuggingFaceCandleProvider::parse_repo_id(&ModelId::Name(
        "HuggingFaceTB/SmolLM2-135M".to_string(),
        None,
    ));
    assert_eq!(parsed, Some("HuggingFaceTB/SmolLM2-135M".to_string()));

    // With quantization suffix after colon
    let parsed = HuggingFaceCandleProvider::parse_repo_id(&ModelId::Name(
        "HuggingFaceTB/SmolLM2-135M:f32".to_string(),
        None,
    ));
    assert_eq!(parsed, Some("HuggingFaceTB/SmolLM2-135M".to_string()));

    // Invalid — no slash
    let parsed =
        HuggingFaceCandleProvider::parse_repo_id(&ModelId::Name("noslash".to_string(), None));
    assert!(parsed.is_none());
}

/// Test HuggingFaceCandleProvider describe.
///
/// Does not require network or model files.
#[test]
fn test_candle_provider_describe() {
    let cache_dir = std::env::temp_dir().join("candle_describe_test");
    let config = HuggingFaceCandleConfig::builder()
        .cache_dir(&cache_dir)
        .build();

    let provider = HuggingFaceCandleProvider::new(config).unwrap();
    let descriptor = provider.describe().unwrap();

    assert_eq!(descriptor.id, "huggingface-candle");
    assert_eq!(descriptor.name, "HuggingFace Hub (Candle)");
    assert_eq!(
        descriptor.provider,
        foundation_ai::types::ModelProviders::HUGGINGFACE
    );
    assert_eq!(
        descriptor.base_url,
        Some("https://huggingface.co".to_string())
    );
    assert_eq!(descriptor.context_window, 4096);
    assert_eq!(descriptor.max_tokens, 2048);
    assert!(!descriptor.reasoning);
}

/// Test downloading SmolLM2 safetensors model from HuggingFace Hub.
///
/// Uses `HuggingFaceTB/SmolLM2-135M` — a small model (~270MB safetensors).
/// Downloaded to `artefacts/models/`.
#[test]
#[traced_test]
#[ignore = "requires HF_TOKEN and downloads ~270MB safetensors model"]
fn test_candle_provider_download_smollm_safetensors() {
    let _guard = init_valtron();

    let _token = get_token().expect("HF_TOKEN must be set for integration tests");
    let project_root = get_project_root();
    let cache_dir = get_artefacts_dir(project_root.as_path());

    let config = HuggingFaceCandleConfig::builder()
        .hf_token(_token)
        .cache_dir(&cache_dir)
        .context_length(512)
        .dtype(CandleDType::F32)
        .build();

    let provider = HuggingFaceCandleProvider::new(config).unwrap();

    let model_id = ModelId::Name("HuggingFaceTB/SmolLM2-135M".to_string(), None);

    let result = provider.get_model(model_id);

    match result {
        Ok(_model) => {
            // Verify model was cached
            let expected_dir = cache_dir.join("HuggingFaceTB--SmolLM2-135M");
            assert!(expected_dir.exists(), "Model directory should exist");
            assert!(
                expected_dir.join("config.json").exists(),
                "config.json should exist"
            );
            assert!(
                expected_dir.join("tokenizer.json").exists(),
                "tokenizer.json should exist"
            );
            // At least one safetensors file
            let has_safetensors = std::fs::read_dir(&expected_dir)
                .map(|entries| {
                    entries.filter_map(|e| e.ok()).any(|e| {
                        e.path()
                            .extension()
                            .map_or(false, |ext| ext == "safetensors")
                    })
                })
                .unwrap_or(false);
            assert!(has_safetensors, "Should have safetensors files");
        }
        Err(e) => {
            panic!("Failed to load model: {e:?}");
        }
    }
}

/// Test SmolLM2 safetensors model inference via HuggingFaceCandleProvider.
///
/// Downloads the model, loads it, and performs text generation.
#[test]
#[traced_test]
#[ignore = "requires HF_TOKEN and downloads ~270MB model for inference"]
fn test_candle_provider_smollm_inference() {
    let _guard = init_valtron();

    let token = get_token().expect("HF_TOKEN must be set for integration tests");
    let project_root = get_project_root();
    let cache_dir = get_artefacts_dir(project_root.as_path());

    let config = HuggingFaceCandleConfig::builder()
        .hf_token(token)
        .cache_dir(&cache_dir)
        .context_length(512)
        .dtype(CandleDType::F32)
        .build();

    let provider = HuggingFaceCandleProvider::new(config).unwrap();

    let model_id = ModelId::Name("HuggingFaceTB/SmolLM2-135M".to_string(), None);

    let model = provider
        .get_model(model_id)
        .expect("Failed to download and load model");

    // Test generation
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

    let params = foundation_ai::types::ModelParams {
        max_tokens: 32,
        temperature: 0.7,
        top_p: 0.9,
        top_k: 40.0,
        repeat_penalty: 1.1,
        seed: Some(42),
        stop_tokens: vec![],
        thinking_level: foundation_ai::types::ThinkingLevels::default(),
        cache_retention: foundation_ai::types::CacheRetention::default(),
        thinking_budget: None,
    };

    let result = model.generate(interaction, Some(params));
    assert!(
        result.is_ok(),
        "Generation should succeed: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert!(!response.is_empty(), "Response should not be empty");

    // Verify response structure
    if let Messages::Assistant { content, usage, .. } = &response[0] {
        match content {
            ModelOutput::Text(TextContent { content, .. }) => {
                assert!(!content.is_empty(), "Generated text should not be empty");
                println!("Generated: {content}");
            }
            _ => panic!("Expected Text output"),
        }
        assert!(usage.output > 0.0, "Should have generated output tokens");
        assert!(usage.input > 0.0, "Should have input tokens from prompt");
    } else {
        panic!("Expected Assistant response");
    }
}

/// Test model loading via get_model_by_spec with a pre-downloaded local path.
///
/// This test first downloads the model, then loads it via get_model_by_spec
/// to verify the spec-based loading path works independently.
#[test]
#[traced_test]
#[ignore = "requires HF_TOKEN and downloads model"]
fn test_candle_provider_load_by_spec() {
    let _guard = init_valtron();

    let token = get_token().expect("HF_TOKEN must be set for integration tests");
    let project_root = get_project_root();
    let cache_dir = get_artefacts_dir(project_root.as_path());

    // First, download the model
    let download_config = HuggingFaceCandleConfig::builder()
        .hf_token(token.clone())
        .cache_dir(&cache_dir)
        .build();

    let provider = HuggingFaceCandleProvider::new(download_config).unwrap();
    let model_id = ModelId::Name("HuggingFaceTB/SmolLM2-135M".to_string(), None);
    let model = provider
        .get_model(model_id)
        .expect("Failed to download model");

    // Verify we can call get_model_by_spec with the same model
    let spec = model.spec();
    let result = provider.get_model_by_spec(spec);
    assert!(
        result.is_ok(),
        "get_model_by_spec should succeed for cached model"
    );
}
