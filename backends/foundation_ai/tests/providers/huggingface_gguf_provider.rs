//! Integration tests for `HuggingFaceGGUFProvider`.
//!
//! Two tiers (spec-60 F03):
//!   * **Offline** — model-id parsing and provider description. No network, run
//!     by default with the `provider_tests` suite.
//!   * **External service** — download SmolLM2 from the HuggingFace Hub and run
//!     inference. Gated behind `external-service-tests` and self-skips (never
//!     fails) when `HF_TOKEN` is unset, so a keyless run is green. All network
//!     tests share one cache dir + one small model, so the model is pulled once
//!     and reused.

use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFProvider,
};
use foundation_ai::types::{
    Model, ModelId, ModelProvider, Quantization,
};
use foundation_core::valtron::valtron_test;

// ---------------------------------------------------------------------------
// Offline — always run (no network, no token)
// ---------------------------------------------------------------------------

#[valtron_test]
fn gguf_provider_parses_model_ids() {
    let config = HuggingFaceGGUFConfig::default();
    let provider = HuggingFaceGGUFProvider::new(config).unwrap();

    // repo:quant
    let parsed = provider
        .parse_model_id(&ModelId::Name(
            "TheBloke/Llama-2-7B-GGUF:q4_k_m".to_string(),
            None,
        ))
        .expect("parses repo:quant");
    assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
    assert_eq!(parsed.quantization, Some("q4_k_m".to_string()));
    assert_eq!(parsed.revision, "main");

    // repo:revision:quant
    let parsed = provider
        .parse_model_id(&ModelId::Name(
            "TheBloke/Llama-2-7B-GGUF:main:q5_k_m".to_string(),
            None,
        ))
        .expect("parses repo:rev:quant");
    assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
    assert_eq!(parsed.quantization, Some("q5_k_m".to_string()));
    assert_eq!(parsed.revision, "main");

    // repo only → default quantization
    let parsed = provider
        .parse_model_id(&ModelId::Name("TheBloke/Llama-2-7B-GGUF".to_string(), None))
        .expect("parses bare repo");
    assert_eq!(parsed.repo_id, "TheBloke/Llama-2-7B-GGUF");
    assert_eq!(parsed.revision, "main");
    assert!(parsed.quantization.is_some(), "bare repo gets a default quant");
}

#[valtron_test]
fn gguf_provider_parses_quantization_enum_and_revision() {
    let provider = HuggingFaceGGUFProvider::new(HuggingFaceGGUFConfig::default()).unwrap();

    // A ModelId Quantization enum takes priority and maps to GGUF filename form.
    let parsed = provider
        .parse_model_id(&ModelId::Name("some/repo".to_string(), Some(Quantization::Q2K)))
        .expect("parses with enum quant");
    assert_eq!(parsed.repo_id, "some/repo");
    assert_eq!(parsed.quantization, Some("Q2_K".to_string()));

    let parsed = provider
        .parse_model_id(&ModelId::Name(
            "some/repo".to_string(),
            Some(Quantization::Q4_KM),
        ))
        .expect("parses with enum quant");
    assert_eq!(parsed.quantization, Some("Q4_K_M".to_string()));

    // repo:<not-a-quant> is treated as a revision, not a quantization.
    let parsed = provider
        .parse_model_id(&ModelId::Name("some/repo:v2-branch".to_string(), None))
        .expect("parses repo:revision");
    assert_eq!(parsed.repo_id, "some/repo");
    assert_eq!(parsed.revision, "v2-branch");
}

#[valtron_test]
fn gguf_quantization_filename_pattern() {
    assert_eq!(
        HuggingFaceGGUFProvider::quantization_to_filename_pattern("q4_k_m"),
        "*Q4_K_M.gguf"
    );
    assert_eq!(
        HuggingFaceGGUFProvider::quantization_to_filename_pattern("q2_k"),
        "*Q2_K.gguf"
    );
}

#[valtron_test]
fn gguf_config_builder_full_chain_and_clone() {
    use foundation_ai::backends::llamacpp::LlamaBackends;

    let config = HuggingFaceGGUFConfig::builder()
        .token("hf_xxx")
        .cache_dir("/tmp/gguf-cache")
        .default_quantization("Q4_K_M")
        .n_gpu_layers(10)
        .n_threads(3usize)
        .context_length(1024usize)
        .llama_backend(LlamaBackends::LLamaCPU)
        .build();

    assert_eq!(config.default_quantization, Some("Q4_K_M".to_string()));
    // Clone preserves the config (exercises the manual Clone impl).
    let cloned = config.clone();
    assert_eq!(cloned.default_quantization, config.default_quantization);
}

