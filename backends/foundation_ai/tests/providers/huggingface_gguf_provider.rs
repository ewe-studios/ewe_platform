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
    fn gguf_download_of_nonexistent_repo_errors() {
        // A nonexistent repo 404s (no token needed) — exercises download_model's
        // body + error mapping without a large download. Gated (needs network).
        let config = HuggingFaceGGUFConfig::builder()
            .cache_dir(&cache_dir())
            .build();
        let provider = HuggingFaceGGUFProvider::new(config).expect("provider builds");
        let bad = ModelId::Name(
            "ewe-platform-nonexistent/does-not-exist-xyz-404".to_string(),
            Some(Quantization::Q2K),
        );
        assert!(
            provider.get_model(bad).is_err(),
            "a nonexistent repo must fail to download"
        );
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

// ---------------------------------------------------------------------------
// parse_model_id — the remaining branches
// ---------------------------------------------------------------------------

/// A default-config provider; parse_model_id needs no cache or network.
fn provider() -> HuggingFaceGGUFProvider {
    HuggingFaceGGUFProvider::new(HuggingFaceGGUFConfig::default())
        .expect("provider builds offline")
}

#[test]
fn parse_model_id_rejects_non_name_variants() {
    // Only ModelId::Name carries a repo path. Alias/Group/Architecture have no
    // repo to resolve, so they must return None rather than being coerced.
    let provider = provider();
    for id in [
        ModelId::Alias("some/repo".to_string(), None),
        ModelId::Group("some/repo".to_string(), None),
        ModelId::Architecture("some/repo".to_string(), None),
    ] {
        assert!(
            provider.parse_model_id(&id).is_none(),
            "{id:?} has no repo path and must not parse"
        );
    }
}

#[test]
fn parse_model_id_accepts_repo_revision_quantization() {
    // The three-part form pins both a branch and a quantization.
    let parsed = provider()
        .parse_model_id(&ModelId::Name(
            "org/model:v2-branch:q5_k_m".to_string(),
            None,
        ))
        .expect("the three-part form must parse");
    assert_eq!(parsed.repo_id, "org/model");
    assert_eq!(parsed.revision, "v2-branch");
    assert_eq!(parsed.quantization.as_deref(), Some("q5_k_m"));
}

#[test]
fn parse_model_id_rejects_more_than_three_parts() {
    // A fourth colon is ambiguous — better to reject than to guess which field
    // the extra segment belongs to.
    assert!(
        provider()
            .parse_model_id(&ModelId::Name("a/b:c:d:e".to_string(), None))
            .is_none(),
        "an over-long id must not be silently truncated"
    );
}

#[test]
fn an_explicit_quantization_enum_beats_the_string_form() {
    // ModelId's Quantization takes priority over a `:suffix`; otherwise a
    // caller who passed the enum explicitly would be silently overridden.
    let parsed = provider()
        .parse_model_id(&ModelId::Name(
            "org/model:q2_k".to_string(),
            Some(Quantization::Q4_KM),
        ))
        .expect("parses");
    assert_eq!(
        parsed.quantization.as_deref(),
        Some("Q4_K_M"),
        "the enum must win over the string suffix"
    );
}

// ---------------------------------------------------------------------------
// find_cached_file (via download_model's cache short-circuit)
// ---------------------------------------------------------------------------
//
// The cache lookup is what stops a re-download of a multi-GB GGUF. It matches
// on a quantization-derived filename pattern, so both the hit and the
// wrong-quantization miss matter: a too-loose match hands back the WRONG
// weights, which is worse than a re-download.
//
// The HIT tests are plain `#[test]` with no valtron pool on purpose — they pass
// only because the lookup returns before any network work, which is itself the
// property under test. The MISS test reaches the download path, so it needs the
// pool; the fetch then fails offline, which is fine: the assertion is only that
// the wrong file is never handed back.

fn fresh_cache(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "hf-gguf-{tag}-{}",
        foundation_compact::ids::new_scru128()
    ));
    std::fs::create_dir_all(&dir).expect("temp cache");
    dir
}

fn seed(cache: &std::path::Path, repo: &str, filename: &str) -> std::path::PathBuf {
    let dir = cache.join(repo.replace('/', "--"));
    std::fs::create_dir_all(&dir).expect("repo dir");
    let f = dir.join(filename);
    std::fs::write(&f, b"").expect("marker gguf");
    f
}

fn provider_with_cache(cache: &std::path::Path) -> HuggingFaceGGUFProvider {
    let config = HuggingFaceGGUFConfig::builder()
        .cache_dir(cache.to_path_buf())
        .build();
    HuggingFaceGGUFProvider::new(config).expect("provider builds offline")
}

#[test]
fn a_cached_gguf_matching_the_quantization_is_found() {
    let cache = fresh_cache("hit");
    seed(&cache, "org/model", "model-Q4_K_M.gguf");

    let provider = provider_with_cache(&cache);
    let parsed = provider
        .parse_model_id(&ModelId::Name("org/model:q4_k_m".to_string(), None))
        .expect("parses");
    let result = provider.download_model(&parsed);
    std::fs::remove_dir_all(&cache).ok();

    let path = result.expect("a matching cached file must short-circuit the download");
    assert!(
        path.file_name().is_some_and(|n| n == "model-Q4_K_M.gguf"),
        "got {path:?}"
    );
}

#[test]
fn the_quantization_match_is_case_insensitive_on_the_request_side() {
    // Callers write `q4_k_m` lowercase; files ship uppercase. The pattern
    // uppercases the request, so this must still hit.
    let cache = fresh_cache("case");
    seed(&cache, "org/model", "somemodel.Q4_K_M.gguf");

    let provider = provider_with_cache(&cache);
    let parsed = provider
        .parse_model_id(&ModelId::Name("org/model:q4_k_m".to_string(), None))
        .expect("parses");
    let result = provider.download_model(&parsed);
    std::fs::remove_dir_all(&cache).ok();

    assert!(result.is_ok(), "lowercase request must match an uppercase filename");
}

#[foundation_core::valtron::valtron_test]
fn a_cached_file_of_a_different_quantization_is_not_returned() {
    // The dangerous case: returning a Q2_K file for a Q4_K_M request would load
    // the wrong weights silently. A miss here should attempt a download (and
    // fail without network) rather than hand back the wrong file.
    let cache = fresh_cache("wrongquant");
    let wrong = seed(&cache, "org/model", "model-Q2_K.gguf");

    let provider = provider_with_cache(&cache);
    let parsed = provider
        .parse_model_id(&ModelId::Name("org/model:q4_k_m".to_string(), None))
        .expect("parses");
    let result = provider.download_model(&parsed);
    std::fs::remove_dir_all(&cache).ok();

    if let Ok(path) = result {
        assert_ne!(
            path, wrong,
            "a Q2_K file must never satisfy a Q4_K_M request"
        );
    }
}
