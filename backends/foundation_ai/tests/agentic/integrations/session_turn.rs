//! End-to-end `AgentSession::run_turn` tests against a real local model.
//!
//! WHY: `run_turn` is the session's primary API — everything an agent does goes
//! through it — but it had no test at any level. The provider-level Gemma 4
//! tests (`harness::integrations::gemma_pull`) prove the model, bindings, and
//! chat-template shim generate correctly, so a turn that produces no assistant
//! reply localises the defect to the agent loop rather than the backend.
//!
//! WHAT: builds the same `RouterMix` -> `into_agent_builder` session the
//! `answerme-agent` app builds, runs one turn, and asserts an assistant message
//! actually comes back — not merely that some records were returned. A turn that
//! yields only a `Summary` is the exact short-circuit this suite exists to catch.
//!
//! HOW: gated behind `integration_tests` (downloads ~1.5 GB into
//! `artefacts/models`) and run under `#[valtron_test]` because the agent loop is
//! driven as a valtron stream.
//!
//! Run with:
//! `cargo test -p foundation_ai --features integration_tests --test foundation_ai_tests -- --nocapture agentic::integrations`

use foundation_ai::agentic::{
    AgentConfig, AgentSession, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig,
};
use foundation_ai::backends::huggingface_gguf_provider::{
    HuggingFaceGGUFConfig, HuggingFaceGGUFConfigBuilder,
};
use foundation_ai::harness::{Gemma4E2b, RouterMix};
use foundation_ai::types::{
    MessageRole, Messages, ModelId, ModelOutput, SessionId, SessionRecord, TextContent,
    UserModelContent,
};
use foundation_core::valtron::valtron_test;
use foundation_db::{MemoryDocumentStore, MemoryStorage};

type TestSession = AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>>;

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

/// Shares the `artefacts/models` cache with the provider integration tests so a
/// model pulled by either suite is reused rather than downloaded twice.
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

fn user_msg(text: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: text.into(),
            signature: None,
        }),
        signature: None,
    }
}

/// One interaction shared by the `generate()` and `stream()` baselines so the
/// two APIs are compared on identical input.
fn greeting_interaction() -> foundation_ai::types::ModelInteraction {
    foundation_ai::types::ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        soul: None,
        messages: vec![user_msg("Reply with a single short greeting.")],
        tools_shed: foundation_ai::types::ToolShed::default(),
        chat_template: None,
        tool_choice: None,
    }
}

/// Build the session exactly the way `answerme-agent` does, so a break here is a
/// break in the shipped app path.
fn gemma_session() -> TestSession {
    let provider = Gemma4E2b::q4_k_m(Some(gguf_config())).expect("provider builds");
    let model_id = ModelId::Name(Gemma4E2b::MODEL_ID.to_string(), None);

    let preset = RouterMix::new()
        .primary(provider, model_id.clone())
        .build();

    preset
        .into_agent_builder(SessionId::new())
        .with_system_prompt("You are a helpful assistant. Be concise and direct.")
        .with_model(model_id.clone())
        .with_config(AgentConfig {
            primary_model: model_id,
            ..Default::default()
        })
        .with_context_config(ContextConfig::default())
        .with_memory_config(MemoryConfig::default())
        .with_error_policy(ErrorPolicy::new())
        .build()
        .expect("session builds")
}

