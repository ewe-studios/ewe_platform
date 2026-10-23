//! T1 — shareability spike: one loaded model serves many sessions at once.
//!
//! WHY: the whole agent-rpc memory model ("load once per machine, many threaded
//! sessions") rests on `foundation_ai`'s loaded backend being `Send + Sync` and
//! safe to drive from several generations at the same time. This file is the
//! evidence, run against the committed tiny-random Candle fixtures (offline,
//! milliseconds per token).
//!
//! WHAT: a generation's output must not depend on what other generations on the
//! same loaded model are doing. Greedy decoding (temperature 0) makes every
//! generation deterministic, so "the same output as when run alone" is an exact
//! check for cross-talk through shared inference state (e.g. a KV cache).

use foundation_ai::backends::candle::{CandleBackend, CandleModels};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams,
    ModelProvider, ModelSpec, TextContent, ToolDeclarations, UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};

fn fixture_dir(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join(name)
}

fn load(fixture: &str) -> CandleModels {
    let spec = ModelSpec {
        name: fixture.to_string(),
        id: ModelId::Name(fixture.to_string(), None),
        devices: None,
        model_location: Some(fixture_dir(fixture).to_string_lossy().to_string().into()),
        lora_location: None,
    };
    CandleBackend::cpu()
        .get_model_by_spec(spec)
        .expect("tiny fixture loads offline")
}

fn ask(text: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: text.to_string(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolDeclarations::default(),
        chat_template: None,
        tool_choice: None,
    }
}

fn greedy(max_tokens: usize) -> ModelParams {
    ModelParams {
        max_tokens,
        temperature: 0.0,
        ..Default::default()
    }
}

fn token_text(item: &Stream<Messages, foundation_ai::types::ModelState>) -> Option<String> {
    match item {
        Stream::Next(Messages::Assistant {
            content: ModelOutput::Text(t),
            ..
        }) => Some(t.content.clone()),
        _ => None,
    }
}

/// Drain a stream on its own and return the tokens it produced.
fn run_alone(model: &CandleModels, prompt: &str) -> Vec<String> {
    model
        .stream(ask(prompt), Some(greedy(12)))
        .expect("stream starts")
        .filter_map(|item| token_text(&item))
        .collect()
}

/// Two streams on ONE loaded model, polled alternately (A, B, A, B, ...):
/// each must produce exactly what it produces alone. Gemma2 keeps its KV cache
/// inside the model, so any shared per-generation state shows up here as
/// diverging tokens.
#[valtron_test]
fn interleaved_streams_on_one_gemma2_model_do_not_cross_talk() {
    let model = load("tiny-random-Gemma2ForCausalLM");
    let alone_a = run_alone(&model, "Tell me about the sea.");
    let alone_b = run_alone(&model, "List three prime numbers, please, quickly.");
    assert!(
        !alone_a.is_empty() && !alone_b.is_empty(),
        "fixture must emit tokens"
    );

    let mut a = model
        .stream(ask("Tell me about the sea."), Some(greedy(12)))
        .expect("stream a");
    let mut b = model
        .stream(
            ask("List three prime numbers, please, quickly."),
            Some(greedy(12)),
        )
        .expect("stream b");

    let (mut got_a, mut got_b) = (Vec::new(), Vec::new());
    let (mut done_a, mut done_b) = (false, false);
    while !(done_a && done_b) {
        if !done_a {
            match a.next() {
                Some(item) => got_a.extend(token_text(&item)),
                None => done_a = true,
            }
        }
        if !done_b {
            match b.next() {
                Some(item) => got_b.extend(token_text(&item)),
                None => done_b = true,
            }
        }
    }

    assert_eq!(got_a, alone_a, "stream A diverged when interleaved with B");
    assert_eq!(got_b, alone_b, "stream B diverged when interleaved with A");
}

/// The same check on the Llama fixture, whose KV cache is external (and is
/// now actually used: tokens after the first attend to the whole context).
#[valtron_test]
fn interleaved_streams_on_one_llama_model_do_not_cross_talk() {
    let model = load("tiny-random-LlamaForCausalLM");
    let alone_a = run_alone(&model, "Tell me about the sea.");
    let alone_b = run_alone(&model, "List three prime numbers, please, quickly.");

    let mut a = model
        .stream(ask("Tell me about the sea."), Some(greedy(12)))
        .expect("stream a");
    let mut b = model
        .stream(
            ask("List three prime numbers, please, quickly."),
            Some(greedy(12)),
        )
        .expect("stream b");
    let (mut got_a, mut got_b) = (Vec::new(), Vec::new());
    loop {
        let na = a.next();
        let nb = b.next();
        if na.is_none() && nb.is_none() {
            break;
        }
        got_a.extend(na.as_ref().and_then(token_text));
        got_b.extend(nb.as_ref().and_then(token_text));
    }
    assert_eq!(got_a, alone_a, "stream A diverged when interleaved with B");
    assert_eq!(got_b, alone_b, "stream B diverged when interleaved with A");
}

fn session_on(model: &CandleModels) -> foundation_ai::agentic::AgentSession {
    use foundation_ai::agentic::{AgentConfig, AgentSession};
    use foundation_ai::types::PreloadedProvider;

    let id = ModelId::Name("shared-tiny-llama".into(), None);
    AgentSession::builder(PreloadedProvider::new(model.clone(), id.clone()).into_router())
        .with_system_prompt("You are a helpful assistant.")
        .with_model(id)
        .with_config(AgentConfig {
            model_params: greedy(8),
            ..Default::default()
        })
        .build()
        .expect("session builds")
}

fn user(text: &str) -> Messages {
    Messages::User {
        id: foundation_compact::ids::new_scru128(),
        role: MessageRole::User,
        content: UserModelContent::Text(TextContent {
            content: text.to_string(),
            signature: None,
        }),
        signature: None,
    }
}

/// Two agent sessions, one loaded model, two OS threads at once: both turns
/// complete with the answer each gives alone, and both sessions run on the
/// same weights (no per-session load).
#[valtron_test(threads = 4)]
fn two_sessions_share_one_model_concurrently() {
    let model = load("tiny-random-LlamaForCausalLM");

    let answer = |s: &foundation_ai::agentic::AgentSession, q: &str| {
        s.ask(user(q)).expect("turn starts").text().to_string()
    };
    let alone_a = answer(&session_on(&model), "Hi there.");
    let alone_b = answer(&session_on(&model), "What is two plus two?");

    let (sa, sb) = (session_on(&model), session_on(&model));
    let ta = std::thread::spawn(move || sa.ask(user("Hi there.")).map(|a| a.text().to_string()));
    let tb = std::thread::spawn(move || {
        sb.ask(user("What is two plus two?"))
            .map(|a| a.text().to_string())
    });
    let got_a = ta.join().expect("thread a").expect("concurrent turn a");
    let got_b = tb.join().expect("thread b").expect("concurrent turn b");

    assert_eq!(
        got_a, alone_a,
        "session A's answer changed under concurrency"
    );
    assert_eq!(
        got_b, alone_b,
        "session B's answer changed under concurrency"
    );

    let clone = model.clone();
    assert!(
        clone.shares_weights_with(&model),
        "clones must share one load"
    );
    let other = load("tiny-random-LlamaForCausalLM");
    assert!(
        !other.shares_weights_with(&model),
        "a second load is a different model"
    );
}

/// The bound agent-rpc's `ModelHandle` relies on.
#[test]
fn candle_models_is_send_sync_static() {
    fn assert_bounds<T: Send + Sync + 'static>() {}
    assert_bounds::<CandleModels>();
}
