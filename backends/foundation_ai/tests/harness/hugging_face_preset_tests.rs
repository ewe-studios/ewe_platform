//! `RouterPreset::hugging_face` and its CPU/GPU forms, plus the
//! `HuggingFaceGGUFConfig` shortcuts they build on.
//!
//! WHY: a local session should be "repo id + optional config": the preset
//! names the model, the config defaults, and the CPU/GPU forms force the
//! backend without dropping the rest of a supplied config.
//!
//! HOW: constructing a GGUF provider only builds the `HuggingFace` client and
//! creates the cache dir — nothing is downloaded or loaded — so these build
//! real presets and sessions offline. Explicit configs point the cache at a
//! per-test temp dir.

use foundation_core::valtron::valtron_test;
use std::path::PathBuf;

use foundation_ai::agentic::AgentSession;
use foundation_ai::backends::huggingface_gguf_provider::HuggingFaceGGUFConfig;
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::harness::{gguf_provider, Gemma4E2b, Gemma4_26b, Glm52, RouterPreset};
use foundation_ai::types::ModelId;

fn named(id: &str) -> ModelId {
    ModelId::Name(id.to_string(), None)
}

/// A fresh cache dir under the system temp dir, unique per test.
fn temp_cache(test: &str) -> PathBuf {
    std::env::temp_dir()
        .join("foundation_ai_hf_preset_tests")
        .join(format!("{test}-{}", std::process::id()))
}

/// A non-default config: custom cache dir, context, threads and quantization,
/// on the "wrong" backend so forcing it is observable.
fn custom_config(test: &str) -> HuggingFaceGGUFConfig {
    HuggingFaceGGUFConfig::builder()
        .cache_dir(temp_cache(test))
        .context_length(2048)
        .n_threads(3)
        .default_quantization("q5_k_m")
        .llama_backend(LlamaBackends::LLamaMetal)
        .n_gpu_layers(7)
        .build()
}

fn is_gpu_backend(backend: LlamaBackends) -> bool {
    if cfg!(target_vendor = "apple") {
        matches!(backend, LlamaBackends::LLamaMetal)
    } else {
        matches!(backend, LlamaBackends::LLamaGPU)
    }
}

// ---------------------------------------------------------------------------
// Config shortcut

#[test]
fn with_cache_dir_sets_the_dir_and_defaults_the_rest() {
    let config = HuggingFaceGGUFConfig::with_cache_dir("/models/here");
    let default = HuggingFaceGGUFConfig::default();
    let llama_default = LlamaBackendConfig::default();

    assert_eq!(config.cache_dir, PathBuf::from("/models/here"));
    assert!(config.auth.is_none());
    assert_eq!(config.default_quantization, default.default_quantization);
    assert!(matches!(config.llama_backend, LlamaBackends::LLamaCPU));
    assert_eq!(config.llama_config.n_gpu_layers, 0);
    assert_eq!(config.llama_config.context_length, 4096);
    assert_eq!(config.llama_config.batch_size, 512);
    assert_eq!(config.llama_config.n_threads, llama_default.n_threads);
}

#[test]
fn on_cpu_forces_the_backend_and_keeps_the_rest() {
    let config = custom_config("on-cpu").on_cpu();

    assert!(matches!(config.llama_backend, LlamaBackends::LLamaCPU));
    assert_eq!(config.llama_config.n_gpu_layers, 0);
    assert_eq!(config.cache_dir, temp_cache("on-cpu"));
    assert_eq!(config.llama_config.context_length, 2048);
    assert_eq!(config.llama_config.n_threads, 3);
    assert_eq!(config.default_quantization.as_deref(), Some("q5_k_m"));
}

#[test]
fn on_gpu_offloads_every_layer_and_keeps_the_rest() {
    let config = custom_config("on-gpu").on_gpu();

    assert!(is_gpu_backend(config.llama_backend));
    assert_eq!(config.llama_config.n_gpu_layers, u32::MAX);
    assert_eq!(config.cache_dir, temp_cache("on-gpu"));
    assert_eq!(config.llama_config.context_length, 2048);
    assert_eq!(config.llama_config.n_threads, 3);
    assert_eq!(config.default_quantization.as_deref(), Some("q5_k_m"));
}

// ---------------------------------------------------------------------------
// The shared provider base

#[test]
fn gguf_provider_with_none_uses_the_default_config() {
    let provider = gguf_provider(None).expect("default provider builds");
    let default = HuggingFaceGGUFConfig::default();

    assert_eq!(provider.cache_dir(), default.cache_dir.as_path());
    assert_eq!(
        provider.default_quantization(),
        default.default_quantization.as_deref()
    );
    assert!(matches!(provider.llama_backend(), LlamaBackends::LLamaCPU));
    assert_eq!(provider.llama_config().n_gpu_layers, 0);
    assert_eq!(provider.llama_config().context_length, 4096);
}

