# 15. Upgrading CUDA, cudarc, candle and llama.cpp

Setup lives in [14. GPU Acceleration](14-gpu-acceleration.md) — what the
environment variables mean and how to get a GPU build working the first time.
This guide is the **upgrade procedure**: what to change, in what order, and how
to prove you did not silently break the GPU path.

Read it before touching the CUDA toolkit, the NVIDIA driver, `cudarc`, `candle`
or the vendored llama.cpp. These four have version constraints that are **not**
expressed in `Cargo.toml`, so nothing stops you from creating a broken
combination — and most of the failure modes are silent or point somewhere else.

## The one thing to remember

`CUDARC_CUDA_VERSION=13000` is a **floor-rounding, not a downgrade**. `cudarc`
ships separate generated bindings per CUDA minor version; the pinned version
tops out at `cuda-13000` while the installed toolkit is 13.3. The pin says "use
the 13.0 bindings", which is safe because CUDA is minor-version compatible
within a major series.

That reasoning is what decides every upgrade below.

## Upgrading the CUDA toolkit

| New toolkit | Action |
|---|---|
| Another **13.x** (13.4, 13.5, …) | **Nothing.** Keep `13000`. Minor-compatible. |
| **14.x** | `13000` becomes a *major* mismatch. Bump `cudarc` first, then set the variable to the highest `cuda-*` binding the new `cudarc` offers that is `<=` your toolkit. Never leave it pointing across a major boundary. |

Then run the verification suite below. A major-version mismatch can compile and
bind the wrong ABI — only the parity test catches that.

## Upgrading `cudarc`

`cudarc` is a **transitive** dependency via `candle-core`, so a candle bump can
move it without you asking. Check after any candle upgrade.

```bash
cargo tree -p foundation_ai --features candle-cuda -i cudarc     # version in use
```

Then, in that version's `Cargo.toml`, see which bindings exist:

```bash
grep -o 'cuda-[0-9]*' Cargo.toml | sort -u | tail -5
```

If the new `cudarc` offers a binding `<=` your installed toolkit, **raise**
`CUDARC_CUDA_VERSION` to it. Matching bindings always beat rounding down. Update
it in three places or the pin drifts out of sync with reality:

- `docs/14-gpu-acceleration.md` (setup instructions)
- this file
- any CI/scripts that export it

## Upgrading candle

Check the feature wiring survived. `candle-cuda` must enable `cuda` on **all
three** candle crates:

```toml
candle-cuda = [
    "candle",
    "candle-core/cuda",
    "candle-nn/cuda",           # fused rms-norm, softmax, rope live here
    "candle-transformers/cuda", # the model code lives here
]
```

Enabling only `candle-core/cuda` builds cleanly and fails at **runtime** with
"no cuda implementation for rms-norm". That was a real bug here. If a candle
crate joins the dependency list, add its `cuda` feature at the same time.

## Upgrading the vendored llama.cpp

It lives at `infrastructure/llama-bindings/llama.cpp` — tracked in this repo,
with an upstream remote. Check whether the C API we bind actually moved:

```bash
cd infrastructure/llama-bindings/llama.cpp
git fetch --tags origin
git diff <current>..<target> -- include/llama.h ggml/include/ggml.h \
  | grep -E '^[+-]' | grep -v '^[+-][+-]'
```

Additive changes — new functions, new enum values, a `GGML_TYPE_COUNT` bump —
are safe; bindgen picks them up with no work. Removals or signature changes need
binding changes in `infrastructure/llama-cpp/src/`.

The risk in these upgrades is rarely the headers. It is the build flags and the
chat-template shim: the legacy template API returns -1 for modern models
(Gemma-4), which is why the minja/common shim exists. Budget ~23 minutes for the
rebuild — nvcc recompiles the CUDA kernels.

## Verifying — the part that is not optional

```bash
CUDARC_CUDA_VERSION=13000 CUDA_COMPUTE_CAP=89 \
  cargo test --profile uat -p foundation_ai --features "testing cuda candle-cuda" \
  -- cuda --test-threads=1
```

Ordered so the most diagnostic failure comes first:

| Test | Proves |
|---|---|
| `the_build_reports_gpu_offload_support` | the `cuda` feature really compiled llama.cpp **with** GPU support. If this fails you have a CPU-only library and every "GPU" run is a lie. |
| `a_cuda_backend_can_be_constructed_and_opened` | the device opens — catches driver skew and wrong `cudarc` bindings. |
| `cuda_and_cpu_agree_on_the_same_fixture` | **the load-bearing one.** A wrong-ABI bind, a dtype mismatch or a fused op computing the wrong thing will still run and still produce text. This is what catches it. |
| `a_two_way_tensor_split_generates_across_both_gpus` | multi-GPU placement reaches llama.cpp and works. |
| `main_gpu_one_generates_on_the_second_device` | the second GPU is genuinely usable. |

### Confirm the tests did not skip

They **self-skip** when no device is present, so a green run on a GPU-less
machine proves nothing. On a machine with GPUs, verify they actually ran:
temporarily replace each `eprintln!("[skip] …"); return;` guard with `panic!` and
re-run. Zero panics means every test executed.

A skipping GPU test that reports `ok` is precisely the failure this feature
exists to prevent — a test that passes when the model silently ran on CPU is
worse than no test, because it certifies the bug.

### Why parity is compared on tokens, not logits

Comparing raw logits between CPU and GPU is wrong: different kernel orderings
produce bit-different floating-point sums, so a byte comparison fails on a
*correct* implementation. Greedy decoding at temperature 0 takes an argmax,
which is stable under those differences unless something is genuinely broken. So
the comparison is on the **decoded token sequence** — the strongest claim that
survives being true.

## If it breaks

| Symptom | Cause |
|---|---|
| `Unsupported cuda toolkit version` at build | `CUDARC_CUDA_VERSION` unset, or `cudarc` older than the toolkit |
| `no cuda implementation for rms-norm` at runtime | feature wiring — `candle-nn/cuda` missing |
| `no kernel image is available for execution` | `CUDA_COMPUTE_CAP` wrong for the card; set it to the **lowest** capability across the GPUs in use |
| `Driver/library version mismatch` from `nvidia-smi` | kernel module vs userspace skew. **Reboot** — no build configuration works around it |
| Builds and runs, output is wrong or garbage | wrong-ABI bind. The parity test is the check; re-examine `CUDARC_CUDA_VERSION` against the toolkit |
| GPU tests all `ok` suspiciously fast | they skipped. See "Confirm the tests did not skip" |
| `E0432: unresolved imports foundation_nativeapis::{VfsSearcher, …}` in **doctests** only | a stale build artifact after alternating the `cuda` and CPU-only feature sets — not a code or feature problem (resolution is identical under both). Rerun; it self-corrects. `cargo clean -p foundation_nativeapis` if it persists. |
