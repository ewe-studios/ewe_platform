# Spec 44 — Embedded Deno runtime for zero-install JS test execution

Status: **IMPLEMENTED** (2026-06-14) — W1–W7 all green (see §5). The owned
`#[wasm_test]` harness runs entirely **in-process** on an embedded `deno_core` +
`deno_web` runtime (behind the `wasm-embedded-js` feature, `uat` profile): no
external `node`/`deno`, the node runner is gone, and the runner tests run the real
sample crate through V8 with zero install. Open decisions in §9 are resolved inline. Featureless (single design doc); if
approved it becomes a feature breakdown.

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

### 4.4 Deferred capability — Web Crypto (`deno_crypto`) — IMPORTANT

We deliberately **do not** pull `deno_crypto` (Web Crypto: `crypto.subtle`,
`crypto.getRandomValues`). Two reasons:

1. **Not needed today.** The harness JS uses `WebAssembly`, ES modules, `console`,
   `TextEncoder`/`TextDecoder`, timers, and `fetch` — none touch Web Crypto, and
   `foundation-wasm.js` (the code under test) doesn't either.
2. **It was the build blocker.** Pulling the *full* `deno_runtime` dragged in
   `deno_crypto`, which pins `aes = "=0.8.3"` (an **exact** pin). The workspace
   already resolves `aes 0.8.4` via `foundation_db → turso → turso_core → aes-gcm`.
   Both are in the `0.8.x` compatibility band, which Cargo unifies to **one**
   version; `=0.8.3 ∩ >=0.8.4 = ∅` → unresolvable. (`foundation_testbed` itself
   never depends on turso; the clash is purely the shared workspace `Cargo.lock`,
   because the root `ewe_platform` binary depends on both.) Composing `deno_core` +
   only the extensions we need removes the `=0.8.3` pin and the workspace resolves.

**Future need (recorded for awareness):** when we want to test/validate wasm or JS
that *uses* the Web Crypto APIs (e.g. `crypto.subtle.digest`, `getRandomValues`,
key derivation), we will need crypto in the embedded runtime. Re-add path at that
point, in order of preference:

- Add a `deno_crypto` release whose `aes` requirement no longer pins `=0.8.3`
  (i.e. bump `deno_core`/extension set to a version line where the pin is gone or
  is `>=0.8.4`-compatible) so it unifies with turso's `aes 0.8.4`; **or**
- if still pinned, provide the needed primitives via a small `#[op2]` shim backed by
  a RustCrypto `aes`/`sha2` version already in the tree (no new conflicting pin);
- gate it behind a further sub-feature (e.g. `wasm-embedded-crypto`) so it's opt-in
  and a plain in-process JS build stays free of the crypto dep tree.

This trade-off (no `crypto.subtle` for now) is the only capability we gave up by
choosing `deno_core` + extensions over `deno_runtime`.

### 4.3 Runtime model

`deno_core`'s event loop is **tokio-based** — not a choice: `reactor_tokio.rs` builds
its timers on `tokio::time::Sleep`, and the loop calls `tokio::runtime::Handle::current()`.
foundation uses valtron, not tokio; valtron could *poll* the future but can't satisfy
deno_core's tokio-runtime-context requirements, so a tokio runtime is unavoidable.
The embedded JS therefore runs on a small **current-thread tokio runtime** (time
driver only) *inside* the test driver — confined to the `wasm-embedded-js` feature,
never touching foundation's valtron production runtime.

## 5. Work breakdown

| # | Work | Output |
|---|------|--------|
| W1 | ✅ **DONE (green)**: `deno_core` 0.404 (rusty_v8 / V8 149, prebuilt) builds + links on `uat`; in-process isolate evals `1+1` → `2.0` and runs `WebAssembly.Module`/`Instance`. Code: `src/wasm/embedded_js.rs`, behind the `wasm-embedded-js` sub-feature (`dep:deno_core`, no `deno_crypto`; see §4.4). API note: deno_core 0.404 dropped `JsRuntime::handle_scope` — use the exported `deno_core::scope!(scope, rt)` macro. | go/no-go on the build → **GO** |
| W2 | ✅ **DONE (green)**: `build_runtime()` composes `deno_core` + `deno_web` + `deno_webidl` with `FsModuleLoader` (the harness stages to a temp dir, so relative imports resolve from disk — no custom loader needed). `run_module()` drives the event loop on a current-thread **tokio** runtime (deno_core's reactor/timers are tokio — `reactor_tokio.rs`; not optional). Test `loader_runs_relative_import_graph_with_web_globals` proves relative ESM + `TextEncoder`/`TextDecoder` + `setTimeout` + `console` in-process. **Key findings:** `deno_console`/`deno_url` are deprecated (folded into `deno_web`); `deno_web` ships globals as `lazy_loaded_js` IIFEs that are NOT auto-installed — a tiny bootstrap globalizes them via `Deno.core.loadExtScript("ext:deno_web/08_text_encoding.js")` etc.; the `extension!` macro's init fn is `init()` (not `init_ops_and_esm`). | loader + runtime |
| W3 | ✅ **DONE (green)**: `foundation_fwt` extension with `op_fwt_read_file` (`Uint8Array`, replaces `node:fs`/`fetch`) + `op_fwt_report` (captures result JSON into a `ReportSink` in `OpState`). `runner.mjs` feature-detects the ops; `run_staged_harness()` evaluates it and reads the report → `HarnessReport`. Test `embedded_runs_sample_fixture_end_to_end` runs the real `fwt_sample.wasm` fixture's 5 cases (3 pass, 1 fail, 1 ignored, `should_panic` inverted) in-process. (Also globalized `URL` from `00_url.js`.) | green sample |
| W4 | ✅ **DONE (green)**: `run_deno` rebuilt on `run_staged_harness` → `RunOutcome` (`exit_code` from failed count); gated `#[cfg(wasm-embedded-js)]` with a clear `EmbeddedRuntimeDisabled` error otherwise. Removed `run_host` + `HostRuntimeNotFound`. (`deno.rs` is the *wasm-bindgen interop* shell-out — left intact; that output needs full Deno, out of the owned-harness scope.) | embedded runner |
| W5 | ✅ **DONE**: removed the `node` runner (`run_node`) + the `Node` CLI subcommand; `runner.mjs` no longer uses `node:fs`/`process.exit`/`Deno.exit`; `InitType::Node` → `Owned`; `mise` `test:wasm-testbed:node` → `:deno` (embedded). | deno-only |
| W6 | ✅ **DONE (green)**: `fwt_runner_tests` run the sample crate (built to wasm32) through the embedded runtime — no external node/deno — gated `wasm-embedded-js`; the node-availability skip is gone. | tests |
| W7 | ✅ **DONE**: README "Zero-install JS testing (spec-44)" section + `wasm-embedded-js` feature row; this spec → IMPLEMENTED. | docs |

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

1. ~~**`deno_core` + extensions vs `deno_runtime`**~~ — **DECIDED: `deno_core` +
   extensions.** `deno_runtime` is unusable here: its `deno_crypto` pins
   `aes =0.8.3`, conflicting with turso's `aes 0.8.4` in the shared workspace lock
   (§4.4). `deno_core` + chosen extensions avoids it; cost is no `crypto.subtle`
   (deferred, §4.4).
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
