---
feature: "F20 — CUDA end-to-end for candle and llama.cpp"
status: "in-progress"
priority: "high"
depends_on: ["F03"]
---

# F20 — CUDA end-to-end for candle and llama.cpp

## Progress (2026-07-22, post-reboot)

**Candle CUDA works end-to-end.** The tiny Llama fixture loads and generates on
the RTX 4070 Ti SUPER; 6 gated tests green (`tests/candle/candle_cuda_tests.rs`).

- **B1 resolved** — reboot synced the driver (kernel + userspace both 610.43.03);
  `nvidia-smi` lists both GPUs.
- **B2 resolved** — `CandleBackend` gained `cuda`, `try_cuda` (eager device
  open), `cuda_with_config`, `cpu_with_config`, Apple `metal`/`metal_with_config`,
  and `best_available` (detect + report, never a silent CPU downgrade). The
  `Cuda` variant is now reachable, so the 6 previously-uncoverable `cfg` arms are
  exercised.
- **W2 resolved** — cudarc 0.17.8 tops out at `cuda-13000`; the installed toolkit
  is 13.3. `CUDARC_CUDA_VERSION=13000` forces the 13.0 bindings, which are
  minor-compatible with 13.3. Build env: `CUDARC_CUDA_VERSION=13000
  CUDA_COMPUTE_CAP=89`.
- **Feature-wiring bug fixed** — `candle-cuda` enabled only `candle-core/cuda`,
  so the fused ops in candle-nn errored with "no cuda implementation for
  rms-norm". Now enables cuda on candle-core, candle-nn AND candle-transformers.

**llama.cpp CUDA works end-to-end too.** The committed tiny GGUF loads with full
offload and generates; 3 gated tests green
(`tests/providers/integrations/llamacpp_cuda.rs`).

