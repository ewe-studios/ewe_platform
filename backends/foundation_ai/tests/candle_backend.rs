//! Unit tests for Candle backend configuration, config builder, and AuthProvider.
//!
//! These tests verify config defaults, builder patterns, AuthProvider trait,
//! dtype conversion, and repo_id parsing without requiring model files.

#![cfg(feature = "candle")]

use foundation_ai::backends::candle::{
    CandleArchitecture, CandleBackend, CandleBackendConfig, CandleDType,
};
use foundation_ai::backends::huggingface_candle_provider::{
    HuggingFaceCandleConfig, HuggingFaceCandleProvider,
};
use foundation_ai::types::{AuthProvider, ModelProvider};

// ==================================
// CandleBackendConfig Tests
// ==================================

#[test]
fn test_candle_backend_config_default() {
    let config = CandleBackendConfig::default();

    assert_eq!(config.context_length, 4096);
    assert_eq!(config.dtype, CandleDType::F32);
    assert!(matches!(config.architecture, CandleArchitecture::Llama));
    assert!(config.auth.is_none());
    assert!(config.cache_dir.is_none());
}

#[test]
fn test_candle_backend_config_builder() {
    let temp_dir = std::env::temp_dir();
    let config = CandleBackendConfig::builder()
        .context_length(8192)
        .dtype(CandleDType::BF16)
        .architecture(CandleArchitecture::Custom("mistral".to_string()))
        .cache_dir(&temp_dir)
        .build();

    assert_eq!(config.context_length, 8192);
    assert_eq!(config.dtype, CandleDType::BF16);
    assert!(
        matches!(config.architecture, CandleArchitecture::Custom(ref name) if name == "mistral")
    );
    assert_eq!(config.cache_dir.as_deref(), Some(temp_dir.as_path()));
}

#[test]
fn test_candle_backend_config_auth_provider() {
    let config = CandleBackendConfig::default();
    assert!(config.auth().is_none());

    let config = CandleBackendConfig::builder()
        .auth("test_token".to_string())
        .build();
    assert!(config.auth().is_some());
}

#[test]
fn test_candle_backend_config_clone_skips_auth() {
    let config = CandleBackendConfig::builder()
        .auth("secret_token".to_string())
        .context_length(2048)
        .build();

    let cloned = config.clone();

    // Auth should be None after clone (AuthCredential is not Clone)
    assert!(cloned.auth.is_none());
    // Other fields should be preserved
    assert_eq!(cloned.context_length, 2048);
    assert_eq!(cloned.dtype, CandleDType::F32);
    assert!(matches!(cloned.architecture, CandleArchitecture::Llama));
}

#[test]
fn test_candle_dtype_equality() {
    // Verify dtype variants are distinct and can be compared
    assert_eq!(CandleDType::F32, CandleDType::F32);
    assert_eq!(CandleDType::F16, CandleDType::F16);
    assert_eq!(CandleDType::BF16, CandleDType::BF16);
    assert_ne!(CandleDType::F32, CandleDType::F16);
    assert_ne!(CandleDType::F16, CandleDType::BF16);
}

// ==================================
// HuggingFaceCandleConfig Tests
// ==================================

#[test]
fn test_huggingface_candle_config_default() {
    let config = HuggingFaceCandleConfig::default();

    assert_eq!(config.context_length, 4096);
    assert_eq!(config.dtype, CandleDType::F32);
    assert!(matches!(config.architecture, CandleArchitecture::Llama));
    assert!(config.auth.is_none());
    // cache_dir should default to some HF cache path (not None)
    assert!(config.cache_dir.as_os_str().len() > 0);
}

