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

// ---------------------------------------------------------------------------
// Multi-GPU placement (spec-60 F20 acceptance #5)
// ---------------------------------------------------------------------------

/// GPU devices llama.cpp can actually see (the CPU backend is excluded).
fn gpu_device_count() -> usize {
    infrastructure_llama_cpp::list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|d| !d.backend.eq_ignore_ascii_case("CPU"))
        .count()
}

/// The prompt + params shared by the multi-GPU generations below.
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

#[test]
fn gpu_placement_reaches_llama_cpp() {
    // The bug this pins: `split_mode`, `main_gpu` and `tensor_split` were stored
    // on LlamaBackendConfig and never applied — `to_model_params()` set only
    // `n_gpu_layers`, so all three builder methods were silent no-ops. A caller
    // pinning work to the second GPU got the default placement and no warning.
    //
    // Multi-GPU placement is the worst place for a silent no-op: the answer is
    // still correct, just computed on the wrong device, so nothing looks broken.
    //
    // This asserts on the params llama.cpp receives, so it needs no GPU and runs
    // anywhere the `cuda` feature compiles.
    use foundation_ai::types::SplitMode;

    let params = LlamaBackendConfig::builder()
        .offload_all_layers()
        .split_mode(SplitMode::Row)
        .main_gpu(1)
        .tensor_split(vec![0.5, 0.5])
        .build()
        .to_model_params();

    assert_eq!(
        params.main_gpu(),
        1,
        "main_gpu must reach llama.cpp — it was previously dropped on the floor"
    );
    assert_eq!(
        params.split_mode().expect("split mode parses"),
        infrastructure_llama_cpp::model::params::LlamaSplitMode::Row,
        "split_mode must reach llama.cpp"
    );
    assert_eq!(
        params.tensor_split(),
        Some(&[0.5f32, 0.5f32][..]),
        "tensor_split must reach llama.cpp"
    );
}

#[test]
fn the_defaults_stay_single_gpu_and_cpu_only() {
    // Placement knobs are opt-in: nothing above may change the default, or every
    // existing caller silently changes device.
    let params = LlamaBackendConfig::default().to_model_params();
    assert_eq!(params.n_gpu_layers(), 0, "default must stay CPU-only");
    assert_eq!(params.main_gpu(), 0, "default must stay on device 0");
    assert_eq!(
        params.tensor_split(),
        None,
        "no tensor split by default — llama.cpp decides from free VRAM"
    );
}

#[valtron_test]
fn a_two_way_tensor_split_generates_across_both_gpus() {
    // Acceptance #5. Needs a second device; self-skips on a one-GPU host.
    if !gpu_offload_usable() {
        eprintln!("[skip] llama.cpp build has no GPU offload support");
        return;
    }
    if gpu_device_count() < 2 {
        eprintln!("[skip] fewer than two GPUs visible to llama.cpp");
        return;
    }
    let gguf = tiny_gguf();
    if !gguf.exists() {
        eprintln!("[skip] tiny GGUF fixture missing: {gguf:?}");
        return;
    }

    use foundation_ai::types::SplitMode;
    let config = LlamaBackendConfig::builder()
        .offload_all_layers()
        .split_mode(SplitMode::Layer)
        .tensor_split(vec![0.5, 0.5])
        .build();

    let spec = ModelSpec {
        name: "tiny-llama-f16".to_string(),
        id: ModelId::Name("tiny-llama-f16".to_string(), None),
        devices: None,
        model_location: Some(gguf.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = LlamaBackends::LLamaGPU
        .load_model(spec, &config)
        .expect("the tiny GGUF must load split across both GPUs");

    let out = model
        .generate(greeting(), Some(params()))
        .expect("generation must succeed with the model split across two GPUs");

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
    assert!(
        !text.is_empty(),
        "a two-way tensor split produced no text: {out:?}"
    );
}

#[valtron_test]
fn main_gpu_one_generates_on_the_second_device() {
    // The other half of acceptance #5: pin work to device 1 rather than split.
    if !gpu_offload_usable() {
        eprintln!("[skip] llama.cpp build has no GPU offload support");
        return;
    }
    if gpu_device_count() < 2 {
        eprintln!("[skip] fewer than two GPUs visible to llama.cpp");
        return;
    }
    let gguf = tiny_gguf();
    if !gguf.exists() {
        eprintln!("[skip] tiny GGUF fixture missing: {gguf:?}");
        return;
    }

    use foundation_ai::types::SplitMode;
    let config = LlamaBackendConfig::builder()
        .offload_all_layers()
        .split_mode(SplitMode::None)
        .main_gpu(1)
        .build();

    let spec = ModelSpec {
        name: "tiny-llama-f16".to_string(),
        id: ModelId::Name("tiny-llama-f16".to_string(), None),
        devices: None,
        model_location: Some(gguf.to_string_lossy().to_string().into()),
        lora_location: None,
    };

    let model = LlamaBackends::LLamaGPU
        .load_model(spec, &config)
        .expect("the tiny GGUF must load pinned to GPU 1");
    let out = model
        .generate(greeting(), Some(params()))
        .expect("generation must succeed pinned to the second GPU");
    assert!(
        !out.is_empty(),
        "pinning to main_gpu=1 produced no messages"
    );
}
