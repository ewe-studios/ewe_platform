---
feature: "wasm target matrix — prepare foundation_wasm + foundation_testbed for emscripten + WASI (p1/p2)"
description: "Make the agentic-relevant wasm-capable crates compile + run across the full wasm matrix — wasm32-unknown-unknown (CF Workers/wasm-bindgen), wasm32-unknown-emscripten (browser/WebGPU/llama), wasm32-wasip1, wasm32-wasip2 — by replacing blanket target_arch gating with target_os-aware gating, adding WASI/emscripten host backends to foundation_wasm, and extending the foundation_testbed harness to run them"
status: "pending"
priority: "medium"
depends_on: ["00-foundation-compact"]
estimated_effort: "large"
created: 2026-06-15
last_updated: 2026-06-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature 00d: wasm target matrix (emscripten + WASI p1/p2)

> Phase-0 prep so the agentic stack isn't locked to a single wasm target. Today the wasm story targets
> only **`wasm32-unknown-unknown`** (CF Workers / wasm-bindgen). This feature prepares
> **`foundation_wasm`** (the nostd WASM/JS runtime) and **`foundation_testbed`** (the harness) — plus the
> spec's cfg-gating discipline — for the full matrix: **`unknown-unknown`**, **`unknown-emscripten`**
> (the llama/WebGPU path, F00b OD-00b-7), **`wasip1`**, **`wasip2`**.

## WHY: Problem Statement

The spec gates wasm with blanket **`cfg(target_arch = "wasm32")`** / `cfg(not(target_arch="wasm32"))`.
That is too coarse — it lumps four *very different* targets together:

| Target | std? | time | threads | host | net/HTTP | use |
|--------|------|------|---------|------|----------|-----|
| `wasm32-unknown-unknown` | **no** (no libc) | panics (needs polyfill) | no | wasm-bindgen / foundation_wasm JS | fetch (none in netio yet) | CF Workers, browser SPA |
| `wasm32-unknown-emscripten` | **partial** (emscripten libc) | std works | pthreads | emscripten + WebGPU | emscripten sockets | browser local inference (llama, WebGPU) |
| `wasm32-wasip1` | **yes** (WASI preview1) | std (clock) works | wasi-threads | WASI host (wasmtime/wasmer) | WASI sockets | server/edge wasm, Deno, some CF |
| `wasm32-wasip2` | **yes** (WASI 0.2 component model) | std works | yes | WASI component host | wasi:http | component-model deployments |

So emscripten + WASI have **far more std capability** than `unknown-unknown` — and the code that's
gated off "all wasm" today wrongly excludes targets where it would work (e.g. `std::time`,
`std::thread`, parts of fff/fjall). The two crates that own the wasm runtime + testing must be made
**target-OS-aware**, and the entropy/time substrate (F00) already covers the matrix (its vendored
getrandom ships `wasm_js` + `wasi_p1` + `wasi_p2_3` + emscripten `getentropy`; time uses std on
emscripten/wasi, polyfill only on `unknown-unknown`).

## WHAT: Solution

### 1. cfg discipline — target_os-aware, not blanket target_arch

Establish the canonical discriminators (used spec-wide):

```rust
// the RESTRICTIVE no-std browser/CF target (the one needing polyfills + JS host):
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
// emscripten (libc + threads + WebGPU; llama works):
#[cfg(all(target_arch = "wasm32", target_os = "emscripten"))]
// WASI preview 1 / preview 2:
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]   // (+ target_env "p1"/"p2" where needed)
```

Audit the agentic crates' wasm gates: native-only C tooling (fff's heed/git2/memmap2, llama's CMake)
stays gated to **native + emscripten** where it actually builds; pure-Rust agentic machinery
(foundation_compact, foundation_vectors, the agentic types/loop/memory) targets **all four**.

### 2. `foundation_wasm` — add WASI + emscripten host backends

`foundation_wasm` today: a nostd WASM/JS interop runtime; the **`web`** feature enables the JS host ABI
(`wasm_import_module = "abi"` imports, timers, `host_apply`) — that's the `unknown-unknown`/browser
path. Its cfg gates on `target_arch` (`wasm32`/`wasm64`), not `target_os`. Needed:

- **Make the host abstraction target-OS-aware.** The `web` (JS) host stays for unknown-unknown +
  emscripten-in-browser. Add a **WASI host backend** (`wasi` feature) — under wasip1/p2 there is no JS
  `abi`; host calls go through WASI (preview1 syscalls / preview2 component imports). Time, entropy,
  scheduling resolve via WASI, not the JS ABI.
