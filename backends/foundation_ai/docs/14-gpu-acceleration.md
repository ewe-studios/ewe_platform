# GPU Acceleration (CUDA, Metal, Vulkan)

Both local backends — **Candle** (pure-Rust) and **llama.cpp** (GGUF) — can run
on the GPU. This guide covers turning it on, the APIs that select a device, how
to *prove* a run actually used the GPU, and the build-environment gotchas.

> GPU support is **compile-time**: it is a Cargo feature that changes how the
> native libraries are built. A binary compiled without the feature has no GPU
> code in it at all — there is no runtime flag that turns it on. Pick the feature
> that matches your hardware before you build.

---

## 1. Feature flags

| Flag | Backend | Hardware |
|---|---|---|
| `candle-cuda` (alias `candle-gpu`) | Candle | NVIDIA (CUDA) |
| `cuda` | llama.cpp | NVIDIA (CUDA) |
| `cuda_static` | llama.cpp | NVIDIA, static CUDA linking |
| `metal` | llama.cpp | Apple Silicon |
| `vulkan` | llama.cpp | Cross-platform GPU |

Candle's Metal support is compiled in automatically on Apple targets (it is
`target_vendor = "apple"`-gated), so there is no separate `candle-metal` flag to
set.

```toml
# Cargo.toml — NVIDIA, both backends on the GPU
[dependencies]
foundation_ai = { version = "0.0.1", features = ["candle-cuda", "cuda"] }
```

---

## 2. Candle on the GPU

`CandleBackend` is an enum with `Cpu`, `Cuda`, and (Apple) `Metal` variants. Use
the constructors rather than building the variant by hand:

```rust
use foundation_ai::backends::candle::CandleBackend;

// Explicit CPU (always available).
let cpu = CandleBackend::cpu();

// Explicit CUDA device 0. Construction is lazy — the device opens on first
// model load, so this call itself cannot fail.
# #[cfg(feature = "candle-cuda")]
let gpu = CandleBackend::cuda(0);

// Validate the device NOW. Opens the CUDA context eagerly, so a missing GPU, a
// driver/library mismatch, or an out-of-range ordinal is a clear error here
// rather than a confusing failure deep inside the first load.
# #[cfg(feature = "candle-cuda")]
let gpu = CandleBackend::try_cuda(0)?;

// Second GPU, custom config.
# #[cfg(feature = "candle-cuda")]
let gpu1 = CandleBackend::cuda_with_config(1, config);
```

### `best_available()` — detect, don't guess

Picks CUDA (then Metal on Apple), and drops to CPU when no accelerator is usable.
The choice is **never silent** — the selected device is logged at `info`, and a
failed CUDA attempt is logged at `warn` with the reason:

```rust
use foundation_ai::backends::candle::CandleBackend;

// Uses the GPU if there is one, CPU otherwise, and logs which.
let backend = CandleBackend::best_available();
```

This exists because a GPU job that silently ran on CPU is the worst failure mode:
correct answers, quietly slow, no signal. `best_available` makes the fallback
visible in the logs so you can diagnose it after the fact.

### Running a model

Construction aside, the backend is used exactly like the CPU one — the device is
transparent to `ModelProvider`:

```rust
use foundation_ai::types::{ModelId, ModelProvider, ModelSpec};

# #[cfg(feature = "candle-cuda")]
# fn run() -> Result<(), Box<dyn std::error::Error>> {
let backend = CandleBackend::try_cuda(0)?;
let model = backend.get_model_by_spec(ModelSpec {
    name: "my-model".into(),
    id: ModelId::Name("my-model".into(), None),
    devices: None,
    model_location: Some("/path/to/model-dir".into()), // dir with config.json + *.safetensors
    lora_location: None,
})?;
// model.generate(...) / model.stream(...) now run on the GPU.
# Ok(())
# }
```

`describe()` reports the device in its name — `"Candle (CUDA)"` vs
`"Candle (CPU)"` — so you can confirm which variant you hold.

---

## 3. llama.cpp on the GPU

Unlike Candle, llama.cpp offload is a **layer count**, and the default is `0`
(CPU-only). A build with the `cuda` feature still runs entirely on CPU unless you
ask for layers — the opt-in is deliberate so GPU use is never accidental.

