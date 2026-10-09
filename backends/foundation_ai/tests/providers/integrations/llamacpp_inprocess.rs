//! In-process llama.cpp coverage against a LOCAL GGUF — the code paths where
//! docs/fixes/006 and 007 lived.
//!
//! WHY: the other llama.cpp integration tests either drive an external
//! llama-server over HTTP (which never touches `backends/llamacpp.rs`) or load
//! a multi-GB Gemma-4 in-process. Neither is a routine, in-process exercise of
//! the LlamaCppStream / generate_text / BOS / model-cache code fixed this
//! session. This loads the small local qwen GGUF directly and drives both APIs.
//!
//! Gated `live-model-tests` because the qwen GGUF is downloaded (mise), not a
//! committed fixture — our committed tiny fixtures are safetensors (candle only).
//! Self-skips when the model file is absent, so it never fails a checkout that
//! has not pulled it.

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{
    MessageRole, Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelSpec,
    TextContent, ToolShed, UserModelContent,
};
use foundation_core::valtron::{valtron_test, Stream};

/// Local qwen GGUF path, honouring the same env the harness uses.
fn qwen_path() -> Option<std::path::PathBuf> {
    let p = std::env::var("LLAMA_TEST_MODEL_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../artefacts/models/qwen2.5-0.5b-instruct-q4_k_m.gguf")
        });
    p.exists().then_some(p)
}

fn load_qwen() -> Option<impl Model> {
    let path = qwen_path()?;
    let config = LlamaBackendConfig::builder()
        .n_gpu_layers(0)
        .context_length(1024)
        .n_threads(4)
        .build();
    let spec = ModelSpec {
        name: "qwen2.5-0.5b-instruct".to_string(),
        id: ModelId::Name("qwen".to_string(), None),
        devices: None,
        model_location: Some(path.to_string_lossy().to_string().into()),
        lora_location: None,
    };
    Some(
        LlamaBackends::LLamaCPU
            .load_model(spec, &config)
            .expect("local qwen GGUF should load in-process"),
    )
}

fn greeting() -> ModelInteraction {
    ModelInteraction {
        system_prompt: Some("You are a helpful assistant. Be concise.".to_string()),
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Reply with one short greeting.".to_string(),
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
        max_tokens: 16,
        temperature: 0.0, // greedy -> deterministic, coherent
        ..Default::default()
    }
}

fn text_of(msgs: &[Messages]) -> String {
    msgs.iter()
        .filter_map(|m| match m {
            Messages::Assistant {
                content: ModelOutput::Text(t),
                ..
            } => Some(t.content.clone()),
            _ => None,
        })
        .collect()
}

/// generate() produces coherent multi-token output — the docs/fixes/007
/// regression guard: a degenerate single "." would fail the length check.
#[valtron_test]
fn qwen_generate_is_coherent() {
    let Some(model) = load_qwen() else {
        eprintln!("[skip] local qwen GGUF not present");
        return;
    };
    let out = model.generate(greeting(), Some(params())).expect("generate");
    let text = text_of(&out);
    assert!(
        text.trim().chars().filter(|c| c.is_alphanumeric()).count() >= 2,
        "expected a real word, got degenerate output: {text:?}"
    );
    println!("qwen generate: {text:?}");
}

/// stream() advances past its first token (docs/fixes/006 regression guard: the
/// stream that died on poll 2 emitted exactly one item).
#[valtron_test]
fn qwen_stream_advances() {
    let Some(model) = load_qwen() else {
        eprintln!("[skip] local qwen GGUF not present");
        return;
    };
    let stream = model.stream(greeting(), Some(params())).expect("stream");
    let mut text_tokens = 0;
    for item in stream {
        if let Stream::Next(Messages::Assistant {
            content: ModelOutput::Text(_),
            ..
        }) = item
        {
            text_tokens += 1;
        }
        if text_tokens >= 2 {
            break; // proved it advances past the first token
        }
    }
    assert!(
        text_tokens >= 2,
        "stream must advance past its first token (got {text_tokens})"
    );
}

/// generate() and stream() agree the model produces text for one interaction.
#[valtron_test]
fn qwen_generate_and_stream_agree() {
    let Some(model) = load_qwen() else {
        eprintln!("[skip] local qwen GGUF not present");
        return;
    };
    let gen_has_text = !text_of(&model.generate(greeting(), Some(params())).expect("gen")).is_empty();
    let stream = model.stream(greeting(), Some(params())).expect("stream");
    let stream_has_text = stream.into_iter().any(|i| {
        matches!(
            i,
            Stream::Next(Messages::Assistant {
                content: ModelOutput::Text(_),
                ..
            })
        )
    });
    assert_eq!(gen_has_text, stream_has_text, "both APIs must agree on text");
    assert!(gen_has_text);
}