- **B3 resolved** — added `LlamaBackendConfig::builder().offload_all_layers()`
  (passes `u32::MAX`, which llama.cpp clamps to the model's depth). The default
  stays 0/CPU-only, so GPU use is never accidental.
- **W4** — llama.cpp exposes no per-model "layers actually offloaded" counter, so
  the honest proof that this is a GPU build (not a CPU one ignoring
  `n_gpu_layers`) is `LlamaBackend::supports_gpu_offload()` returning true AND a
  device being present; the test asserts both, then generates with full offload.
- The llama.cpp CUDA library builds against the 13.3 toolkit (CMake/nvcc,
  ~21 min) with `CUDARC_CUDA_VERSION=13000 CUDA_COMPUTE_CAP=89`.
- Test discoverability: the `integrations` test module was gated only on
  `live-model-tests`; now `any(live-model-tests, cuda)` so the GPU test (which
  uses a committed fixture) runs under `--features cuda` alone. The other
  integration tests self-skip without their downloaded models.

Still open: a CPU-vs-CUDA output comparison on the fixture (acceptance #2's
"byte-comparable" — deferred pending a decision on floating-point tolerance
between CPU and GPU argmax), and the two-way `tensor_split` multi-GPU llama.cpp
run (acceptance #5; candle's second-GPU path is already covered).

## Goal

Local inference runs **on the GPU** through both backends — `candle` and
`llama.cpp` — from a public API a caller can actually reach, proven by a test
that fails if execution silently falls back to CPU.

Today neither backend can reach the GPU on this workstation, for three
independent reasons. Each is a separate work item below.

## Measured starting state (2026-07-22)

Hardware present and adequate; nothing here needs new silicon.

| Item | Value |
|---|---|
| GPU 0 | NVIDIA RTX 4070 Ti SUPER (AD102, Ada, SM 8.9) |
| GPU 1 | NVIDIA RTX 4060 Ti (AD106, Ada, SM 8.9) |
| CUDA toolkit | `nvcc` 13.3 (`/opt/cuda`) |
| Kernel module | `610.43.02` (open kernel module) |
| Userspace lib | `libnvidia-ml.so.610.43.03` |
| `nvidia-smi` | **fails** — `Driver/library version mismatch` |
| `cudarc` (lockfile) | 0.17.8 |
| `candle-core` | 0.11 |

Two GPUs matters: it makes `main_gpu`, `split_mode` and `tensor_split`
meaningful to test rather than degenerate single-device no-ops.

## The three blockers

### B1 — Driver/userspace version skew (environment)

The loaded kernel module is `610.43.02`; the userspace libraries installed on
Jul 10 are `610.43.03`. NVML refuses to initialise across that skew, so
**every** CUDA path fails before any of our code runs. The modules are in use
(`nvidia` refcount 115, `nvidia_drm` bound alongside `amdgpu`), so they cannot
be unloaded live on a running display server.

*Resolution:* reboot to load the matching `610.43.03` module. This is an
operator action, not a code change. Everything below is untestable until
`nvidia-smi` reports both GPUs.

### B2 — Candle's CUDA backend is unconstructible (code)

`CandleBackend` (`src/backends/candle.rs:175`) declares a `Cuda` variant behind
`#[cfg(feature = "candle-cuda")]`, and every internal `match` handles it —
`config()`, `device()` (`Device::new_cuda(*device_id)`), `cache()`,
`name()`. But the only public constructor is `cpu()` (line 200). No `cuda()`,
no `new_cuda(device_id)`, no auto-detect.

The variant is therefore reachable only by an internal re-wrap at line 256,
which preserves an *already existing* `Cuda` value that nothing can create. **As
shipped, `candle-cuda` cannot execute on a GPU no matter what flags are set** —
enabling the Cargo feature compiles the arms and changes nothing observable.

This also explains the 6 permanently-uncovered `cfg` sites at
`candle.rs:182,210,221,231,255,276`: they are unreachable by construction, not
merely untested.

### B3 — llama.cpp defaults to CPU and needs a compile-time feature (code + build)

Better shape than candle: `LlamaBackendConfig` already exposes `n_gpu_layers`,
`main_gpu`, `split_mode` and `tensor_split` as builder setters
(`llamacpp.rs:119-152, 230-280`), and `n_gpu_layers` reaches llama.cpp via
`params.with_n_gpu_layers()` at line 187. Two gaps:

1. `n_gpu_layers` defaults to `0` — "CPU-only by default" (line 144). Correct as
   a safe default, but it means a caller who enables the `cuda` feature and
   changes nothing else still runs entirely on CPU.
2. The `cuda` Cargo feature must be on for llama.cpp to be *built* with CUDA
   support (`cuda = ["llamacpp", "infrastructure_llama_cpp/cuda"]`). Without it
   the setters are accepted and silently ignored by a CPU-only build.

The dangerous combination is that both failures are **silent**: wrong feature
flags or a zero layer count produce correct answers, slowly, with no warning.

## Work items

- **W1** (operator) Reboot; confirm `nvidia-smi` lists both GPUs and
  `nvidia-smi -q` reports driver `610.43.03`.
- **W2** Verify toolchain compatibility *before* writing code: `cudarc` 0.17.8
  and `candle-core` 0.11 against **CUDA 13.3**. CUDA 13 is very new and cudarc
  pins supported toolkit versions via features. If unsupported, decide between
  pinning a CUDA 12.x toolkit alongside 13.3 or bumping candle/cudarc. Record
  the decision in `decisions/`.
- **W3** Give candle a real CUDA entry point: `CandleBackend::cuda(device_id)`,
  plus a `try_cuda()`/`best_available()` that falls back to CPU **and says so**
  through a return value or log — never a silent downgrade. Mirror for Metal so
  Apple does not regress.
- **W4** Make llama.cpp GPU use explicit and verifiable: a helper that offloads
  all layers (e.g. `n_gpu_layers(u32::MAX)` semantics as llama.cpp defines it),
  and a way to *read back* how many layers actually landed on GPU so a test can
  assert offload happened rather than assuming it.
- **W5** Detection + diagnostics: one shared "is CUDA usable right now" probe
  that distinguishes *not compiled in*, *no device*, and *driver/library
  mismatch* (B1's exact failure). Surface the mismatch as an actionable error —
  it is the single most likely field failure and today it appears as an opaque
  init error.
- **W6** Build/CI: document the feature matrix (`cuda`, `cuda_static`,
  `candle-cuda`/`candle-gpu`) and what each produces. Ensure a CPU-only build
  still compiles with the CUDA features absent.

## Acceptance criteria

1. `nvidia-smi` healthy; `cargo test -p foundation_ai --features cuda,candle-cuda`
   builds.
2. A public API creates a CUDA candle backend and runs a generation whose output
   is byte-comparable to the CPU path on the committed tiny fixtures.
3. A llama.cpp generation runs with layers demonstrably offloaded, asserted from
   a read-back count — **not** inferred from wall-clock speed.
4. Requesting CUDA when it is unavailable produces a *distinguishable* error per
   B1/B2/B3 cause; falling back to CPU is either explicit or refused.
5. Both GPUs exercised: `main_gpu = 1` and a two-way `tensor_split` run.
6. The 6 `candle.rs` `cfg(candle-cuda)` sites become genuinely covered — they
   are unreachable today (B2), so this is the honest close of that coverage gap
   rather than an excuse note.
7. CPU-only build with no CUDA features still compiles and passes.

## Test plan

Gate GPU tests behind a new `gpu-tests` feature, self-skipping when no device is
present, matching the existing `live-model-tests` / `external-service-tests`
convention — a machine without an NVIDIA card must never fail the suite.

The load-bearing assertion is **item 3**: a test that passes when the model
silently ran on CPU is worse than no test, because it certifies the exact bug
this feature exists to prevent. Assert on a read-back offload count.

## Notes / risks

- CUDA 13.3 with cudarc 0.17.8 is the largest unknown (W2) and is worth
  resolving before any other code work.
- `candle-core`'s `metal` feature is correctly Apple-gated
  (`Cargo.toml:91`); do not "fix" it.
- `nvidia_drm` is bound alongside `amdgpu` on this box; the AMD Raphael iGPU is
  present. Keep display duties on whichever device currently drives them —
  compute work should target the discrete cards explicitly by index.