```rust
use foundation_ai::backends::llamacpp::{LlamaBackendConfig, LlamaBackends};

// Offload every layer. Passes u32::MAX, which llama.cpp clamps to the model's
// actual depth — you don't have to know the layer count.
let config = LlamaBackendConfig::builder()
    .offload_all_layers()
    .context_length(4096)
    .build();

// Or offload a specific number (e.g. to fit a large model in limited VRAM):
let partial = LlamaBackendConfig::builder()
    .n_gpu_layers(20)
    .build();

let model = LlamaBackends::LLamaGPU.load_model(spec, &config)?;
```

### Multi-GPU

```rust
use foundation_ai::backends::llamacpp::LlamaBackendConfig;
use foundation_ai::types::SplitMode;

let config = LlamaBackendConfig::builder()
    .offload_all_layers()
    .split_mode(SplitMode::Layer) // split layers across GPUs
    .main_gpu(0)                  // primary device for the KV cache
    .build();
```

### Proving the GPU was used

llama.cpp does not expose a per-model "layers actually offloaded" counter, so the
honest check that you have a GPU-capable build — rather than a CPU-only one that
silently ignored `n_gpu_layers` — is `supports_gpu_offload()`:

```rust
use infrastructure_llama_cpp::llama_backend::LlamaBackend;

let backend = LlamaBackend::init_or_get()?;
if !backend.supports_gpu_offload() {
    // This build was compiled without a GPU feature — everything will run on CPU.
    eprintln!("warning: no GPU offload support in this build");
}
```

If this returns `false` under `--features cuda`, the native library was compiled
without GPU support — check the build environment below.

---

## 4. Build environment (NVIDIA / CUDA)

Building the CUDA backends compiles native kernels with `nvcc`. Two environment
variables matter, and one is a genuine footgun.

```bash
export CUDA_COMPUTE_CAP=89        # your GPU's compute capability (Ada = 89)
export CUDARC_CUDA_VERSION=13000  # see below — required on CUDA 13.1+
cargo build --features "candle-cuda cuda"
```

### `CUDARC_CUDA_VERSION` — the CUDA 13.1+ footgun

`cudarc` (Candle's CUDA binding) ships a hard-coded table of recognised toolkit
versions. As of the pinned version it tops out at **CUDA 13.0**; a newer toolkit
(13.1, 13.2, 13.3, …) makes its `nvcc --version` parser **panic** with
`Unsupported cuda toolkit version`.

The fix is to pin the binding to the 13.0 API, which is minor-compatible with any
CUDA 13.x toolkit:

```bash
export CUDARC_CUDA_VERSION=13000   # forces the 13.0 bindings
```

You do not need this on CUDA 13.0 or 12.x — only when your installed toolkit is
newer than cudarc knows about. Find your compute capability with
`nvidia-smi --query-gpu=compute_cap --format=csv,noheader`.

### Driver / library version mismatch

If `nvidia-smi` reports `Failed to initialize NVML: Driver/library version
mismatch`, the loaded kernel module and the userspace libraries are out of sync
— usually after a driver upgrade without a reboot. **Reboot.** No amount of
build configuration works around a mismatched driver; every CUDA call fails
before your code runs.

---

## 5. What runs where

| | Candle | llama.cpp |
|---|---|---|
| Select device | `cuda(id)` / `try_cuda(id)` / `best_available()` | `offload_all_layers()` / `n_gpu_layers(n)` |
| Default | CPU | CPU (0 layers) |
| Multi-GPU | `cuda(1)`, `cuda(2)`, … | `split_mode` + `main_gpu` |
| Fallback reporting | `best_available()` logs the choice | check `supports_gpu_offload()` |
| Confirm device | `describe().name` contains `"CUDA"` | `supports_gpu_offload()` |

---

## 6. Verifying your setup

The repository ships gated GPU tests that double as worked examples:

- `tests/candle/candle_cuda_tests.rs` — device open, generation on CUDA,
  distinguishable error for an absent device, second-GPU use, `best_available`.
- `tests/providers/integrations/llamacpp_cuda.rs` — `supports_gpu_offload`,
  `offload_all_layers`, GGUF generation with offload.

Run them against your hardware:

```bash
export CUDARC_CUDA_VERSION=13000 CUDA_COMPUTE_CAP=89
cargo test --profile uat --features "testing candle-cuda" candle::candle_cuda
cargo test --profile uat --features "testing cuda"        llamacpp_cuda
```

They self-skip when no usable device is present, so they never fail a machine
without a GPU.

---

## See also

- **Getting Started: Providers** (`getting-started/01-providers.md`) — provider
  setup, including the local backends.
- **Doc 03 — Model Providers** — the provider/router internals.
