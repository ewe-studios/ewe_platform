//! Harness model-pull integration tests.
//!
//! WHY: The harness presets claim to "just work" for real models. That promise
//! is only meaningful if a preset can actually pull a model from HuggingFace,
//! load it through llama.cpp, and generate — both directly (provider preset)
//! and end-to-end through a harness router (the path agents take).
//!
//! WHAT: pulls the smallest Gemma 4 variant (E2B, the harness memory model) and
//! exercises (1) the [`Gemma4E2b`] preset's provider directly and (2) the
//! [`harness::gemma_router`] memory model resolved + downloaded through the
//! router. Mirrors `tests/providers/integrations` — gated behind the
//! `integration_tests` feature (no `#[ignore]`: when the feature is on these
//! are meant to run, like the llama.cpp download tests). It downloads ~1.5 GB.
//!
//! HOW: caches into the repo `artefacts/models` dir (same as the GGUF provider
//! integration tests), honours `HF_TOKEN` when present, and runs under
//! `#[valtron_test]` because the download + inference paths use valtron streams.
//!
//! Run with:
//! `cargo test -p foundation_ai --features integration_tests --test foundation_ai_tests -- --nocapture harness::integrations`

use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFConfigBuilder,
};
use foundation_ai::harness::{self, Gemma4E2b};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelParams, ModelProvider, TextContent,
    ToolShed, UserModelContent,
};
use foundation_core::valtron::valtron_test;
use tracing_test::traced_test;

// ---------------------------------------------------------------------------
// Helpers

fn project_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories")
        .to_path_buf()
}

/// GGUF config that caches into the shared `artefacts/models` dir, pins the
/// smallest practical quantization, and forwards `HF_TOKEN` when set.
fn gguf_config() -> HuggingFaceGGUFConfig {
    let cache_dir = project_root().join("artefacts").join("models");
    let builder: HuggingFaceGGUFConfigBuilder = HuggingFaceGGUFConfig::builder()
        .cache_dir(cache_dir)
        .default_quantization("Q4_K_M");
    let builder = match std::env::var("HF_TOKEN") {
        Ok(token) => builder.token(token),
        Err(_) => builder,
    };
    builder.build()
}

fn hello_interaction() -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Reply with a single short greeting.".to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Pull + load + generate via the Gemma4E2b provider preset directly

#[valtron_test]
#[traced_test]
fn test_pull_gemma4_e2b_via_preset() {
    let provider = Gemma4E2b::q4_k_m(Some(gguf_config())).expect("provider builds");

    let model = provider
        .get_model(ModelId::Name(Gemma4E2b::MODEL_ID.to_string(), None))
        .expect("Gemma 4 E2B should download and load");

    let output = model
        .generate(hello_interaction(), Some(ModelParams::default()))
        .expect("generation should succeed");

    assert!(!output.is_empty(), "model should return at least one message");
    println!("Gemma 4 E2B (preset) generated: {output:?}");
}

// ---------------------------------------------------------------------------
// 2. Pull + load + generate via a harness router (the agent path)

#[valtron_test]
#[traced_test]
fn test_pull_gemma4_memory_via_harness_router() {
    // gemma_router pairs a big main model (26B) with the small E2B memory model.
    // Building both providers downloads nothing; we only resolve + pull the
    // memory model, so this test stays at the E2B download size.
    let preset =
        harness::gemma_router(Some(gguf_config()), Some(gguf_config())).expect("router builds");

    let memory_id = preset
        .memory_model
        .clone()
        .expect("gemma_router wires a memory model");
    assert_eq!(memory_id, ModelId::Name(Gemma4E2b::MODEL_ID.to_string(), None));

    let model = preset
        .router
        .get_model(&memory_id)
        .expect("router should resolve, download, and load the memory model");

    let output = model
        .generate(hello_interaction(), Some(ModelParams::default()))
        .expect("generation should succeed");

    assert!(!output.is_empty(), "model should return at least one message");
    println!("Gemma 4 E2B (router) generated: {output:?}");
}
