//! Always-run in-process llama.cpp coverage on a COMMITTED tiny GGUF fixture.
//!
//! WHY: `backends/llamacpp.rs` — where every docs/fixes/006 and 007 bug lived —
//! was otherwise reachable only by tests needing a downloaded/large GGUF, so it
//! sat gated behind `live-model-tests`. This loads the committed
//! `tiny-random-LlamaForCausalLM/tiny-llama-f16.gguf` (2.7 MB, converted from
//! the safetensors fixture via `convert_hf_to_gguf.py`), giving the streaming /
//! generate / BOS / model-cache paths coverage on EVERY `cargo test`, offline.
//!
//! Random weights emit gibberish, so assertions are structural — text is
//! produced, the stream advances past its first token, the two APIs agree.

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelSpec, TextContent, ToolShed, UserModelContent,
};
use foundation_ai::types::ModelState;
use foundation_core::valtron::{valtron_test, Stream};

fn fixture_gguf() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models/tiny-random-LlamaForCausalLM/tiny-llama-f16.gguf")
}

fn load() -> foundation_ai::backends::llamacpp::LlamaModels {
    let path = fixture_gguf();
    assert!(path.exists(), "committed GGUF fixture missing: {path:?}");
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(256)
        .n_threads(2)
        .build();
    let spec = ModelSpec {
        name: "tiny-llama-gguf".to_string(),
        id: ModelId::Name("tiny-llama-gguf".to_string(), None),
        devices: None,
        model_location: Some(path.to_string_lossy().to_string().into()),
        lora_location: None,
    };
    LlamaBackends::LLamaCPU
        .load_model(spec, &config)
        .expect("committed tiny GGUF should load in-process")
}

fn interaction() -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Hi.".to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

fn params() -> ModelParams {
    ModelParams {
        max_tokens: 8,
        ..Default::default()
    }
}

#[valtron_test]
fn tiny_gguf_loads_offline() {
    let _model = load();
}

#[valtron_test]
fn tiny_gguf_generate_produces_output() {
    let model = load();
    let out = model.generate(interaction(), Some(params())).expect("generate");
    assert!(!out.is_empty(), "generate() must produce a message");
}

#[valtron_test]
fn tiny_gguf_stream_advances() {
    let model = load();
    let stream = model.stream(interaction(), Some(params())).expect("stream");
    let mut text = 0;
    let mut items = 0;
    for item in stream {
        items += 1;
        if let Stream::Next(Messages::Assistant {
            content: ModelOutput::Text(_),
            ..
        }) = item
        {
            text += 1;
        }
        if text >= 2 || items > 100 {
            break;
        }
    }
    assert!(text >= 1, "stream must produce at least one text token");
}

/// Full AgentSession::run_turn through the committed llama.cpp GGUF, offline.
///
/// The most direct regression guard for docs/fixes/006 and 007: those bugs were
/// in the llama.cpp STREAM, driven by the AgentLoop. This runs that exact path
/// end to end on every `cargo test` — the loop pumping the real LlamaCppStream.
#[valtron_test]
fn agent_session_run_turn_through_gguf() {
    use foundation_ai::agentic::{
        AgentConfig, AgentSession, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig,
    };
    use foundation_ai::types::{PreloadedProvider, SessionId, SessionRecord};
    use foundation_db::{MemoryDocumentStore, MemoryStorage};

    let model = load(); // concrete LlamaModels (Clone via Arc)
    let model_id = ModelId::Name("tiny-llama-gguf".into(), None);
    let router = PreloadedProvider::new(model, model_id.clone()).into_router();

    let session: AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>> =
        AgentSession::builder(SessionId::new(), router)
            .with_system_prompt("You are a helpful assistant.")
            .with_model(model_id.clone())
            .with_config(AgentConfig {
                primary_model: model_id,
                model_params: params(),
                ..Default::default()
            })
            .with_context_config(ContextConfig::default())
            .with_memory_config(MemoryConfig::default())
            .with_error_policy(ErrorPolicy::new())
            .build()
            .expect("session builds");

    let records = session
        .run_turn(Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Hi.".into(),
                signature: None,
            }),
            signature: None,
        })
        .expect("a real llama.cpp turn must complete, not error");

    assert!(
        records
            .iter()
            .any(|r| matches!(r, SessionRecord::Conversation { .. })),
        "run_turn through the GGUF must emit a conversation record, not just a \
         Summary (the docs/fixes/006 short-circuit): {records:?}"
    );
}

