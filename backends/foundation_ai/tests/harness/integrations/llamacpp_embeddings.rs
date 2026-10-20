//! Local llama.cpp embedding integration test (spec 67).
//!
//! WHY: the llama.cpp embedding branch used to return `EmbeddingsError::NotEnabled`
//! unconditionally — the embedding context was built with generation params
//! (embeddings off, no pooling), and the encoder was fed the chat-templated
//! prompt instead of the raw text. This test proves the fixed branch end-to-end:
//! an embedding-shaped interaction → `Model::generate` → a non-empty, 768-dim
//! embedding from `nomic-embed-text-v1.5` under the default Mean pooling.
//!
//! WHAT: loads `nomic-embed-text-v1.5.Q8_0.gguf` from `artefacts/models/` —
//! downloaded at run time, NOT committed (real weights are gitignored; see
//! `.gitignore`) — and embeds a short string through `LlamaBackends::LLamaCPU`.
//! Skips when the GGUF is absent, matching `mtp_generate`.
//!
//! HOW: builds the same interaction shape as `make_embedding_interaction`
//! (`agentic/embedding.rs`): an `Assistant` message carrying an empty
//! `ModelOutput::Embedding` marker (what `is_embedding_request` keys on) plus
//! the `User` text.
//!
//! Gated behind the `integration_tests` feature (like the other pull tests), no
//! `#[ignore]`: when the feature is on this is meant to run.
//!
//! Run with:
//! `cargo test -p foundation_ai --features integration_tests --test foundation_ai_tests -- --nocapture harness::integrations::llamacpp_embeddings`

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{
    CostStatus, MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelSpec,
    ModelProviders, StopReason, TextContent, ToolDeclarations, UsageCosting, UsageReport,
    UserModelContent,
};
use foundation_core::valtron::valtron_test;
use infrastructure_llama_cpp::context::params::LlamaPoolingType;

fn project_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories")
        .to_path_buf()
}

/// Locate the `nomic-embed-text-v1.5` GGUF (downloaded at run time into the
/// gitignored `artefacts/models/` dir). `None` → skip.
fn nomic_embed_path() -> Option<std::path::PathBuf> {
    let path = project_root()
        .join("artefacts")
        .join("models")
        .join("nomic-ai--nomic-embed-text-v1.5-GGUF")
        .join("nomic-embed-text-v1.5.Q8_0.gguf");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn zero_usage() -> UsageReport {
    UsageReport {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCosting::zero(CostStatus::Unknown),
    }
}

/// An embedding-shaped interaction: an empty `Embedding` marker on an
/// `Assistant` message (what `is_embedding_request` keys on) + the user text.
fn embedding_interaction(text: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        tools_shed: ToolDeclarations::default(),
        chat_template: None,
        tool_choice: None,
        messages: vec![
            Messages::Assistant {
                id: foundation_compact::ids::new_scru128(),
                model: ModelId::Name("embedding".to_string(), None),
                timestamp: foundation_compact::SystemTime::now(),
                usage: zero_usage(),
                content: ModelOutput::Embedding {
                    dimensions: 0,
                    values: vec![],
                },
                stop_reason: StopReason::Stop,
                provider: ModelProviders::Custom("embedding".into()),
                error_detail: None,
                signature: None,
                metadata: None,
            },
            Messages::User {
                id: foundation_compact::ids::new_scru128(),
                role: MessageRole::User,
                content: UserModelContent::Text(TextContent {
                    content: text.to_string(),
                    signature: None,
                }),
                signature: None,
            },
        ],
    }
}

/// Unit (no model): the embedding context enables embeddings + the configured
/// pooling type, while the generation context leaves embeddings off (R1/R2).
#[test]
fn test_embedding_context_params_enable_embeddings_and_pooling() {
    let gen = LlamaBackendConfig::default().to_context_params();
    assert!(
        !gen.embeddings(),
        "generation context must not enable embeddings"
    );

    // Default pooling is Mean.
    let emb = LlamaBackendConfig::default().to_embedding_context_params();
    assert!(emb.embeddings(), "embedding context must enable embeddings");
    assert_eq!(emb.pooling_type(), LlamaPoolingType::Mean);

    // Builder + a custom pooling type propagate to the context.
    let emb_last = LlamaBackendConfig::builder()
        .with_pooling_type(LlamaPoolingType::Last)
        .build()
        .to_embedding_context_params();
    assert!(emb_last.embeddings());
    assert_eq!(emb_last.pooling_type(), LlamaPoolingType::Last);
}

/// End-to-end: `nomic-embed-text-v1.5` → `Model::generate` → non-empty 768-dim
/// embedding (proves plumbing: no `NotEnabled`, correct dimensions, raw-text
/// path).
#[valtron_test]
fn test_llamacpp_embeddings_nomic_end_to_end() {
    let Some(model_path) = nomic_embed_path() else {
        eprintln!(
            "skipping embedding e2e test: nomic-embed-text-v1.5 GGUF not found under \
             artefacts/models/nomic-ai--nomic-embed-text-v1.5-GGUF/ — download it at \
             run time (weights are gitignored) to enable"
        );
        return;
    };

    let backend = LlamaBackends::LLamaCPU;
    // Mean pooling is the default and correct for nomic-embed-text-v1.5.
    let config = LlamaBackendConfig::builder()
        .context_length(512)
        .batch_size(512)
        .build();

    let spec = ModelSpec {
        name: "nomic-embed-text-v1.5".to_string(),
        id: ModelId::Name("nomic-embed-text-v1.5".to_string(), None),
        devices: None,
        model_location: Some(model_path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = backend
        .load_model(spec, &config)
        .expect("nomic-embed-text-v1.5 should load");

    let output = model
        .generate(
            embedding_interaction("What is the capital of France?"),
            None,
        )
        .expect("embedding generation should succeed");

    let (dimensions, values) = output
        .iter()
        .find_map(|m| match m {
            Messages::Assistant {
                content: ModelOutput::Embedding { dimensions, values },
                ..
            } => Some((*dimensions, values.clone())),
            _ => None,
        })
        .expect("embedding output should carry ModelOutput::Embedding");

    assert!(!values.is_empty(), "embedding values must be non-empty");
    assert_eq!(dimensions, 768, "nomic-embed-text-v1.5 is 768-dimensional");
    assert_eq!(values.len(), dimensions, "values.len() must equal dimensions");

    println!("nomic-embed-text-v1.5 embedding: {dimensions} dims");
}
