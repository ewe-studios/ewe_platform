//! T1 for GGUF: one llama.cpp model, loaded once, serves many sessions at once.
//!
//! The candle evidence lives in `model_share_tests.rs`; this is the same check
//! on the llama.cpp backend, run against the committed tiny-random GGUF
//! fixture on the CPU build of llama.cpp (offline, fast).
//!
//! WHAT: greedy decoding (temperature 0) makes each generation deterministic, so
//! "the same tokens as when run alone" is an exact check for cross-talk through
//! shared inference state. Each llama.cpp generation builds its own context (and
//! KV cache) from the shared weights; these tests pin that.

use foundation_agent_rpc::{GgufDevice, ModelHandle, ModelSource};
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends, LlamaModels};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelSpec,
    TextContent, ToolDeclarations, UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};

fn fixture() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models/tiny-random-LlamaForCausalLM/tiny-llama-f16.gguf")
}

fn load() -> LlamaModels {
    let spec = ModelSpec {
        name: "tiny-llama-gguf".to_string(),
        id: ModelId::Name("tiny-llama-gguf".to_string(), None),
        devices: None,
        model_location: Some(fixture()),
        lora_location: None,
    };
    LlamaBackends::LLamaCPU
        .load_model(spec, &LlamaBackendConfig::default())
        .expect("tiny GGUF fixture loads on the CPU build")
}

fn ask(text: &str) -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        soul: None,
        messages: vec![user(text)],
        tools_shed: ToolDeclarations::default(),
        chat_template: None,
        tool_choice: None,
    }
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

fn run_alone(model: &LlamaModels, prompt: &str) -> Vec<String> {
    model
        .stream(ask(prompt), Some(greedy(12)))
        .expect("stream starts")
        .filter_map(|item| token_text(&item))
        .collect()
}

/// `ModelSource::Gguf` loads through `ModelHandle::load`, and clones of the
/// handle share the one load.
#[valtron_test]
fn gguf_source_loads_once_into_a_shared_handle() {
    let source = ModelSource::gguf_on("tiny-llama-gguf", fixture(), GgufDevice::Cpu);
    assert_eq!(source, ModelSource::gguf("tiny-llama-gguf", fixture()));
    let handle = ModelHandle::load(&source).expect("loads");
    assert_eq!(handle.name(), "tiny-llama-gguf");
    assert!(handle.same_model(&handle.clone()));
}

/// A missing file is a load error, not a panic.
#[valtron_test]
fn missing_gguf_is_a_model_load_error() {
    let err = ModelHandle::load(&ModelSource::gguf("nope", "/no/such/model.gguf"))
        .expect_err("missing file must not load");
    assert!(
        err.to_string().contains("nope"),
        "error names the model: {err}"
    );
}

/// Greedy decoding on llama.cpp is deterministic run to run — the premise the
/// interleaving check below relies on.
#[valtron_test]
fn greedy_gguf_generation_is_deterministic() {
    let model = load();
    let first = run_alone(&model, "Tell me about the sea.");
    assert!(!first.is_empty(), "fixture must emit tokens");
    assert_eq!(first, run_alone(&model, "Tell me about the sea."));
}

/// Two streams on ONE loaded GGUF model, polled alternately: each produces
/// exactly what it produces alone, so they share no KV/context state.
#[valtron_test]
fn interleaved_streams_on_one_gguf_model_do_not_cross_talk() {
    let model = load();
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

/// Two generations on two OS threads at once on the same loaded model: both
/// produce their solo output.
#[valtron_test(threads = 4)]
fn concurrent_gguf_generations_on_two_threads_match_solo_runs() {
    let model = load();
    let alone_a = run_alone(&model, "Hi there.");
    let alone_b = run_alone(&model, "What is two plus two?");

    let (ma, mb) = (model.clone(), model.clone());
    let ta = std::thread::spawn(move || run_alone(&ma, "Hi there."));
    let tb = std::thread::spawn(move || run_alone(&mb, "What is two plus two?"));
    assert_eq!(ta.join().expect("thread a"), alone_a);
    assert_eq!(tb.join().expect("thread b"), alone_b);
}