#[valtron_test]
fn gguf_config_auth_provider_and_clone_drops_secret() {
    use foundation_ai::backends::llamacpp::LlamaBackendConfig;
    use foundation_ai::types::AuthProvider;

    // `.token()` populates the auth credential; AuthProvider exposes it.
    let config = HuggingFaceGGUFConfig::builder()
        .token("hf_secret")
        .llama_config(LlamaBackendConfig::default())
        .build();
    assert!(config.auth().is_some(), "token sets the auth credential");

    // Clone intentionally drops the (non-Clone) secret — documents the behaviour.
    let cloned = config.clone();
    assert!(cloned.auth().is_none(), "clone drops the auth secret");
}

#[valtron_test]
fn gguf_provider_new_with_token_creates_cache_dir() {
    // Constructing a provider from a token-bearing config exercises new()'s
    // SecretOnly auth extraction + HFClient build + cache-dir creation, all
    // offline (no network until a model is fetched).
    let tmp = std::env::temp_dir().join(format!("gguf_new_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let config = HuggingFaceGGUFConfig::builder()
        .token("hf_secret")
        .cache_dir(&tmp)
        .build();
    let provider = HuggingFaceGGUFProvider::new(config).expect("provider builds with token");
    assert!(tmp.exists(), "new() creates the cache directory");
    // describe() works on the token-configured provider too.
    assert_eq!(provider.describe().unwrap().id, "huggingface");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[valtron_test]
fn gguf_provider_describes_itself() {
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

// ---------------------------------------------------------------------------
// External service — HuggingFace Hub download + inference (SmolLM2-360M).
// Gated behind `external-service-tests`; self-skips without HF_TOKEN.
// ---------------------------------------------------------------------------

#[cfg(feature = "external-service-tests")]
mod live {
    use super::*;
    use foundation_ai::types::{
        MessageRole, Messages, ModelInteraction, ModelParams, TextContent, ToolShed,
        UserModelContent,
    };

    /// The single small GGUF model every network test in this module shares.
    const SMOLLM_REPO: &str = "unsloth/SmolLM2-360M-Instruct-GGUF";

    fn project_root() -> std::path::PathBuf {
        let manifest_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
        std::path::Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn cache_dir() -> std::path::PathBuf {
        project_root().join("artefacts").join("models")
    }

    /// Build a CPU provider pointed at the shared cache, or `None` (with a skip
    /// note) when `HF_TOKEN` is unset. The shared cache means SmolLM is pulled
    /// once and reused across the tests below.
    fn provider_or_skip() -> Option<HuggingFaceGGUFProvider> {
        let token = std::env::var("HF_TOKEN").ok().filter(|t| !t.is_empty());
        let Some(token) = token else {
            eprintln!("[skip] HF_TOKEN not set — skipping HuggingFace Hub download test");
            return None;
        };
        let config = HuggingFaceGGUFConfig::builder()
            .token(token)
            .cache_dir(&cache_dir())
            .llama_backend(foundation_ai::backends::llamacpp::LlamaBackends::LLamaCPU)
            .n_gpu_layers(0)
            .n_threads(2usize)
            .context_length(512usize)
            .build();
        Some(HuggingFaceGGUFProvider::new(config).expect("provider builds"))
    }

    #[valtron_test]
    fn gguf_downloads_smollm_to_cache() {
        let Some(provider) = provider_or_skip() else {
            return;
        };
        let model_id = ModelId::Name(SMOLLM_REPO.to_string(), Some(Quantization::Q2K));
        provider.get_model(model_id).expect("SmolLM downloads + loads");

        let expected = cache_dir()
            .join("unsloth--SmolLM2-360M-Instruct-GGUF/SmolLM2-360M-Instruct-Q2_K.gguf");
        assert!(expected.exists(), "GGUF file cached at {expected:?}");
    }

    #[valtron_test]
    fn gguf_smollm_generates() {
        let Some(provider) = provider_or_skip() else {
            return;
        };
        let model_id = ModelId::Name(SMOLLM_REPO.to_string(), Some(Quantization::Q2K));
        let model = provider.get_model(model_id).expect("load SmolLM");

        let interaction = ModelInteraction {
            system_prompt: Some("You are a helpful assistant.".to_string()),
            soul: None,
            messages: vec![Messages::User {
                id: foundation_compact::ids::new_scru128(),
                role: MessageRole::User,
                content: UserModelContent::Text(TextContent {
                    content: "Reply with a single friendly word.".to_string(),
                    signature: None,
                }),
                signature: None,
            }],
            tools_shed: ToolShed::default(),
            chat_template: None,
            tool_choice: None,
        };

        let response = model
            .generate(interaction, Some(ModelParams::default()))
            .expect("SmolLM generation succeeds");
        assert!(!response.is_empty(), "response should not be empty");
        println!("SmolLM generated: {response:?}");
    }
}
