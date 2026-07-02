//! MTP end-to-end speculative-generation integration test (spec-51 Phase 2).
//!
//! WHY: Phase 2 wires the C++ `common/speculative.h` engine (via
//! `wrapper_mtp.{h,cpp}` and the [`LlamaMtp`] safe wrapper) into the llama.cpp
//! provider's decode path. This test proves the full pipeline end-to-end:
//! target model + separate MTP head GGUF → `Model::generate` → non-empty
//! assistant text with drafted/accepted counts surfaced through tracing.
//!
//! WHAT: loads Gemma 4 E2B (`gemma-4-E2B-it-Q4_K_M.gguf`) as the target and
//! `mtp-gemma-4-E2B-it.gguf` as the draft head — both already present under
//! `artefacts/models/unsloth--gemma-4-E2B-it-GGUF/` from the harness pull test
//! and the manual MTP head download recorded in the spec. If either file is
//! missing (fresh clone, no cache) the test is skipped rather than failing
//! spuriously.
//!
//! HOW: builds a [`LlamaBackendConfig`] with `.mtp(Some(mtp_path), 4)`, loads
//! via `LlamaBackends::LLamaCPU`, and runs a short generation through
//! `Model::generate`. Correctness of drafts is enforced by the speculative
//! engine itself (drafts are verified against the target, so output is not
//! degraded); we assert the result is non-empty text.
//!
//! Gated behind the `integration_tests` feature (like the other pull tests),
//! no `#[ignore]`: when the feature is on this is meant to run.

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelSpec,
    TextContent, ToolShed, UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};
use tracing_test::traced_test;

fn project_root() -> std::path::PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    std::path::Path::new(&manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("Should have parent directories")
        .to_path_buf()
}

fn gemma_paths() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let base = project_root()
        .join("artefacts")
        .join("models")
        .join("unsloth--gemma-4-E2B-it-GGUF");
    let target = base.join("gemma-4-E2B-it-Q4_K_M.gguf");
    let draft = base.join("mtp-gemma-4-E2B-it.gguf");
    if target.exists() && draft.exists() {
        Some((target, draft))
    } else {
        None
    }
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

/// End-to-end: Gemma 4 E2B + MTP head → non-empty generated text.
#[valtron_test]
#[traced_test]
fn test_mtp_generate_gemma4_e2b_end_to_end() {
    let Some((target_path, draft_path)) = gemma_paths() else {
        eprintln!(
            "skipping MTP e2e test: cached Gemma 4 E2B or MTP head not found under \
             artefacts/models/unsloth--gemma-4-E2B-it-GGUF/ — run the gemma_pull \
             integration test and download mtp-gemma-4-E2B-it.gguf to enable"
        );
        return;
    };

    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .context_length(2048)
        .batch_size(512)
        .mtp(Some(draft_path.clone()), 4)
        .build();

    let spec = ModelSpec {
        name: "gemma-4-E2B-it-Q4_K_M".to_string(),
        id: ModelId::Name("gemma-4-E2B-it".to_string(), None),
        devices: None,
        model_location: Some(target_path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = backend
        .load_model(spec, &config)
        .expect("Gemma 4 E2B + MTP head should load under the capability gate");

    let params = ModelParams {
        max_tokens: 32,
        ..ModelParams::default()
    };

    let output = model
        .generate(hello_interaction(), Some(params))
        .expect("MTP speculative generation should succeed");

    assert!(
        !output.is_empty(),
        "MTP generation should return at least one message"
    );

    let text = output.iter().find_map(|m| match m {
        Messages::Assistant { content, .. } => match content {
            ModelOutput::Text(TextContent { content, .. }) => Some(content.clone()),
            _ => None,
        },
        _ => None,
    });
    let text = text.expect("MTP generation should yield an Assistant text message");
    assert!(
        !text.trim().is_empty(),
        "MTP-generated text must be non-empty; got: {text:?}"
    );

    println!("MTP end-to-end generated: {text:?}");
}

/// Streaming MTP: `Model::stream` drives the engine step-by-step and yields
/// non-empty assistant pieces (spec-51 #1 follow-up).
#[valtron_test]
#[traced_test]
fn test_mtp_stream_gemma4_e2b() {
    let Some((target_path, draft_path)) = gemma_paths() else {
        eprintln!("skipping MTP stream test: cached Gemma 4 E2B or MTP head not found");
        return;
    };

    let backend = LlamaBackends::LLamaCPU;
    let config = LlamaBackendConfig::builder()
        .context_length(2048)
        .batch_size(512)
        .mtp(Some(draft_path), 4)
        .build();

    let spec = ModelSpec {
        name: "gemma-4-E2B-it-Q4_K_M".to_string(),
        id: ModelId::Name("gemma-4-E2B-it".to_string(), None),
        devices: None,
        model_location: Some(target_path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = backend
        .load_model(spec, &config)
        .expect("Gemma 4 E2B + MTP head should load");

    let params = ModelParams {
        max_tokens: 24,
        ..ModelParams::default()
    };

    let stream = model
        .stream(hello_interaction(), Some(params))
        .expect("MTP stream should start");

    let mut collected = String::new();
    let mut saw_init = false;
    for item in stream {
        match item {
            Stream::Init => saw_init = true,
            Stream::Next(Messages::Assistant { content, .. }) => {
                if let ModelOutput::Text(TextContent { content, .. }) = content {
                    collected.push_str(&content);
                }
            }
            _ => {}
        }
    }

    assert!(saw_init, "stream should emit Init");
    assert!(
        !collected.trim().is_empty(),
        "MTP stream must yield non-empty text; got: {collected:?}"
    );
    println!("MTP stream collected: {collected:?}");
}
