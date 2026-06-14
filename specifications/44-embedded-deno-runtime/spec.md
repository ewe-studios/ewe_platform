# Spec 44 — Embedded Deno runtime for zero-install JS test execution

Status: **PROPOSAL** (2026-06-14) — design + complexity + timeline for review. No
implementation yet. Featureless (single design doc); if approved it becomes a
feature breakdown.

## 1. Goal

Run the `foundation_testbed` `wasm` harness's JavaScript **in-process** via an
embedded Deno runtime (a Rust **library**), so:

- `cargo build` gives JS execution **for free** — no `node`/`deno` install, no
  `cargo install deno`, no `which::which` checks at runtime;
- we consolidate on one JS engine (Deno/V8) and **drop the node path** entirely;
- the JS-runtime test path becomes hermetic and CI-trivial (no toolchain setup).

This is the path chosen over "install the deno binary": a `[dependencies]` entry
on the `deno` *binary* crate does **not** yield an executable (Cargo never builds
a dependency's `[[bin]]`), and `cargo install deno` is a separate, heavy, and now
largely-deprecated step. Embedding the runtime as a **library** is the only option
that actually removes the install.

## 2. Why V8/Deno specifically (not wasmtime or QuickJS)

The point of the JS-runtime test is to validate a built wasm module against the
**real JS host runtime** (`runtimes/foundation-wasm.js` — the host ABI a browser
or Deno provides), the same way it would run in production. So we need a JS engine
that runs *both* our host-runtime JS *and* `WebAssembly`:

- **wasmtime** — runs the wasm, but there's no JS; we'd have to reimplement
  `foundation-wasm.js`'s host ABI in Rust, which defeats the purpose (it stops
  testing the JS side). ✗
- **QuickJS** (`rquickjs`) — tiny, but no/weak `WebAssembly` support. ✗
- **V8** (via `deno_core`) — full `WebAssembly`, browser-parity semantics, and
  it's what Deno already is. ✓

## 3. Current state (what changes)

`foundation_testbed` (`wasm` feature) has these runners:

| Runner | Mechanism | After this spec |
|--------|-----------|-----------------|
| browser | `foundation_browser` CDP/BiDi (pure Rust, no node) | **unchanged** |
| deno | `Command::new("deno") run --allow-read --allow-net` (`src/wasm/deno.rs`) | replaced by the embedded runtime |
| node | `Command::new("node") runner.mjs` (`src/wasm/fwt_runner.rs`) | **removed** |
| wrangler | `Command::new` wrangler | out of scope (keep as-is or revisit) |

The harness JS (`runner.mjs` + `foundation-wasm.js`) needs, from the engine:
URL-relative **ES module imports**, **`WebAssembly`** (Module/Instance/instantiate),
byte loading (**`fetch`** and/or `Deno.readFile` / `node:fs`), **`console`**,
**`TextEncoder`/`TextDecoder`**, and an **exit code** (`Deno.exit`/`process.exit`).
It currently branches on node-vs-deno; going deno-only lets us delete the node
branches.

## 4. Approach

Add `deno_core` (or `deno_runtime`) as a `wasm`-gated library dependency and run
the harness through a Rust driver instead of a subprocess.

### 4.1 Crate choice (key decision)

- **`deno_runtime`** — the *full* runtime as a library: `fetch`, timers, `Deno.*`,
  fs, permissions, web APIs. Least JS adaptation, but the heaviest dep (pulls a
  large transitive tree incl. **tokio**) and the most version churn.
- **`deno_core` + selected extensions** (`deno_web`, `deno_console`, `deno_url`,
  `deno_fetch` if needed) — leaner, more explicit wiring; we provide only what the
  harness uses. More upfront work, smaller surface, fewer surprises.

Recommendation: start the spike with **`deno_core` + `deno_web` + `deno_console`**
and a custom module loader + a couple of ops; only reach for `deno_runtime` if the
harness genuinely needs the full web platform. (Byte loading can be a Rust op
instead of `fetch`, avoiding `deno_fetch`/net entirely.)

### 4.2 Components to build

1. **Module loader** — a `deno_core::ModuleLoader` that resolves the harness's
   URL-relative imports (`./foundation-wasm.js`, `./module.wasm`-adjacent JS) from
   the **embedded** runtime assets (`foundation_wasm::embedded::*`) + the staged
   module dir, instead of disk/network.
2. **Ops (Rust ↔ JS bridge)** — `#[op2]` functions for: read module bytes
   (replaces `Deno.readFile`/`fetch`), and **report results** (the harness calls
   `report(json)` → Rust captures it directly, replacing stdout parsing + exit
   codes). This makes the result contract robust and engine-agnostic.
3. **The driver** — a function that builds a `JsRuntime`, registers the loader +
   ops, evaluates `runner.mjs` against a staged module, drives the event loop to
   completion, and returns the existing `RunOutcome` (so `fwt_runner`'s public
   contract is unchanged).
