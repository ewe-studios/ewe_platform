# Embedded JS runtime — overview & architecture (spec-44)

The `wasm-embedded-js` feature runs the owned `#[wasm_test]` harness's JavaScript
**in-process**, inside the `wasm-testbed` process itself, on an embedded V8 engine.
No `node`, no `deno`, no `cargo install` — `cargo build` links the engine and you
get JS execution for free.

This page explains *what* it is, *why* it exists, and *how* the pieces fit. For
hands-on use see **[running JS & wasm tests](./running-js-and-wasm-tests.md)**; to
add capabilities (globals, ops, **crypto**) see
**[extending the runtime](./extending-the-runtime.md)**; for the deep mechanics and
the decisions behind it see **[internals](./embedded-js-internals.md)**.

## What problem it solves

Before this, the owned harness shelled out to an external JS host: `node runner.mjs`
or `deno run runner.mjs`. That meant every contributor and every CI runner had to
install a JS toolchain, and a missing/mismatched binary was a runtime failure after
a multi-minute wasm build.

The embedded runtime removes that entirely. The JS that validates a built wasm
module runs against a **real browser-parity host** (V8 + Web APIs), in the same
process, with the toolchain coming from `cargo` — hermetic and CI-trivial.

## Why V8 (via `deno_core`), and why *not* the full `deno_runtime`

We need an engine that runs **both** our host-runtime JS (`foundation-wasm.js`,
which implements the wasm host ABI a browser/Deno provides) **and** `WebAssembly`:

- **wasmtime** runs the wasm but has no JS — we'd have to reimplement the host ABI
  in Rust, defeating the purpose. ✗
- **QuickJS** is tiny but has weak/no `WebAssembly`. ✗
- **V8 via `deno_core`** has full `WebAssembly`, browser-parity semantics, and is
  what Deno already is. ✓