#[test]
fn gguf_provider_respects_a_supplied_config() {
    let provider = gguf_provider(Some(custom_config("provider"))).expect("custom provider builds");

    assert_eq!(provider.cache_dir(), temp_cache("provider").as_path());
    assert!(temp_cache("provider").is_dir(), "the cache dir is created");
    assert_eq!(provider.default_quantization(), Some("q5_k_m"));
    assert!(matches!(
        provider.llama_backend(),
        LlamaBackends::LLamaMetal
    ));
    assert_eq!(provider.llama_config().n_gpu_layers, 7);
    assert_eq!(provider.llama_config().context_length, 2048);
}

#[test]
fn family_quantization_constructor_overrides_the_config_quantization() {
    // `q3_k_m(Some(cfg))` used to load cfg's `q4_k_m` default instead.
    let provider = Glm52::q3_k_m(Some(HuggingFaceGGUFConfig::with_cache_dir(temp_cache(
        "family-quant",
    ))))
    .expect("provider builds");

    assert_eq!(provider.default_quantization(), Some("Q3_K_M"));
    assert_eq!(provider.cache_dir(), temp_cache("family-quant").as_path());
}

// ---------------------------------------------------------------------------
// Presets

#[test]
fn hugging_face_with_none_builds_a_single_model_preset() {
    let preset = RouterPreset::hugging_face("example-org/some-model-GGUF", None)
        .expect("preset builds with the default config");

    assert_eq!(
        preset.primary_model,
        Some(named("example-org/some-model-GGUF"))
    );
    assert_eq!(preset.memory_model, None);
    assert_eq!(preset.fallback_models, Vec::<ModelId>::new());
    assert!(preset
        .router
        .resolve(&named("example-org/some-model-GGUF"))
        .is_ok());
}

#[test]
fn huggingface_cpu_and_gpu_build_with_a_supplied_config() {
    let cpu = RouterPreset::huggingface_cpu(Gemma4E2b::MODEL_ID, Some(custom_config("preset-cpu")))
        .expect("cpu preset builds");
    assert_eq!(cpu.primary_model, Some(named(Gemma4E2b::MODEL_ID)));
    assert!(
        temp_cache("preset-cpu").is_dir(),
        "the supplied cache dir is used"
    );

    let gpu = RouterPreset::huggingface_gpu(Gemma4E2b::MODEL_ID, Some(custom_config("preset-gpu")))
        .expect("gpu preset builds");
    assert_eq!(gpu.primary_model, Some(named(Gemma4E2b::MODEL_ID)));
    assert!(
        temp_cache("preset-gpu").is_dir(),
        "the supplied cache dir is used"
    );
}

#[test]
fn huggingface_cpu_and_gpu_build_with_none() {
    RouterPreset::huggingface_cpu(Gemma4E2b::MODEL_ID, None).expect("cpu preset builds");
    RouterPreset::huggingface_gpu(Gemma4E2b::MODEL_ID, None).expect("gpu preset builds");
}

#[valtron_test]
async fn hugging_face_preset_model_reaches_the_session() {
    let preset = RouterPreset::huggingface_cpu(
        Gemma4E2b::MODEL_ID,
        Some(HuggingFaceGGUFConfig::with_cache_dir(temp_cache("session"))),
    )
    .expect("preset builds");

    let session = AgentSession::builder(preset)
        .with_system_prompt("You are a helpful assistant.")
        .build()
        .await
        .expect("session builds without with_model");

    assert_eq!(session.models().primary, named(Gemma4E2b::MODEL_ID));
    assert_eq!(session.models().memory, None);
    assert!(session
        .router()
        .resolve(&named(Gemma4E2b::MODEL_ID))
        .is_ok());
}

#[test]
fn named_local_pairs_build_on_the_shared_base() {
    let main = HuggingFaceGGUFConfig::with_cache_dir(temp_cache("pair-main"));
    let memory = HuggingFaceGGUFConfig::with_cache_dir(temp_cache("pair-memory"));

    let preset = RouterPreset::gemma(Some(main), Some(memory)).expect("pair builds");

    assert_eq!(preset.primary_model, Some(named(Gemma4_26b::MODEL_ID)));
    assert_eq!(preset.memory_model, Some(named(Gemma4E2b::MODEL_ID)));
    assert!(temp_cache("pair-main").is_dir());
    assert!(temp_cache("pair-memory").is_dir());
    assert!(preset.router.resolve(&named(Gemma4_26b::MODEL_ID)).is_ok());
    assert!(preset.router.resolve(&named(Gemma4E2b::MODEL_ID)).is_ok());
}