4. **Harness adaptation** — collapse `runner.mjs` to the embedded-runtime API
   (drop `node:fs`/`process.exit` branches; use the report op). Keep
   `foundation-wasm.js` as-is (it's the thing under test).

### 4.3 Runtime model

`deno_core`'s event loop is **tokio-based**. foundation uses valtron, not tokio.
The embedded JS runs on a small tokio runtime *inside* the test driver — acceptable
for a test tool, but it introduces tokio into `foundation_testbed`'s `wasm`
feature. (Noted as a deliberate trade-off, gated behind `wasm`.)

## 5. Work breakdown

| # | Work | Output |
|---|------|--------|
| W1 | **Spike**: add the dep, eval `1+1` + a `WebAssembly.instantiate` in-process; confirm rusty_v8 prebuilt V8 resolves on our nightly toolchain + the `uat` (LLVM) profile. | go/no-go on the build |
| W2 | Module loader resolving the harness imports from embedded assets + staged dir | loader |
| W3 | Ops: read-bytes + report-results; run a real sample module's `#[wasm_test]` cases end-to-end | green sample |
| W4 | Driver returning `RunOutcome`; swap `deno.rs` + the `fwt_runner` host path to it; delete `Command::new`/`which` + `DenoNotFound`/`HostRuntimeNotFound` | embedded runner |
| W5 | **Drop node**: delete node branches in `runner.mjs`/templates, node refs in `mise.toml`, the node runner | deno-only |
| W6 | Port `fwt_runner_tests` to the in-process runner (no external deno); CI no longer needs node/deno | tests |
| W7 | Docs: README "zero-install JS testing"; spec → feature status | docs |

## 6. Complexity

**High**, concentrated in W1–W3:

- **V8 build weight.** `rusty_v8` downloads a prebuilt V8 static lib (it does *not*
  compile V8 from source), but linking it adds **hundreds of MB** to the artifact
  and notable link time to every `foundation_testbed` build that has `wasm` on
  (which is the default). Compile-time and target-dir size both jump.
- **deno_core API churn.** `deno_core`/`deno_runtime` move fast and are tightly
  version-coupled to a V8 version; we pin and bump deliberately.
- **Module loader + ops** are fiddly (async resolution, ESM vs the wasm side).
- **tokio** enters the dep tree (engine event loop).
- **Toolchain interaction** with the project's Cranelift dev profile (already
  `uat`-only for this crate) + nightly + edition 2024 — the spike must de-risk this
  first.

## 7. Risks & mitigations

- **Default build gets heavy** (V8 on every `wasm`-enabled build). *Mitigation:*
  put V8 behind a sub-feature (e.g. `wasm-embedded-js`) so plain `wasm` keeps the
  lighter capabilities and only the in-process JS runner pulls V8. Trade-off: not
  "free" in the absolute default, but opt-in-cheap. **Open decision.**
- **rusty_v8 prebuilt availability** for our exact nightly — if missing, it builds
  V8 from source (very heavy). De-risk in W1.
- **deno_runtime is large/opinionated** — prefer `deno_core` + minimal extensions.
- **Maintenance** — pin versions; a periodic bump task.

## 8. Timeline (rough — wide error bars)

Assuming focused work and that the W1 spike goes green:

| Phase | Estimate |
|-------|----------|
| W1 spike (build de-risk) | 0.5–1.5 days |
| W2 loader | 1 day |
| W3 ops + sample green | 1–1.5 days |
| W4 driver + swap runners | 1 day |
| W5 drop node | 0.5 day |
| W6 tests | 0.5–1 day |
| W7 docs | 0.5 day |
| **Total** | **~5–6 focused days** |

Dominant uncertainty is W1 (V8/rusty_v8 + toolchain). If the spike reveals build
trouble, re-evaluate vs. the simpler "install deno binary via mise/cargo-binstall"
fallback.

## 9. Open decisions (for review)

1. **`deno_core` + extensions vs `deno_runtime`** — lean-and-wire vs fat-and-easy.
2. **V8 behind a sub-feature** vs always-on with `wasm` (heavy default).
3. **Keep the deno *binary* fallback** for environments that don't want V8 in the
   build? (i.e. both an embedded runner and a shell-out runner.)
4. **wrangler** — leave on `Command::new`, or also revisit (Workers needs the
   wrangler/workerd toolchain regardless; likely out of scope).

## 10. Recommendation

Proceed **only after a timeboxed W1 spike** (≤1.5 days) proves rusty_v8 + the
runtime compile and run a `WebAssembly.instantiate` on our nightly/`uat` setup.
Use `deno_core` + minimal extensions, and (likely) gate V8 behind a
`wasm-embedded-js` sub-feature so a plain `wasm` build stays light. If the spike is
rough, fall back to installing the deno binary via `mise`/`cargo binstall` and just
dropping node — a fraction of the effort, though it keeps an install step.