// ---------------------------------------------------------------------------
// Model trait surface on a real loaded GGUF
// ---------------------------------------------------------------------------
//
// The existing fixture tests load the model and immediately generate, so
// spec/descriptor/costing/tool_formatter were never called. Those four are how
// callers inspect a model before using it: tool_formatter() in particular
// decides whether tools are sent as native JSON or as XML text, and llama.cpp
// has no native tool API — it MUST get the text-based formatter, or every tool
// call goes out in a shape the model was never prompted to produce.

#[valtron_test]
fn loaded_gguf_reports_its_spec() {
    let model = load();
    assert_eq!(
        model.spec().name,
        "tiny-llama-gguf",
        "spec must name the model actually loaded"
    );
}

#[valtron_test]
fn loaded_gguf_descriptor_identifies_llamacpp() {
    let model = load();
    let d = model
        .descriptor()
        .expect("the llama.cpp model exposes a descriptor");
    assert_eq!(d.id, "llamacpp");
    assert_eq!(
        d.provider,
        foundation_ai::types::ModelProviders::LLAMACPP,
        "a local model must not claim a cloud provider's identity"
    );
    assert_eq!(
        d.inputs,
        foundation_ai::types::MessageType::TextAndImages,
        "llama.cpp supports multimodal input via mtmd"
    );
}

#[valtron_test]
fn loaded_gguf_costing_starts_at_zero() {
    // Local inference is free, but the accumulator must still exist and start
    // clean — a budget check reads this and must not see garbage.
    let model = load();
    let usage = model.costing().expect("costing is available");
    assert_eq!(usage.total_tokens, 0.0);
}

#[valtron_test]
fn loaded_gguf_uses_the_text_based_tool_formatter() {
    // llama.cpp has no native function-calling API, so tools have to be
    // described in the prompt and parsed back out of the text. Getting a
    // native-JSON formatter here would silently break every tool call.
    let model = load();
    let formatter = model.tool_formatter();
    let instructions = formatter
        .tool_calling_instructions()
        .expect("a text-based model MUST supply prompt instructions for tools");
    assert!(
        instructions.contains("<ToolCall>"),
        "the XML tool-call convention must be described to the model: {instructions}"
    );
}

// ---------------------------------------------------------------------------
// The empty-message-list branch
// ---------------------------------------------------------------------------
//
// Both generate() and stream() special-case an interaction with NO messages:
// the system prompt is used raw, WITHOUT running the chat template (there is no
// conversation to template). That branch is easy to break — routing an empty
// list through apply_chat_template would either error or emit a bare template
// skeleton, and neither surfaces as an obvious failure.

fn system_only_interaction() -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("Continue this text:".to_string()),
        soul: None,
        messages: Vec::new(),
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

#[valtron_test]
fn generate_with_no_messages_uses_the_system_prompt_raw() {
    let model = load();
    let out = model
        .generate(system_only_interaction(), Some(params()))
        .expect("a system-prompt-only interaction is valid input");
    assert!(
        !out.is_empty(),
        "the model must still produce output from a bare system prompt"
    );
}

#[valtron_test]
fn generate_with_no_messages_and_no_system_prompt_is_still_valid() {
    // The fully-degenerate input: nothing at all. It must not panic — an
    // empty prompt is a legitimate (if useless) request.
    let model = load();
    let empty = ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: Vec::new(),
        tools_shed: ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    };
    let result = model.generate(empty, Some(params()));
    assert!(
        result.is_ok(),
        "an entirely empty interaction must not panic or error: {result:?}"
    );
}

#[valtron_test]
fn stream_with_no_messages_advances() {
    // Same branch on the streaming side, which has its own copy of the
    // empty-messages check.
    let model = load();
    let stream = model
        .stream(system_only_interaction(), Some(params()))
        .expect("stream should be created from a bare system prompt");

    let mut saw_item = false;
    for item in stream {
        saw_item = true;
        if let Stream::Pending(ModelState::Error(e)) = item {
            panic!("streaming a bare system prompt errored: {e}");
        }
    }
    assert!(saw_item, "the stream must yield at least one item");
}