- **Features:** keep `web`; add `wasi` (WASI preview1) and `wasip2` (component model) so a consumer
  selects its host. The `host_runtime`/`frames`/`intervals`/`schedule` modules gate the JS-import path
  on `web` and provide a WASI path under `wasi`.
- The executor's JS-event-loop yield (`local.rs:2682`, `js-wasmbindgen`/`js-foundation-wasm`) is
  unknown-unknown/emscripten-specific; WASI hosts drive the executor differently (poll/yield via WASI)
  — document the per-host scheduling model.

### 3. `foundation_testbed` — run the matrix

`foundation_testbed`'s `wasm` feature is a **`wasm32-unknown-unknown`**-only harness (browser via the
pure-Rust CDP/BiDi driver, Deno, CF Workers; built with `walrus` + `foundation_browser`/`foundation_http`/
`foundation_wasm_ui`). Extend it to build + run the other targets:

- **emscripten** (`wasm32-unknown-emscripten`): build via the vendored EMSDK (`tools/emsdk`), run in
  node/browser (WebGPU). Add an emscripten runner.
- **WASI** (`wasm32-wasip1` / `wasm32-wasip2`): run under **wasmtime** (no wasmer — OD-00d-3 resolved).
  The WASI runner is owned by **`foundation_wasmtime`** which is **DEFERRED to Phase 4 / last** (user,
  2026-06-15): nothing in core machinery depends on it, so this **WASI-runner portion of 00d splits out
  and lands *with* `foundation_wasmtime`**. 00d ships the native / `unknown-unknown` / emscripten runners
  first; the `wasi` harness runner (loads `.wasm` + WASI imports, asserts output) is built when
  `foundation_wasmtime` is.
- A **target enum** in the harness (`unknown-unknown | emscripten | wasip1 | wasip2`) + per-target
  runner, so a test declares which targets it must pass on.

### 4. The agentic build matrix (what must compile where)

| Crate | uu | emscripten | wasip1 | wasip2 | notes |
|-------|----|-----------|--------|--------|-------|
| `foundation_compact` | ✅ | ✅ | ✅ | ✅ | F00 vendored entropy covers all; time=std except uu |
| `foundation_vectors` | ✅ | ✅ | ✅ | ✅ | pure Rust (flat/IVF/HNSW/BM25); code-graph native-only |
| `foundation_db` (agentic subset) | ✅ | ✅ | ✅ | ✅ | in-memory + CF; fjall/native gated to native+emscripten? (verify) |
| `foundation_ai` (agentic, `--features agentic`) | ✅ | ✅(+llama) | ✅ | ✅ | llama on native+emscripten (OD-00b-7); providers' HTTP per host |
| `foundation_wasm` | ✅(web) | ✅(web) | ✅(wasi) | ✅(wasip2) | this feature |

## Architecture

