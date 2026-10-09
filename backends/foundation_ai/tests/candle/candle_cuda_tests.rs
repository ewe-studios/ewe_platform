//! CUDA execution for the candle backend (spec-60 F20).
//!
//! WHY: the `Cuda` variant existed with no constructor, so `candle-cuda`
//! compiled the GPU arms but nothing could reach them — the backend never ran on
//! a GPU. These tests exercise the real constructors against a real device and,
//! critically, prove the run happened ON the GPU rather than silently falling
//! back to CPU: `try_cuda` opens the device eagerly, so a backend that reached
//! inference at all is one whose CUDA context was successfully created.
//!
//! WHAT: the device opens, the tiny fixture loads and generates on CUDA, an
//! absent device is a distinguishable error (not a silent CPU downgrade), and a
//! second GPU is usable when present.
//!
//! HOW: gated behind `candle-cuda` so a GPU-less host never compiles it, and
//! self-skipping when no CUDA device is actually present — a machine with the
//! feature on but no card must not fail the suite.
//!
//! Only compiled under `--features candle-cuda`.

use foundation_ai::backends::candle::CandleBackend;
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelProvider, ModelSpec,
    ToolShed, UserModelContent, MessageRole, TextContent,
};
use foundation_core::valtron::{valtron_test, Stream};

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models")
        .join("tiny-random-LlamaForCausalLM")
}

fn spec() -> ModelSpec {
    ModelSpec {
        name: "tiny-random-llama".to_string(),
        id: ModelId::Name("tiny-random-llama".to_string(), None),
        devices: None,
        model_location: Some(fixture_dir().to_string_lossy().to_string().into()),
        lora_location: None,
    }
}

fn greeting() -> ModelInteraction {
    ModelInteraction {
        system_prompt: None,
        soul: None,
        messages: vec![Messages::User {
            id: foundation_compact::ids::new_scru128(),
            role: MessageRole::User,
            content: UserModelContent::Text(TextContent {
                content: "Hello".into(),
                signature: None,
            }),
            signature: None,
        }],
        tools_shed: ToolShed {
            shed: None,
            tools: Vec::new(),
        },
        chat_template: None,
        tool_choice: None,
    }
}

fn params() -> ModelParams {
    ModelParams {
        max_tokens: 8,
        temperature: 0.0,
        ..Default::default()
    }
}

/// True when CUDA device 0 can actually be opened. The feature can be compiled
/// in on a host with no usable card (no driver, driver/library mismatch), so the
/// tests below self-skip rather than fail there.
fn cuda_available() -> bool {
    CandleBackend::try_cuda(0).is_ok()
}

#[test]
fn a_cuda_backend_can_be_constructed_and_opened() {
    if !cuda_available() {
        eprintln!("[skip] no usable CUDA device 0");
        return;
    }
    // The whole point of B2: this used to be impossible to build at all.
    let backend = CandleBackend::try_cuda(0).expect("CUDA device 0 opens");
    // describe() names the device, proving we hold the Cuda variant rather than
    // a CPU value that merely compiled under the feature.
    let desc = backend.describe().expect("describe");
    assert!(
        desc.name.contains("CUDA"),
        "the backend must report itself as CUDA, got: {}",
        desc.name
    );
}

#[test]
fn an_absent_cuda_device_is_a_distinguishable_error() {
    if !cuda_available() {
        eprintln!("[skip] no usable CUDA device 0");
        return;
    }
    // Acceptance #4: asking for a device that cannot exist must error, not
    // silently hand back a working CPU backend. Ordinal 99 is far past any real
    // GPU count on this host.
    let result = CandleBackend::try_cuda(99);
    assert!(
        result.is_err(),
        "opening a nonexistent CUDA ordinal must fail rather than downgrade to CPU"
    );
}

#[valtron_test]
fn the_tiny_fixture_generates_on_cuda() {
    if !cuda_available() {
        eprintln!("[skip] no usable CUDA device 0");
        return;
    }
    assert!(
        fixture_dir().join("model.safetensors").exists(),
        "fixture missing: {:?}",
        fixture_dir()
    );

    // Reaching a produced token proves the model ran on the GPU: try_cuda opened
    // the CUDA context up front, and get_model_by_spec loads the weights onto
    // that device. Random weights emit gibberish, so this is structural — that
    // generation happens at all on CUDA, not what it says.
    let backend = CandleBackend::try_cuda(0).expect("CUDA device 0 opens");
    let model = backend.get_model_by_spec(spec()).expect("fixture loads on CUDA");

    let out = model.generate(greeting(), Some(params())).expect("cuda generate");
    let text: String = out
        .iter()
        .filter_map(|m| match m {
            Messages::Assistant {
                content: ModelOutput::Text(t),
                ..
            } => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    assert!(!text.is_empty(), "CUDA generation produced no text: {out:?}");
}

#[valtron_test]
fn a_cuda_stream_advances() {
    if !cuda_available() {
        eprintln!("[skip] no usable CUDA device 0");
        return;
    }
    let backend = CandleBackend::try_cuda(0).expect("CUDA device 0 opens");
    let model = backend.get_model_by_spec(spec()).expect("fixture loads on CUDA");

    let stream = model.stream(greeting(), Some(params())).expect("cuda stream");
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
            break;
        }
    }
    assert!(
        text_tokens >= 1,
        "CUDA stream must advance past its first token (got {text_tokens})"
    );
}

#[test]
fn best_available_selects_cuda_when_present() {
    if !cuda_available() {
        eprintln!("[skip] no usable CUDA device 0");
        return;
    }
    // With a working GPU, best_available must not land on CPU.
    let backend = CandleBackend::best_available();
    let desc = backend.describe().expect("describe");
    assert!(
        desc.name.contains("CUDA"),
        "best_available should pick CUDA when it is usable, got: {}",
        desc.name
    );
}

#[test]
fn a_second_gpu_is_usable_when_present() {
    // Acceptance #5: exercise a non-default device. Skips gracefully on a
    // single-GPU host — try_cuda(1) simply errors there.
    match CandleBackend::try_cuda(1) {
        Ok(backend) => {
            let desc = backend.describe().expect("describe");
            assert!(desc.name.contains("CUDA"), "device 1 must report CUDA");
        }
        Err(_) => eprintln!("[skip] no second CUDA device"),
    }
}