We compose **`deno_core` + `deno_web` + `deno_webidl`** rather than pulling the
whole `deno_runtime`. The full runtime drags in `deno_crypto`, which pins
`aes = "=0.8.3"`; the workspace already resolves `aes 0.8.4` (via
`foundation_db → turso`), and the two cannot coexist (`=0.8.3 ∩ ≥0.8.4 = ∅`). There
is no compatible turso version, so the full runtime is simply unusable here.
Composing only the extensions we need sidesteps the clash. The single capability we
give up is **Web Crypto** (`crypto.subtle`) — the harness doesn't use it; see
[extending the runtime → adding crypto](./extending-the-runtime.md#adding-web-crypto-cryptosubtle)
when you do need it. (Full reasoning: [internals](./embedded-js-internals.md#the-aes--deno_crypto-conflict).)

## The pipeline

`wasm-testbed deno <crate>` runs one owned loop end to end:

```
 build            discover           stage                     run (in-process)
 ┌────────┐  ┌──────────────┐  ┌──────────────────┐  ┌──────────────────────────┐
 │ cargo  │  │ read __fwt_  │  │ temp dir:        │  │ deno_core + deno_web V8   │
 │ build  │─▶│ exports from │─▶│  runner.mjs      │─▶│  load runner.mjs (ESM)    │
 │ wasm32 │  │ the module   │  │  foundation-wasm │  │  drive the event loop     │
 └────────┘  └──────────────┘  │  module.wasm     │  │  ops bridge bytes+result  │
                               │  cases.json      │  └────────────┬─────────────┘
                               └──────────────────┘               │
                                                          HarnessReport → RunOutcome
```

1. **build** (`build::run`) — compile the crate to `wasm32-unknown-unknown` (LLVM
   backend, no wasm-bindgen/wasm-pack).
2. **discover** (`fwt::discover_cases`) — read the `__fwt_*` exports to enumerate
   cases + their flags (async / should_panic / ignore), without running anything.
3. **stage** (`fwt_runner::stage` → `stage_from_wasm`) — write a self-contained
   harness into a temp dir: the embedded `foundation-wasm.js` runtime, the generic
   `runner.mjs`, the built `module.wasm`, and `cases.json`.
4. **run** (`embedded_js::run_staged_harness`) — evaluate `runner.mjs` on the
   embedded runtime, drive the event loop to completion, and read back the result.

Steps 1–3 are shared with the browser runner (`wasm-testbed web`); only step 4
differs (embedded V8 vs Playwright in a real browser).

## The runtime, in three parts

Everything embedded lives in `src/wasm/embedded_js.rs`.

### 1. Composition — `build_runtime()`

```rust
JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(FsModuleLoader)),  // resolve ./imports from the staged dir
    extensions: vec![
        deno_webidl::deno_webidl::init(),          // WebIDL bindings deno_web needs
        deno_web::deno_web::init(blob_store, None, false, broadcast_channel),
        deno_net::deno_net::init(None, None),      // net/TLS JS deno_fetch lazy-loads
        deno_fetch::deno_fetch::init(deno_fetch::Options::default()),
    ],
    ..Default::default()
})
// + install the rustls CryptoProvider (deno_tls) and put an allow-all
//   deno_permissions::PermissionsContainer into OpState (op_fetch reads it).
```

`deno_core` already installs `console` and provides the ES module loader and the
event loop. `deno_web` provides `TextEncoder`/`TextDecoder`, timers, `URL`, etc.;
`deno_fetch` (+ `deno_net`) provides `fetch`/`Request`/`Response`/`Headers`. Wiring
`fetch` pulled in a short chain of full-runtime assumptions (a TLS CryptoProvider, a
permissions container, a telemetry shim) — see
[extending → how `fetch` was added](./extending-the-runtime.md#a-worked-example-how-fetch-was-added).

### 2. The globals bootstrap

`deno_web` ships its Web APIs as **`lazy_loaded_js`** scripts that it does *not*
auto-install as globals (the full `deno_runtime` bootstrap normally does that). So
right after construction we run a tiny script that loads those modules on demand via
`Deno.core.loadExtScript(...)` and assigns the classes onto `globalThis`:

```js
const enc = Deno.core.loadExtScript("ext:deno_web/08_text_encoding.js");
const timers = Deno.core.loadExtScript("ext:deno_web/02_timers.js");
const url = Deno.core.loadExtScript("ext:deno_web/00_url.js");
const fetchMod = Deno.core.loadExtScript("ext:deno_fetch/26_fetch.js");
Object.assign(globalThis, {
  TextEncoder: enc.TextEncoder, /* …, */ setTimeout: timers.setTimeout,
  URL: url.URL, fetch: fetchMod.fetch, /* Headers/Request/Response */
});
```

This is the seam you extend to add more Web globals — see
[extending the runtime](./extending-the-runtime.md#adding-a-web-global-from-deno_web).

### 3. The ops bridge (`foundation_fwt` extension)

Two `#[op2]` functions form a direct Rust ↔ JS contract, replacing fragile
stdout/exit-code plumbing:

| Op | Direction | Purpose |
|----|-----------|---------|
| `op_fwt_read_file(path) -> Uint8Array` | JS → Rust → JS | read `module.wasm` / `cases.json` bytes (replaces `node:fs`/`fetch`) |
| `op_fwt_report(json)` | JS → Rust | hand the final result (passed/failed/ignored + output) to Rust, captured in `OpState` |

The runner **feature-detects** these (`globalThis.Deno?.core?.ops?.op_fwt_*`): under
the embedded runtime it uses them; in a real browser it falls back to `fetch` + the
`#output` summary line. The same `runner.mjs` therefore serves both hosts unchanged.

## The event loop is tokio — and why that's fine

`deno_core`'s event loop and timers are **tokio** (there is a `reactor_tokio.rs`;
the loop calls `tokio::runtime::Handle::current()`). This is not a choice we can
swap for valtron: valtron could *poll* the future, but the moment deno_core arms a
timer it needs a tokio time driver entered on the thread. So `run_staged_harness`
spins up a **current-thread tokio runtime (time driver only)** purely to drive the
engine. It is confined to this feature and never touches foundation's valtron
production runtime.

## Where things live

| Path | What |
|------|------|
| `src/wasm/embedded_js.rs` | the runtime, bootstrap, ops, drivers (`build_runtime`, `run_module`, `run_staged_harness`) |
| `src/wasm/fwt_runner.rs` | the owned loop: `stage`/`stage_from_wasm`, `run_deno` (embedded), `run_web` |
| `src/wasm/templates/fwt/runner.mjs` | the generic, host-agnostic test runner |
| `tests/fwt_runner_tests.rs` | end-to-end: sample crate → wasm32 → embedded run |
| `Cargo.toml` `[features] wasm-embedded-js` | the opt-in that links V8 |
| `specifications/44-embedded-deno-runtime/spec.md` | the design record + decisions |