```mermaid
graph TD
    subgraph "wasm target matrix"
        UU[unknown-unknown: JS host, polyfill time, CF/browser]
        EM[emscripten: libc+threads+WebGPU, llama]
        W1[wasip1: WASI host, std]
        W2[wasip2: component model]
    end
    FW[foundation_wasm] -->|web host| UU
    FW -->|web host| EM
    FW -->|wasi host| W1
    FW -->|wasip2 host| W2
    FC[foundation_compact: entropy+time per target] --> UU & EM & W1 & W2
    TB[foundation_testbed runners] -->|CDP/Deno/CF| UU
    TB -->|emsdk node| EM
    TB -->|wasmtime/wasmer| W1 & W2
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: the **wasm target matrix** (unknown-unknown vs emscripten vs wasip1
vs wasip2 — libc, threads, time, host model, networking — and the `target_os`/`target_env`
discriminators); **emscripten** (the SDK, libc, pthreads, WebGPU, why llama builds there); **WASI**
(preview1 syscalls, preview2 component model + WIT, `wasi:random`/`wasi:clocks`/`wasi:http`, wasmtime/
wasmer); the **JS host ABI** (`wasm_import_module`, `getRandomValues`, event-loop yield) vs WASI host;
how to write **target-OS-aware cfg** (not blanket `target_arch`); building/running each target in CI.
(Task — see list.)

## HOW: Implementation Steps

1. Define the canonical cfg discriminators + audit the agentic crates' wasm gates (native-C tooling →
   native+emscripten; pure-Rust → all four).
2. `foundation_wasm`: make host gating `target_os`-aware; add `wasi` + `wasip2` features + WASI host
   backend alongside `web`; document per-host scheduling.
3. `foundation_testbed`: add a target enum + emscripten runner (EMSDK) + WASI runner (wasmtime/wasmer,
   OD-00d-3); keep the existing uu browser/Deno/CF harness.
4. Add the targets to tooling (`mise.toml` already has `wasm32-wasip1`; add `wasip2`, emscripten via
   EMSDK) and a CI matrix.
5. Build-check the agentic crates across the matrix; fix residual gates surfaced.
6. Tests: per-target build of foundation_compact/foundation_vectors/foundation_ai(agentic); a
   foundation_wasm host smoke per host; a foundation_testbed runner smoke per target.

## Open Decisions

- **OD-00d-1 — scope of host backends in `foundation_wasm`:** full WASI preview2 component host now, or
  preview1 first + preview2 follow-up. Rec: wasip1 first (broader runtime support), wasip2 next.
      **TODO**: Review, come up with two features for each, lets review and see whats possible and what we need to do and the scale for it.

- **OD-00d-2 — which crates are required on which targets:** the matrix table above is the proposal;
  confirm (esp. whether `foundation_db` fjall/native is gated native-only or native+emscripten).
      Its about where it works, if it wworks with native + emscripten then we make it work and gate them properly.

- **OD-00d-3 — WASI runtime for the harness: RESOLVED (user, 2026-06-15) → `wasmtime` always** (no
  wasmer). Owned by a dedicated **`foundation_wasmtime`** crate (nice API: set host imports, get an
  executable exposing the exports). **DEFERRED to Phase 4 / last** (discussion §B2): nothing in core
  machinery depends on it, so the WASI runner here lands together with `foundation_wasmtime`. The other
  00d runners (native/unknown-unknown/emscripten) are not blocked.

- **OD-00d-4 — emscripten in CI:** EMSDK is vendored (`tools/emsdk`); confirm the CI build wires it and
  whether emscripten is gated behind a testbed feature.
          - Yes its time to invest more in this and get this build target working well.

- **OD-00d-5 — spec-wide cfg refactor:** replace blanket `cfg(target_arch="wasm32")` across the agentic
  features with the target_os discriminators. Rec: do it as part of each feature's wasm gating, with
  this feature owning the canonical pattern + the audit checklist.
       - Feature it and plan it properly, understand the dependency chain and where and when it must land, its our opportunity to make it all worthwhile here. Lets get it right

## Target Files

- `backends/foundation_wasm/` — `Cargo.toml` (`wasi`/`wasip2` features), `host_runtime.rs`/`frames.rs`/
  `intervals.rs`/`schedule.rs` (target_os-aware host backends)
- `backends/foundation_testbed/` — `Cargo.toml` (wasmtime/wasmer + emsdk deps), `src/wasm/` (target enum
  + emscripten + WASI runners)
- `mise.toml` / CI — add `wasm32-wasip2` + emscripten targets
- (coordination) the agentic features' wasm cfg gates (OD-00d-5)

## Tests

```bash
# substrate across the matrix
for t in wasm32-unknown-unknown wasm32-wasip1 wasm32-wasip2; do cargo build -p foundation_compact --target $t; done
cargo build -p foundation_wasm --features wasi --target wasm32-wasip1
cargo test  -p foundation_testbed --features wasm -- matrix   # per-target runner smoke
```

## Verification

```bash
cargo build -p foundation_compact --target wasm32-wasip1
cargo build -p foundation_compact --target wasm32-wasip2
cargo build -p foundation_wasm --features wasi --target wasm32-wasip1
cargo build -p foundation_ai --no-default-features --features agentic --target wasm32-wasip1
cargo clippy --workspace -- -D warnings
```

## Done When

- `foundation_wasm` builds + provides a host on `unknown-unknown` (web), `wasip1`/`wasip2` (wasi), and
  emscripten; cfg gating is target_os-aware.
- `foundation_testbed` can build + run tests on `unknown-unknown` (existing), emscripten, wasip1, wasip2.
- The agentic pure-Rust crates compile across the matrix; native-C tooling (fff/llama) is gated to
  native+emscripten, not "all wasm".
- The canonical target_os cfg pattern is documented + adopted (OD-00d-5). OD-00d-1..4 resolved;
  fundamentals authored.
