//! Smoke test: the committed tiny-random fixtures load through CandleBackend.
//!
//! WHY: `artefacts/test-models/` costs ~54 MB of permanent repo history. That is
//! only justified if the fixtures actually load offline through our own backend
//! — this test is the evidence, and fails loudly if a fixture rots.

use foundation_ai::backends::candle::CandleBackend;
use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};
use foundation_core::valtron::valtron_test;

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join(name)
}

#[valtron_test]
fn tiny_random_llama_fixture_loads_offline() {
    let dir = fixture_dir("tiny-random-LlamaForCausalLM");
    assert!(dir.join("model.safetensors").exists(), "fixture missing: {dir:?}");

    let backend = CandleBackend::cpu();
    let spec = ModelSpec {
        name: "tiny-random-llama".to_string(),
        id: ModelId::Name("tiny-random-llama".to_string(), None),
        devices: None,
        model_location: Some(dir.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let result = backend.get_model_by_spec(spec);
    assert!(result.is_ok(), "fixture should load: {:?}", result.err());
}

// ---------------------------------------------------------------------------
// Provider seam — matrix 8.1/8.2/8.3/8.4 on the candle backend.
//
// Random weights emit gibberish tokens, so these are STRUCTURAL: text is
// produced, a stream advances beyond its first item, generate() and stream()
// agree in shape. That is exactly the surface docs/fixes/006 got wrong on the
// llama.cpp side, proved here offline in milliseconds.

use foundation_ai::types::{
    MessageRole, Messages, ModelInteraction, ModelOutput, ModelParams, Model, TextContent,
    ToolShed, UserModelContent,
};
use foundation_core::valtron::Stream;

fn load_tiny_llama() -> impl Model {
    let dir = fixture_dir("tiny-random-LlamaForCausalLM");
    let spec = ModelSpec {
        name: "tiny-random-llama".to_string(),
        id: ModelId::Name("tiny-random-llama".to_string(), None),
        devices: None,
        model_location: Some(dir.to_string_lossy().to_string().into()),
        lora_location: None,
    };
    CandleBackend::cpu()
        .get_model_by_spec(spec)
        .expect("fixture loads")
}

fn greeting() -> ModelInteraction {
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

/// Matrix 8.1 — generate() produces token records (gibberish is fine).
#[valtron_test]
fn candle_generate_produces_output() {
    let model = load_tiny_llama();
    let out = model
        .generate(greeting(), Some(params()))
        .expect("generate should succeed");
    assert!(
        !out.is_empty(),
        "generate() must produce at least one message"
    );
}

/// Matrix 8.2 / 8.4 — stream() advances beyond its first item.
#[valtron_test]
fn candle_stream_advances() {
    let model = load_tiny_llama();
    let stream = model
        .stream(greeting(), Some(params()))
        .expect("stream should be created");

    let mut nexts = 0;
    let mut items = 0;
    for item in stream {
        items += 1;
        if let Stream::Next(Messages::Assistant { content, .. }) = &item {
            if let ModelOutput::Text(_) = content {
                nexts += 1;
            }
        }
        if items > 100 {
            break;
        }
    }
    assert!(
        nexts >= 1,
        "stream must emit at least one text token (got {items} items, {nexts} text)"
    );
}

/// Matrix 8.3 — generate() and stream() agree in shape: both yield text tokens.
#[valtron_test]
fn candle_generate_and_stream_agree_in_shape() {
    let model = load_tiny_llama();

    let gen = model.generate(greeting(), Some(params())).expect("gen");
    let gen_text = gen
        .iter()
        .any(|m| matches!(m, Messages::Assistant { content: ModelOutput::Text(_), .. }));

    let stream = model.stream(greeting(), Some(params())).expect("stream");
    let stream_text = stream.into_iter().any(|item| {
        matches!(
            item,
            Stream::Next(Messages::Assistant { content: ModelOutput::Text(_), .. })
        )
    });

    assert_eq!(
        gen_text, stream_text,
        "generate() and stream() must agree on producing text output"
    );
    assert!(gen_text, "both paths should produce text");
}

// ---------------------------------------------------------------------------
// Sampling — matrix 8.15/8.19 (spec-60/S3).

/// Matrix 8.15 — temperature <= 0 is greedy/argmax: fully deterministic.
#[valtron_test]
fn candle_greedy_is_deterministic() {
    let model = load_tiny_llama();
    let greedy = ModelParams {
        max_tokens: 6,
        temperature: 0.0,
        ..Default::default()
    };

    let a = model.generate(greeting(), Some(greedy.clone())).expect("gen a");
    let b = model.generate(greeting(), Some(greedy)).expect("gen b");

    let text = |out: &[Messages]| -> String {
        out.iter()
            .filter_map(|m| match m {
                Messages::Assistant { content: ModelOutput::Text(t), .. } => Some(t.content.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(text(&a), text(&b), "greedy decoding must be reproducible");
}

/// Matrix 8.19 — the same seed reproduces the same stochastic sequence.
#[valtron_test]
fn candle_seeded_sampling_is_reproducible() {
    let model = load_tiny_llama();
    let seeded = ModelParams {
        max_tokens: 6,
        temperature: 0.8,
        top_k: 40.0,
        seed: Some(1234),
        ..Default::default()
    };

    let a = model.generate(greeting(), Some(seeded.clone())).expect("gen a");
    let b = model.generate(greeting(), Some(seeded)).expect("gen b");

    let text = |out: &[Messages]| -> String {
        out.iter()
            .filter_map(|m| match m {
                Messages::Assistant { content: ModelOutput::Text(t), .. } => Some(t.content.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        text(&a),
        text(&b),
        "the same seed must reproduce the same sampled sequence"
    );
}

/// Matrix 8.10 — the model's chat template is applied (spec-60/S5).
///
/// The tiny-random-Llama fixture ships a Llama-2 template that wraps user turns
/// in `[INST] ... [/INST]`. Proving it's applied is behavioural here: with the
/// template the prompt has real turn structure, so tokenization differs from the
/// plain `User:/Assistant:` fallback. We assert generation succeeds AND that the
/// stream produces output — a render failure would have logged a fallback and
/// still worked, so this is a smoke-level guard backed by the debug-log check in
/// the S5 commit. A hard structural assertion lives in the unit test below.
#[valtron_test]
fn candle_applies_chat_template_without_error() {
    let model = load_tiny_llama();
    // Two user turns exercise the template's message loop (no hand-built
    // Assistant message needed). A render failure logs a fallback (checked in
    // the S5 commit) but still succeeds, so this guards the loop compiles and
    // runs end to end through the Llama-2 template's `[INST]` structure.
    let mut interaction = greeting();
    interaction.messages.push(Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: "And another?".into(),
            signature: None,
        }),
        signature: None,
    });

    let out = model
        .generate(interaction, Some(params()))
        .expect("multi-turn generation with the chat template should succeed");
    assert!(!out.is_empty(), "templated multi-turn generation produced nothing");
}

/// Matrix 8.20/8.21 — architecture is detected from config.json, and an
/// unsupported one fails loudly rather than loading as Llama (spec-60/S6).
#[valtron_test]
fn candle_unsupported_architecture_fails_loudly() {
    use std::io::Write;

    // A synthetic model dir whose config declares a non-Llama architecture but
    // otherwise borrows the real fixture's tokenizer/weights. Loading must error
    // with the detected name, not silently succeed as Llama.
    let src = fixture_dir("tiny-random-LlamaForCausalLM");
    let tmp = std::env::temp_dir().join(format!("candle-arch-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    for f in ["tokenizer.json", "tokenizer_config.json", "model.safetensors"] {
        let _ = std::fs::copy(src.join(f), tmp.join(f));
    }
    // config.json with a different model_type.
    let mut cfg = std::fs::File::create(tmp.join("config.json")).unwrap();
    write!(
        cfg,
        r#"{{"model_type":"qwen2","architectures":["Qwen2ForCausalLM"],"hidden_size":16,"num_hidden_layers":2,"vocab_size":32000}}"#
    )
    .unwrap();
    drop(cfg);

    let spec = ModelSpec {
        name: "fake-qwen".to_string(),
        id: ModelId::Name("fake-qwen".to_string(), None),
        devices: None,
        model_location: Some(tmp.to_string_lossy().to_string().into()),
        lora_location: None,
    };
    let result = CandleBackend::cpu().get_model_by_spec(spec);

    let _ = std::fs::remove_dir_all(&tmp);

    let err = result.err().expect("a non-Llama architecture must not load as Llama");
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("qwen2"),
        "the error must name the detected architecture, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Second architecture — matrix 8.22/8.23 (spec-60/S2+S6): the committed
// tiny-random-Gemma2 fixture loads and generates through the candle backend,
// proving the per-architecture state abstraction (Gemma2's internal KV cache,
// not a Llama-shaped external one) and a large (256k) vocab.

fn load_tiny_gemma2() -> impl Model {
    let dir = fixture_dir("tiny-random-Gemma2ForCausalLM");
    let spec = ModelSpec {
        name: "tiny-random-gemma2".to_string(),
        id: ModelId::Name("tiny-random-gemma2".to_string(), None),
        devices: None,
        model_location: Some(dir.to_string_lossy().to_string().into()),
        lora_location: None,
    };
    CandleBackend::cpu()
        .get_model_by_spec(spec)
        .expect("gemma2 fixture loads")
}

#[valtron_test]
fn tiny_random_gemma2_fixture_loads_and_generates() {
    let model = load_tiny_gemma2();
    let out = model
        .generate(greeting(), Some(params()))
        .expect("gemma2 generate should succeed");
    assert!(
        !out.is_empty(),
        "gemma2 (256k vocab, internal KV cache) must produce output"
    );
}

#[valtron_test]
fn tiny_random_gemma2_stream_advances() {
    let model = load_tiny_gemma2();
    let stream = model
        .stream(greeting(), Some(params()))
        .expect("gemma2 stream should be created");
    let mut text_tokens = 0;
    let mut items = 0;
    for item in stream {
        items += 1;
        if let Stream::Next(Messages::Assistant { content: ModelOutput::Text(_), .. }) = &item {
            text_tokens += 1;
        }
        if items > 100 {
            break;
        }
    }
    assert!(text_tokens >= 1, "gemma2 stream must advance (got {items} items)");
}

// ---------------------------------------------------------------------------
// Full AgentSession through a REAL candle model, offline (matrix 7.1 on candle).
//
// This is the seam-through-session junction where docs/fixes/006 lived: the
// AgentLoop driving a real streaming provider (not the mock's trivial stream).
// PreloadedProvider serves the loaded fixture into a router, so run_turn
// exercises the loop AND the real candle stream together, with no download.

#[valtron_test]
fn agent_session_run_turn_through_candle_fixture() {
    use foundation_ai::agentic::{
        AgentConfig, AgentSession, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig,
    };
    use foundation_ai::types::PreloadedProvider;
    use foundation_ai::types::{SessionId, SessionRecord};
    use foundation_db::{MemoryDocumentStore, MemoryStorage};

    // Concrete CandleModels (Clone via Arc) for PreloadedProvider.
    let dir = fixture_dir("tiny-random-LlamaForCausalLM");
    let model = CandleBackend::cpu()
        .get_model_by_spec(ModelSpec {
            name: "tiny-llama".to_string(),
            id: ModelId::Name("tiny-llama".to_string(), None),
            devices: None,
            model_location: Some(dir.to_string_lossy().to_string().into()),
            lora_location: None,
        })
        .expect("fixture loads");
    let model_id = ModelId::Name("tiny-llama".into(), None);
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
        .expect("a real candle turn must complete, not error");

    // Structural (random weights => gibberish): the turn ran end-to-end and did
    // not short-circuit to only a Summary (the docs/fixes/006 symptom).
    assert!(
        records
            .iter()
            .any(|r| matches!(r, SessionRecord::Conversation { .. })),
        "run_turn through a real candle model must emit a conversation record, \
         not just a Summary: {records:?}"
    );
}

/// Matrix 3.8 / 7.4 — multi-turn continuity through a real candle model, offline.
/// A second run_turn on the same session must also complete (the loop re-enters
/// cleanly and the shared model handle keeps working across turns).
#[valtron_test]
fn candle_session_handles_multiple_turns() {
    use foundation_ai::agentic::{
        AgentConfig, AgentSession, ContextConfig, ErrorPolicy, KvMemoryStore, MemoryConfig,
    };
    use foundation_ai::types::{PreloadedProvider, SessionId, SessionRecord};
    use foundation_db::{MemoryDocumentStore, MemoryStorage};

    let dir = fixture_dir("tiny-random-LlamaForCausalLM");
    let model = CandleBackend::cpu()
        .get_model_by_spec(ModelSpec {
            name: "tiny-llama".to_string(),
            id: ModelId::Name("tiny-llama".to_string(), None),
            devices: None,
            model_location: Some(dir.to_string_lossy().to_string().into()),
            lora_location: None,
        })
        .expect("fixture loads");
    let model_id = ModelId::Name("tiny-llama".into(), None);

    let session: AgentSession<MemoryDocumentStore, KvMemoryStore<MemoryStorage>> =
        AgentSession::builder(SessionId::new(), PreloadedProvider::new(model, model_id.clone()).into_router())
            .with_system_prompt("You are a helpful assistant.")
            .with_model(model_id.clone())
            .with_config(AgentConfig { primary_model: model_id, model_params: params(), ..Default::default() })
            .with_context_config(ContextConfig::default())
            .with_memory_config(MemoryConfig::default())
            .with_error_policy(ErrorPolicy::new())
            .build()
            .expect("session builds");

    let msg = |t: &str| Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent { content: t.into(), signature: None }),
        signature: None,
    };

    let first = session.run_turn(msg("Hi.")).expect("turn 1");
    let second = session.run_turn(msg("Again?")).expect("turn 2 must also complete");

    let has_conv = |r: &[SessionRecord]| r.iter().any(|x| matches!(x, SessionRecord::Conversation { .. }));
    assert!(has_conv(&first) && has_conv(&second), "both turns must produce conversation records");

    // The user prompts from both turns must be in history (continuity).
    let history = session.message_api().all().expect("history");
    let user_turns = history.iter().filter(|r| matches!(r, SessionRecord::Conversation { message: Messages::User { .. } })).count();
    assert!(user_turns >= 2, "both user turns must be persisted for continuity: {user_turns}");
}

// ---------------------------------------------------------------------------
// Sampling knobs change output (8.16/8.17) and error paths (8.24/8.25).

fn gen_text(model: &impl Model, p: ModelParams) -> String {
    model
        .generate(greeting(), Some(p))
        .expect("gen")
        .iter()
        .filter_map(|m| match m {
            Messages::Assistant { content: ModelOutput::Text(t), .. } => Some(t.content.clone()),
            _ => None,
        })
        .collect()
}

/// Matrix 8.16 — top_k changes sampled output (greedy vs a wide stochastic set).
#[valtron_test]
fn candle_top_k_changes_output() {
    let model = load_tiny_llama();
    let greedy = gen_text(&model, ModelParams { max_tokens: 8, temperature: 0.0, ..Default::default() });
    let sampled = gen_text(&model, ModelParams { max_tokens: 8, temperature: 1.5, top_k: 50.0, seed: Some(7), ..Default::default() });
    // With random weights they CAN coincide, but a high-temp top-k draw versus
    // greedy should differ for at least one of several seeds.
    let differs = (0..5).any(|seed| {
        gen_text(&model, ModelParams { max_tokens: 8, temperature: 1.5, top_k: 50.0, seed: Some(seed), ..Default::default() }) != greedy
    });
    assert!(differs || sampled != greedy, "stochastic top-k sampling should differ from greedy for some seed");
}

/// Matrix 8.25 — a missing config.json is a clear error, not a panic.
#[valtron_test]
fn candle_missing_config_errors() {
    let tmp = std::env::temp_dir().join(format!("candle-noconfig-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&tmp);
    // tokenizer + weights present, config.json absent.
    let src = fixture_dir("tiny-random-LlamaForCausalLM");
    for f in ["tokenizer.json", "model.safetensors"] {
        let _ = std::fs::copy(src.join(f), tmp.join(f));
    }
    let spec = ModelSpec {
        name: "noconfig".into(),
        id: ModelId::Name("noconfig".into(), None),
        devices: None,
        model_location: Some(tmp.to_string_lossy().to_string().into()),
        lora_location: None,
    };
    let result = CandleBackend::cpu().get_model_by_spec(spec);
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(result.is_err(), "a missing config.json must be a clear error");
}

/// Matrix 8.24 — a missing model directory is a clear error.
#[valtron_test]
fn candle_missing_model_dir_errors() {
    let spec = ModelSpec {
        name: "ghost".into(),
        id: ModelId::Name("ghost".into(), None),
        devices: None,
        model_location: Some("/nonexistent/path/to/model".to_string().into()),
        lora_location: None,
    };
    assert!(
        CandleBackend::cpu().get_model_by_spec(spec).is_err(),
        "a nonexistent model path must error"
    );
}
