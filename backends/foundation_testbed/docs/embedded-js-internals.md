# Embedded JS — internals & decisions

The mechanics that aren't obvious from the code, and the reasoning behind the
choices. Useful when debugging, bumping deno, or revisiting a decision.

## How `deno_web` globals actually load (`lazy_loaded_js` + `loadExtScript`)

`deno_web` registers its JS in two ways. A handful is eager `esm`; the Web API
classes we want (`TextEncoder`, timers, `URL`, `console` helpers, …) are
**`lazy_loaded_js`** — included in the binary but **not evaluated at startup**.

These files are **not ES modules**. They're IIFEs in the old deno style:

```js
(function () {
  const { core, primordials } = __bootstrap;   // provided by deno_core
  const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
  /* … define TextEncoder, TextDecoder … */
  return { TextEncoder, TextDecoder, /* … */ };  // ← captured by loadExtScript
})();
```

In the full `deno_runtime`, a big bootstrap (`99_main.js`) loads these and assigns
the classes to `globalThis`. We don't have that bootstrap (it's part of the runtime
we deliberately don't use), so we replicate the *minimum*: call
`Deno.core.loadExtScript("ext:deno_web/<file>.js")`, which evaluates the IIFE and
returns its export object, then assign onto `globalThis`. The Rust side is
`op_load_ext_script` (deno_core `modules/map.rs`); `loadExtScript` is exposed on
`globalThis.Deno.core`. Crucially, because we never run `99_main.js`,
`globalThis.__bootstrap` (with `core` + `primordials`) is still present when those
IIFEs read it — so they just work.

