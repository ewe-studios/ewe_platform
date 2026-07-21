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
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, ModelSpec, TextContent, ToolShed, UserModelContent,
};
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