#[test]
fn test_huggingface_candle_config_builder() {
    let temp_dir = std::env::temp_dir().join("candle_test_cache");
    let config = HuggingFaceCandleConfig::builder()
        .hf_token("my_token")
        .cache_dir(&temp_dir)
        .context_length(2048)
        .dtype(CandleDType::F16)
        .architecture(CandleArchitecture::Llama)
        .build();

    assert!(config.auth.is_some());
    assert_eq!(config.cache_dir, temp_dir);
    assert_eq!(config.context_length, 2048);
    assert_eq!(config.dtype, CandleDType::F16);
    assert!(matches!(config.architecture, CandleArchitecture::Llama));
}

#[test]
fn test_huggingface_candle_config_auth_provider() {
    let config = HuggingFaceCandleConfig::default();
    assert!(config.auth().is_none());

    let config = HuggingFaceCandleConfig::builder()
        .hf_token("token_123")
        .build();
    assert!(config.auth().is_some());
}

#[test]
fn test_huggingface_candle_config_clone_skips_auth() {
    let config = HuggingFaceCandleConfig::builder()
        .hf_token("secret")
        .cache_dir("/tmp/test")
        .build();

    let cloned = config.clone();

    assert!(cloned.auth.is_none());
    assert_eq!(cloned.cache_dir, std::path::PathBuf::from("/tmp/test"));
    assert_eq!(cloned.context_length, 4096);
}

// ==================================
// CandleBackend Tests (no model required)
// ==================================

#[test]
fn test_candle_backend_cpu_creation() {
    let backend = CandleBackend::cpu();
    let descriptor = backend.describe().unwrap();

    assert_eq!(descriptor.id, "candle");
    assert!(descriptor.name.contains("CPU"));
    assert_eq!(descriptor.context_window, 4096);
}

#[test]
fn test_candle_backend_create_with_config() {
    let backend = CandleBackend::cpu();
    let config = CandleBackendConfig::builder().context_length(2048).build();

    let result = backend.create(Some(config));
    assert!(result.is_ok());

    let backend = result.unwrap();
    let descriptor = backend.describe().unwrap();
    assert_eq!(descriptor.context_window, 2048);
}

#[test]
fn test_candle_backend_create_without_config() {
    let backend = CandleBackend::cpu();
    let result = backend.create(None::<CandleBackendConfig>);
    assert!(result.is_ok());
}

#[test]
fn test_candle_backend_get_model_requires_path() {
    let backend = CandleBackend::cpu();
    let result = backend.get_model(foundation_ai::types::ModelId::Name(
        "test".to_string(),
        None,
    ));

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err:?}");
    assert!(err_str.contains("local path") || err_str.contains("HuggingFaceCandleProvider"));
}

// ==================================
// HuggingFaceCandleProvider Tests (no download)
// ==================================

#[test]
fn test_huggingface_candle_provider_parse_repo_id() {
    // Valid repo ID
    let model_id =
        foundation_ai::types::ModelId::Name("HuggingFaceTB/SmolLM2-135M".to_string(), None);
    let result = HuggingFaceCandleProvider::parse_repo_id(&model_id);
    assert_eq!(result, Some("HuggingFaceTB/SmolLM2-135M".to_string()));

    // With colon suffix
    let model_id = foundation_ai::types::ModelId::Name(
        "HuggingFaceTB/SmolLM2-135M:something".to_string(),
        None,
    );
    let result = HuggingFaceCandleProvider::parse_repo_id(&model_id);
    assert_eq!(result, Some("HuggingFaceTB/SmolLM2-135M".to_string()));

    // No slash — invalid
    let model_id = foundation_ai::types::ModelId::Name("no_slash".to_string(), None);
    let result = HuggingFaceCandleProvider::parse_repo_id(&model_id);
    assert!(result.is_none());

    // Wrong ModelId variant
    let model_id = foundation_ai::types::ModelId::Alias("test".to_string(), None);
    let result = HuggingFaceCandleProvider::parse_repo_id(&model_id);
    assert!(result.is_none());
}

#[test]
fn test_huggingface_candle_provider_describe() {
    let config = HuggingFaceCandleConfig::builder()
        .cache_dir("/tmp/candle_test")
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
    assert!(!descriptor.reasoning);
}