This is why the bootstrap is tiny and why adding a global is a one-liner
([extending → web global](./extending-the-runtime.md#adding-a-web-global-from-deno_web)).

### `deno_console` and `deno_url` are deprecated

Don't add them. Both are now empty stub crates ("DEPRECATED: use `deno_web`") — the
console and URL implementations were folded into `deno_web`. `console` is installed
by **`deno_core`** itself (`01_core.js` sets `globalThis.console`); `URL` comes from
`deno_web`'s `00_url.js` via the bootstrap. The composed extension set is exactly
`deno_core` + `deno_web` + `deno_webidl`.

## The `aes` / `deno_crypto` conflict

The single most consequential decision: **`deno_core` + extensions, not the full
`deno_runtime`** — forced by a dependency clash, not preference.

- Every `deno_crypto` version (0.258–0.265) hard-pins `aes = "=0.8.3"` (an *exact*
  requirement). `deno_runtime` pulls `deno_crypto`.
- The workspace already resolves `aes 0.8.4` via
  `foundation_db → turso → turso_core → aes-gcm`. **Every** turso version that uses
  `aes` requires `aes ^0.8.4`; the only turso releases without it (0.1.0–0.1.4)
  predate its `aes` dep entirely — a non-starter downgrade.
- `0.8.3` and `0.8.4` are in the same `0.8.x` compatibility band, which Cargo
  unifies to **one** version. `=0.8.3 ∩ ≥0.8.4 = ∅` → unresolvable.

Why the obvious escapes don't work:

- **"Different crates, different versions?"** Cargo *does* keep semver-incompatible
  versions side by side (the lock has multiple `aes` lines for `0.8.x` vs
  `0.9.0-rc`). But an *exact* pin inside the *shared* `0.8.x` band can't be unified
  with `≥0.8.4`. Had `deno_crypto` written `^0.8.3` it would have floated to 0.8.4.
- **"`foundation_testbed` doesn't even use turso."** True — but a workspace has one
  `Cargo.lock`, and the root `ewe_platform` binary depends on *both*
  `foundation_testbed` and `foundation_db`. So even `cargo build -p foundation_testbed`
  must resolve the whole-workspace lock, and it can't.
- **"Make `deno_runtime` optional so it only resolves for that crate?"** Cargo
  resolves optional deps too (the lock must be valid for all feature combinations);
  an unused-but-declared `deno_runtime` still forces the clash.

Composing only the extensions we need drops `deno_crypto` and the workspace
resolves. The cost — no `crypto.subtle` — is tracked in spec §4.4 with a re-add path
([extending → crypto](./extending-the-runtime.md#adding-web-crypto-cryptosubtle)).

## Why tokio, not valtron

`deno_core`'s event loop and timers are **tokio**, concretely:

- `reactor_tokio.rs` builds the timer reactor on `tokio::time::Sleep` /
  `sleep_until`. `deno_web`'s `setTimeout`/`op_defer` route through deno_core's
  `createTimer` → this reactor.
- `jsruntime.rs` calls `tokio::runtime::Handle::try_current()`; `tasks.rs` uses
  tokio `spawn_blocking`.

valtron could *poll* the top-level `run_event_loop` future, but the instant
deno_core arms a timer (`tokio::time::sleep_until`) it needs a tokio time driver
entered on the thread, or it panics ("no reactor running"). Driving it through
valtron would mean *also* standing up and entering a tokio runtime — valtron would
add a layer with zero benefit. So `run_staged_harness` uses a **current-thread tokio
runtime, time driver only** (`new_current_thread().enable_time()`), the minimal
executor deno_core requires. It's confined to `wasm-embedded-js` and never touches
foundation's valtron production runtime. (current-thread `block_on` also handles the
`!Send` deno_core futures without a `LocalSet`.)

## The result contract (why ops, not stdout)

The old shell-out runner printed lines and set a process exit code; Rust parsed
stdout + read the code. In-process there *is* no subprocess and no `process.exit` /
`Deno.exit` (neither global exists in our runtime). So the runner reports through
`op_fwt_report(json)`: Rust captures the structured result (`passed`/`failed`/
`ignored`/`output`) directly into `OpState` (a cloned `Rc<RefCell<Option<String>>>`
the driver reads after the loop drains), and `HarnessReport::exit_code()` derives the
verdict. No parsing, no exit-code plumbing — engine-agnostic and robust.

The same `runner.mjs` still works in a real browser: it feature-detects
`Deno.core.ops` and falls back to `fetch` (for bytes) + the `#output` summary line
(the verdict Playwright reads). One script, two hosts.

## API drift notes (deno_core 0.404)

Things that bit us and will bite the next bump:

| Expected (older) | Actual (0.404) |
|------------------|----------------|
| `JsRuntime::handle_scope()` | removed — use the exported `deno_core::scope!(scope, rt)` macro |
| `extension!` → `init_ops_and_esm()` | the generated init fn is `init()` |
| `#[op2]` with `state: &mut deno_core::OpState` | must be a **bare** `OpState` ident (the macro matches the token) |
| op returning bytes | `Result<deno_core::convert::Uint8Array, E>` where `E: JsErrorClass` (e.g. `deno_error::JsErrorBox`); `std::io::Error` not accepted directly |
| `resolve_path` | re-exported as `deno_core::resolve_path` (from `deno_path_util`) |

## File map (mechanics)

| Symbol | Role |
|--------|------|
| `BOOTSTRAP_GLOBALS` | the JS that `loadExtScript`s deno_web modules → `globalThis` |
| `build_runtime_with(extra)` | composes the runtime + runs the bootstrap; `build_runtime()` = `with(vec![])` |
| `foundation_fwt` (extension!) | registers `op_fwt_read_file` + `op_fwt_report`, seeds `ReportSink` into `OpState` |
| `ReportSink(Rc<RefCell<Option<String>>>)` | the cell the report op writes; the driver keeps a clone |
| `tokio_runtime()` | the current-thread + time executor for the event loop |
| `load_entry` / `evaluate` | resolve+load the ESM entry, then `mod_evaluate` + `run_event_loop` + await |
| `run_staged_harness` | the harness driver: build runtime (+ `foundation_fwt`), run `runner.mjs`, read the report |