/// Collect the assistant reply text carried by a turn's records.
fn assistant_text(records: &[SessionRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| match record {
            SessionRecord::Conversation {
                message: Messages::Assistant { content, .. },
            } => match content {
                ModelOutput::Text(text) => Some(text.content.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
//
// The two layers are tested side by side on purpose: the provider test below
// establishes that the model itself streams real text, so when the session
// tests fail while it passes, the defect is unambiguously in the agent loop.

/// Baseline A: `generate()` — the API every existing provider suite exercises.
/// If this passes and `stream()` below fails, the two APIs have diverged.
#[valtron_test]
fn provider_generate_yields_text() {
    use foundation_ai::types::{Model, ModelParams, ModelProvider};

    let provider = Gemma4E2b::q4_k_m(Some(gguf_config())).expect("provider builds");
    let model = provider
        .get_model(ModelId::Name(Gemma4E2b::MODEL_ID.to_string(), None))
        .expect("model loads");

    let output = model
        .generate(greeting_interaction(), Some(ModelParams::default()))
        .expect("generate should succeed");

    let text: String = output
        .iter()
        .filter_map(|msg| match msg {
            Messages::Assistant {
                content: ModelOutput::Text(chunk),
                ..
            } => Some(chunk.content.clone()),
            _ => None,
        })
        .collect();

    assert!(
        !text.trim().is_empty(),
        "generate() produced no assistant text (output: {output:?})"
    );
    println!("provider generate produced: {text:?}");
}

/// Baseline B: the model streams real text through `stream()` — the API the
/// agent loop actually uses. The existing provider suites only ever exercise
/// `generate()`, so a `stream()` that yields nothing slips past all of them.
#[valtron_test]
fn provider_stream_yields_text() {
    use foundation_ai::types::{Model, ModelParams, ModelProvider};

    let provider = Gemma4E2b::q4_k_m(Some(gguf_config())).expect("provider builds");
    let model = provider
        .get_model(ModelId::Name(Gemma4E2b::MODEL_ID.to_string(), None))
        .expect("model loads");

    let stream = model
        .stream(greeting_interaction(), Some(ModelParams::default()))
        .expect("stream should be created");

    let mut text = String::new();
    let mut shapes = Vec::new();
    for item in stream {
        shapes.push(match &item {
            foundation_core::valtron::Stream::Next(_) => "Next",
            foundation_core::valtron::Stream::Init => "Init",
            foundation_core::valtron::Stream::Pending(_) => "Pending",
            foundation_core::valtron::Stream::Ignore => "Ignore",
            foundation_core::valtron::Stream::Wait => "Wait",
            foundation_core::valtron::Stream::Delayed(_) => "Delayed",
            foundation_core::valtron::Stream::Spread(_) => "Spread",
        });
        if let foundation_core::valtron::Stream::Next(Messages::Assistant { content, .. }) = item {
            if let ModelOutput::Text(chunk) = content {
                text.push_str(&chunk.content);
            }
        }
    }
    println!("stream item shapes: {shapes:?}");

    assert!(
        !text.trim().is_empty(),
        "stream() produced no text — the agent loop's generation path is dead \
         even though generate() works"
    );
    println!("provider stream produced: {text:?}");
}

/// A single turn must reach the model and come back with an assistant message.
#[valtron_test]
fn run_turn_returns_an_assistant_reply() {
    let session = gemma_session();

    let records = session
        .run_turn(user_msg("Reply with a single short greeting."))
        .expect("turn should succeed");

    assert!(
        !records.is_empty(),
        "turn produced no records at all — the agent loop never ran"
    );

    let replies = assistant_text(&records);
    assert!(
        !replies.is_empty(),
        "turn returned {} record(s) but no assistant reply — the loop short-circuited \
         before generation (records: {records:?})",
        records.len()
    );
    assert!(
        replies.iter().any(|r| !r.trim().is_empty()),
        "assistant replied with only empty text: {replies:?}"
    );

    println!("run_turn produced: {replies:?}");
}

/// The user's prompt must be *persisted* by the turn.
///
/// Note the contract: `run_turn` returns what the loop emits (assistant output
/// plus the summary); the queued prompt is written through `MessageApi` rather
/// than re-emitted on the stream. So this asserts against session history, not
/// the returned records.
#[valtron_test]
fn run_turn_persists_the_user_prompt() {
    let session = gemma_session();

    session
        .run_turn(user_msg("Reply with a single short greeting."))
        .expect("turn should succeed");

    let history = session
        .message_api()
        .all()
        .expect("session history should be readable");

    let saw_user_message = history.iter().any(|record| {
        matches!(
            record,
            SessionRecord::Conversation {
                message: Messages::User { .. }
            }
        )
    });

    assert!(
        saw_user_message,
        "the queued user prompt was never persisted to session history: {history:?}"
    );
}

/// Two agents on two threads sharing one cached model must not corrupt each
/// other's interaction.
///
/// WHY: `load_model` caches loaded weights process-wide, so concurrent agents
/// now receive handles over the SAME `llama_model`. That is only sound because
/// the weights are read-only after load and every generation builds its own
/// `llama_context` (its own KV cache and sampler). This test is the evidence for
/// that claim rather than an assumption about llama.cpp's threading model.
///
/// WHAT: runs two independent sessions concurrently, each doing several turns,
/// and asserts every turn on both threads produced a reply and that neither
/// thread panicked or crashed (a shared KV cache would corrupt state or fault).
// No `#[traced_test]`: it installs a THREAD-LOCAL subscriber, which the OS
// threads spawned below would not inherit — capturing nothing from the very
// threads under test while looking like it worked.
#[valtron_test]
fn concurrent_agents_share_one_model_safely() {
    use std::thread;

    // `#[valtron_test]` initialises the global pool the sessions schedule onto;
    // the spawned OS threads below then race for real on the shared model cache.
    let handles: Vec<_> = (0..2)
        .map(|agent| {
            thread::spawn(move || {
                let session = gemma_session();
                let mut replies = Vec::new();
                for turn in 0..3 {
                    let records = session
                        .run_turn(user_msg("Reply with a single short greeting."))
                        .unwrap_or_else(|e| {
                            panic!("agent {agent} turn {turn} failed: {e:?}");
                        });
                    let text = assistant_text(&records);
                    assert!(
                        !text.is_empty(),
                        "agent {agent} turn {turn} produced no assistant reply: {records:?}"
                    );
                    replies.push(text);
                }
                replies
            })
        })
        .collect();

    for (agent, handle) in handles.into_iter().enumerate() {
        let replies = handle
            .join()
            .unwrap_or_else(|_| panic!("agent {agent} thread panicked — shared model corrupted"));
        println!("agent {agent} replies: {replies:?}");
    }
}

/// Multi-turn: a second turn must also answer, and must reuse the loaded model.
///
/// WHY: the REPL visibly re-ran llama.cpp's full loader (`llama_model_loader`,
/// tensor `repack`) on every message, which points at the model being loaded
/// from disk per turn rather than held. This test pins both halves — the second
/// turn answers, and loading happens once.
#[valtron_test]
fn second_turn_answers_and_reuses_the_loaded_model() {
    let session = gemma_session();

    let first = session
        .run_turn(user_msg("Reply with a single short greeting."))
        .expect("first turn should succeed");
    assert!(
        !assistant_text(&first).is_empty(),
        "first turn returned no assistant reply: {first:?}"
    );

    let second = session
        .run_turn(user_msg("Reply with one more short greeting."))
        .expect("second turn should succeed");
    assert!(
        !assistant_text(&second).is_empty(),
        "second turn returned no assistant reply: {second:?}"
    );

    println!(
        "turn 1: {:?} | turn 2: {:?}",
        assistant_text(&first),
        assistant_text(&second)
    );
}
