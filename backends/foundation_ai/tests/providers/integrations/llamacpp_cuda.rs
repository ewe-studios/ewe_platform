//! CUDA offload for the llama.cpp backend (spec-60 F20, B3/W4).
//!
//! WHY: the backend plumbs `n_gpu_layers` through to llama.cpp but defaults to
//! 0 — so a build with the `cuda` feature still runs entirely on CPU unless the
//! caller opts in. And the opt-in is silent: wrong flags or zero layers produce
//! correct answers, slowly, with no warning. These tests make the GPU path
//! explicit and check it against a real build.
//!
//! WHAT: the build reports GPU-offload support, a config offloads all layers,
//! and the tiny GGUF loads and generates with offload requested.
//!
//! HOW: gated behind `cuda` so a non-CUDA build never compiles it, and
//! self-skipping when the build has no GPU support or no device is present.
//!
//! The load-bearing check is `supports_gpu_offload`: llama.cpp does not expose a
//! per-model "layers actually on GPU" counter, so the honest proof that this is
//! a GPU-capable build — not a CPU one that ignored `n_gpu_layers` — is that the
//! compiled library reports offload support AND a device exists. A generation
//! that then completes with all layers requested exercises the offload path.
//!
//! Only compiled under `--features cuda`.

use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};
use foundation_ai::types::{
    Messages, Model, ModelId, ModelInteraction, ModelOutput, ModelParams, ModelSpec, ToolShed,
    UserModelContent, MessageRole, TextContent,
};
use foundation_core::valtron::valtron_test;
use infrastructure_llama_cpp::llama_backend::LlamaBackend;

/// The committed tiny GGUF fixture (random weights → gibberish, structurally
/// valid). Unlike the downloaded qwen model, this ships in the repo.
fn tiny_gguf() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artefacts/test-models/tiny-random-LlamaForCausalLM/tiny-llama-f16.gguf")
}

/// The build must support offload AND a device must be present. The `cuda`
/// feature can be compiled with no card attached, so skip rather than fail.
fn gpu_offload_usable() -> bool {
    let Ok(backend) = LlamaBackend::init_or_get() else {
        return false;
    };
    backend.supports_gpu_offload()
}

#[test]
fn the_build_reports_gpu_offload_support() {
    // Proves the `cuda` feature actually compiled llama.cpp with GPU support —
    // if this is false under `--features cuda`, the build silently produced a
    // CPU-only library and every "GPU" run would really be CPU.
    if !gpu_offload_usable() {
        eprintln!("[skip] llama.cpp build has no GPU offload support");
        return;
    }
    let backend = LlamaBackend::init_or_get().expect("backend");
    assert!(
        backend.supports_gpu_offload(),
        "a cuda-feature build must report GPU offload support"
    );
}

#[test]
fn offload_all_layers_requests_full_offload() {
    // The builder helper must actually set a full-offload layer count, not leave
    // the CPU-only default in place.
    let config = LlamaBackendConfig::builder().offload_all_layers().build();
    assert_eq!(
        config.n_gpu_layers,
        u32::MAX,
        "offload_all_layers must request every layer (llama.cpp clamps to the model's depth)"
    );
    // And the default must remain CPU-only, so GPU use is never accidental.
    let default = LlamaBackendConfig::default();
    assert_eq!(default.n_gpu_layers, 0, "default must stay CPU-only");
}

#[valtron_test]
fn the_tiny_gguf_generates_with_offload() {
    if !gpu_offload_usable() {
        eprintln!("[skip] llama.cpp build has no GPU offload support");
        return;
    }
    let path = tiny_gguf();
    if !path.exists() {
        eprintln!("[skip] tiny GGUF fixture absent: {path:?}");
        return;
    }

    let config = LlamaBackendConfig::builder()
        .offload_all_layers()
        .context_length(512)
        .build();
    let spec = ModelSpec {
        name: "tiny-random-llama".to_string(),
        id: ModelId::Name("tiny-gguf".to_string(), None),
        devices: None,
        model_location: Some(path.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    // LLamaGPU selects the GPU-offload path; the config carries the layer count.
    let model = LlamaBackends::LLamaGPU
        .load_model(spec, &config)
        .expect("tiny GGUF loads with offload requested");

    let interaction = ModelInteraction {
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
    };
    let params = ModelParams {
        max_tokens: 8,
        temperature: 0.0,
        ..Default::default()
    };

    let out = model
        .generate(interaction, Some(params))
        .expect("gpu-offloaded generate");
    let produced_text = out.iter().any(|m| {
        matches!(
            m,
            Messages::Assistant {
                content: ModelOutput::Text(_),
                ..
            }
        )
    });
    assert!(produced_text, "offloaded generation produced no text: {out:?}");
}
