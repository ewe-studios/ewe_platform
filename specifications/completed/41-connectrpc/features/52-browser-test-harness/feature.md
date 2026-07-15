---
feature: "Wasm-bindgen browser test harness — re-export #[wasm_bindgen_test] from foundation_testbed so crates test their browser API bridges directly (no standalone test crate, no extra deps)"
description: "Fold the wasm-bindgen-test ecosystem into foundation_testbed: re-export the proc-macro + runtime glue, add a top-level `wasm-testbed browser <crate>` command, and let crates put cfg-gated #[wasm_bindgen_test] functions in their own tests/. Deletes the standalone foundation_wasm_testbed crate."
status: "completed"
priority: "high"
phase: 1
depends_on: ["51-unified-http-client-surface"]
estimated_effort: "medium"
created: 2026-07-11
---
# Feature 52: Browser test harness — wasm-bindgen in the testbed

> **Status: implemented (2026-07-11), with a deliberate divergence from the design
> below.** The design proposed a *plain re-export* of `#[wasm_bindgen_test]` from
> `foundation_testbed`. What shipped is richer: two purpose-built proc-macros in
> `foundation_macros` that own the valtron pool lifecycle. See *Implementation
> status* immediately below; the original design is retained for context.

## Implementation status — landed (diverged from design)

What actually shipped, verified in-tree:

- **`#[valtron_bindgen]`** (`foundation_macros/src/bindgen_test.rs`) — a one-step
  wasm-bindgen browser test that auto-inits the valtron pool and drives the body
  via `foundation_core::valtron::block_on_future` (async) or a plain call (sync),
  emitting a `__bindgen_env_<name>` marker. This is the richer replacement for the
  design's bare `pub use wasm_bindgen_test::wasm_bindgen_test;` — the macro exists
  precisely so browser tests get a live valtron pool without hand-rolled setup.
- **`#[valtron_wasm_test]`** (`foundation_macros/src/wasm_test.rs`) — the owned
  (`__fwt_`) wasm test path with valtron pool auto-init, complementing the bindgen
  path.
- **`foundation_testbed` owns the ecosystem** — `src/bindgen.rs` plus the
  `wasm-testbed` bin dispatch (`browser`/`bindgen` handling); `foundation_wasm_testbed`
  standalone crate **deleted** (Step 5 done).
- **Tests relocated into the crate under test** — `foundation_netio/tests/wasm/`
  (`mod.rs` + `websocket.rs`) holds the WebSocket bridge tests; `open_websocket_task`
  on `WebSocketConnector` returns a `Box<dyn TaskIterator>` so the wasm bridge is
  exercisable (yields `Pending(Connecting)` via `next_status`). Step 4 done.
- **Docs updated** — testbed README + `running-js-and-wasm-tests.md` reflect the
  F52 macros + API (HEAD commit `3a15883cb`).

**Reconciled 2026-07-12.**
- **deno** command: shipped (`wasm-testbed deno <crate>` — owned tests in embedded Deno). ✅
- **browser** command: shipped (`wasm-testbed browser <crate>` — owned tests + Chromium CDP). ✅
- **bindgen** command: shipped (`wasm-testbed bindgen <crate>` — wasm-bindgen tests in Chromium). ✅
- **test auto-detect**: NOT shipped. The `test` command is legacy Mode-based
  (Web/Deno/Wrangler/BindgenWeb) — no auto-discovery of `__fwt_`/`__wbgt_` markers
  to run both in one session. Explicit commands cover every use case; auto-detect is
  not needed.
- **wasm-bindgen version**: constant `WASM_BINDGEN_VERSION = "0.2.126"` in
  `bindgen.rs`; no runtime check at CLI startup (non-critical — build-time
  Cargo.toml pins the version).
- **bindgen-web deprecation**: INTEROP warning on legacy `test bindgen-web` mode,
  not a formal deprecation notice (the mode still works, just not the default).

## Problem

Today testing a crate's wasm-bindgen browser bridge (e.g. foundation_netio's
`websocket/wasm/browser.rs`) requires creating a **separate standalone crate**
outside the workspace with its own `[workspace]`, `Cargo.lock`, and a half-dozen
pinned wasm-bindgen dependencies. The developer must:

1. Create `foundation_wasm_testbed/` (standalone crate with own `[workspace]`)
2. Pin `wasm-bindgen = "=0.2.121"`, `wasm-bindgen-test = "=0.3.45"`, `js-sys`, `web-sys`
3. Write `#[wasm_bindgen_test]` functions
4. Run `wasm-testbed test bindgen-web <path> --headless` (a sub-sub-command)

Compare this to the **owned** `#[wasm_test]` path:
1. Add `foundation_wasm` + `foundation_macros` as deps
2. Write `#[wasm_test]` functions
3. Run `wasm-testbed deno <crate>` (top-level command)

The wasm-bindgen path has too much ceremony. And crates that use web_sys
(foundation_netio's wasm client, foundation_browser's CDP driver) MUST test in a
real browser — the embedded Deno runtime has no DOM, no WebSocket global, no
`window`. Browser tests are the only way to exercise `web_sys::WebSocket`,
`web_sys::Request`, or any DOM API bridge.

## Design

### 1. foundation_testbed owns the wasm-bindgen ecosystem

`foundation_testbed` gains an optional `wasm-bindgen-test` feature that depends
on and **re-exports** the key macros and types:

```toml
# foundation_testbed/Cargo.toml
[features]
wasm-bindgen-test = ["dep:wasm-bindgen-test", "dep:wasm-bindgen"]

[dependencies]
wasm-bindgen = { version = "=0.2.121", optional = true }
wasm-bindgen-test = { version = "=0.3.45", optional = true }
```

```rust
// foundation_testbed/src/wasm/bindgen.rs (new)
pub use wasm_bindgen_test::wasm_bindgen_test;     // proc-macro attribute
pub use wasm_bindgen_test::wasm_bindgen_test_configure;  // macro_rules!
```

A consumer crate then only needs:
```toml
[dev-dependencies]
foundation_testbed = { path = "../foundation_testbed", features = ["wasm-bindgen-test"] }
```

And writes:
```rust
#![cfg(target_arch = "wasm32")]
use foundation_testbed::wasm_bindgen_test;
use foundation_testbed::wasm_bindgen_test_configure;
wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn my_browser_test() { ... }
```

**How this works:** `#[wasm_bindgen_test]` is a proc-macro from `wasm-bindgen-test-macro`.
Its expansion references `::wasm_bindgen_test::...` by absolute path. Since
`wasm-bindgen-test` IS a transitive dependency (via `foundation_testbed`), the
expanded code resolves. The `wasm_bindgen_test_configure!` macro uses
`#[macro_export]`; `$crate` resolves to `wasm_bindgen_test` — also available
transitively. The consumer never names `wasm-bindgen-test` or `wasm-bindgen` directly.

### 2. Clean CLI naming

The four top-level commands tell you what test system they run, not just the runtime:

| Command | What it runs |
|---|---|
| `wasm-testbed deno <crate>` | `#[wasm_test]` (owned) in embedded Deno — no browser |
| `wasm-testbed browser <crate>` | `#[wasm_test]` (owned) served + Chromium CDP |
| `wasm-testbed bindgen <crate>` | `#[wasm_bindgen_test]` via wasm-bindgen CLI → Chromium CDP |
| `wasm-testbed test <crate>` | **Auto-detect** — discovers both `__fwt_` and `__wbgt_`, runs all in Chromium |

The zero-thinking `test` command:
1. `cargo build --tests --target wasm32-unknown-unknown` (captures both export systems)
2. Inspect the .wasm: count `__fwt_*` + `__wbgt_*` exports
3. Run `wasm-bindgen` if bindgen exports found
4. Stage a unified harness page that runs both test suites
5. Chromium CDP → poll `#output` → report combined results

### 3. Tests live in the crate under test

The standalone `foundation_wasm_testbed/` crate is deleted. Tests move into the
crates they exercise:

```
foundation_netio/tests/wasm/
├── mod.rs          # cfg(test) + cfg(target_arch = "wasm32") gate
├── websocket.rs    # #[wasm_bindgen_test] WebSocket bridge tests
└── http.rs         # (future) FetchHttpClient browser tests
```

Each test file uses `use foundation_testbed::{wasm_bindgen_test, wasm_bindgen_test_configure};`

### 4. Dev-dependency ergonomics

`foundation_netio`'s `Cargo.toml` gets the wasm-compatible dev-dep and
target-gates its native-only dev-deps (following foundation_core's pattern):

```toml
[dev-dependencies]
foundation_testbed = { path = "../foundation_testbed", features = ["wasm-bindgen-test"] }

[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
# native-only test deps (tokio, reqwest, smol, serial_test, etc.) moved here
```

The existing `[[test]]` targets (`quic`, `http3`) that require native features
are already gated behind `required-features` and won't resolve for wasm32 —
Cargo skips them.

### 5. Export namespace isolation

No collision risk. The two test systems use different prefixes, build modes,
and discovery mechanisms:

| | `#[wasm_test]` | `#[wasm_bindgen_test]` |
|---|---|---|
| Prefix | `__fwt_` | `__wbgt_` |
| Build | `cargo build` (lib only) | `cargo build --tests` |
| Discovery | `fwt.rs` via `foundation_codegen` | `wasm_test.rs` via `walrus` |
| Runtime | Embedded Deno (deno_core) | Chromium (CDP) |

Even in a `--tests` build that includes both, each discovery path filters on
its own prefix and ignores the other.

### 6. wasm-bindgen CLI version check

At the start of `wasm-testbed browser`, read the installed `wasm-bindgen --version`
and compare it against the crate version (`wasm_bindgen::VERSION` at compile time).
If they mismatch, print a clear error:

```
wasm-bindgen CLI version 0.2.128 does not match crate version 0.2.121.
Install: cargo install wasm-bindgen-cli --version 0.2.121
```

## Non-goals

- No change to `deno`/`web` owned subcommands
- No change to `test bindgen-deno` or `test bindgen-wrangler`
- No auto-install of `wasm-bindgen-cli` (one manual `cargo install` is fine)
- No wasm-bindgen deno interop (the existing embedded Deno runtime is the owned path)

## Implementation plan

### Step 1: Add `wasm-bindgen-test` feature to foundation_testbed

- `Cargo.toml`: add optional deps `wasm-bindgen = "=0.2.121"`, `wasm-bindgen-test = "=0.3.45"`, feature `wasm-bindgen-test`
- New file `src/wasm/bindgen.rs`: `pub use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};`
- Wire into `src/wasm/mod.rs`: `#[cfg(feature = "wasm-bindgen-test")] pub mod bindgen;`
- Re-export from `src/lib.rs` so consumers write `use foundation_testbed::wasm_bindgen_test;`

### Step 2: Add `browser` top-level subcommand

- CLI: add `Browser(BrowserArgs)` variant to `Command` enum
- `BrowserArgs`: `crate_path`, `--release`, `--features`, `--filter`, `--headless`
- New file `src/wasm/bindgen_runner.rs`: `run_browser(args) -> Result<RunOutcome>`
  - Preflight: check `wasm-bindgen` on PATH, verify version matches
  - Build: `build::run_with_tests(...)`
  - Bindgen: `wasm::run_wasm_bindgen_with_name(... BindgenTarget::Web ...)` to temp dir
  - Discover: `wasm_test::discover_tests(...)` on `_bg.wasm`
  - Stage: write `index.html` + `run.js` from templates
  - Serve + browser + poll (reuse existing `server.rs` + `browser.rs`)
- Wire into `wasm-testbed.rs` dispatch

### Step 3: Deprecate `test bindgen-web` mode

Leave the code but add a deprecation notice pointing to `wasm-testbed browser`.

### Step 4: Migrate WebSocket tests into foundation_netio/tests/

- Split netio's `[dev-dependencies]`: native-only deps to `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]`
- Add `foundation_testbed = { features = ["wasm-bindgen-test"] }` to `[dev-dependencies]`
- Create `tests/wasm/websocket.rs` with the two WebSocket bridge tests
- Create `tests/wasm/mod.rs` with `mod websocket;` gated behind `#[cfg(target_arch = "wasm32")]`
- No `[[test]]` entry needed — Cargo discovers `tests/wasm/mod.rs` automatically
- Gate netio's lib with: `#[cfg(target_arch = "wasm32")] mod tests_wasm;` — NO, better to not touch lib.rs. Just put it in tests/.

Actually — the issue is `wasm_bindgen_test_configure!(run_in_browser)` can only be called once per crate. Multiple `tests/` files would need this at the crate root. Better approach: put it in `tests/wasm/mod.rs` which is the module root for the `wasm` test target.

Wait, `tests/wasm/mod.rs` is discovered as a separate `[[test]]` target. Each `[[test]]` target is its own crate. So `wasm_bindgen_test_configure!(run_in_browser)` in that file applies to that test target only. Fine.

But actually, `--tests` builds a single binary that includes ALL `[[test]]` targets. wasm-bindgen-test's `__wbgt_` exports from all test modules end up in the same .wasm. The `cx.run(...)` in run.js runs them all. So multiple test files under `tests/wasm/` all contribute exports to the same .wasm binary. Good.

### Step 5: Delete foundation_wasm_testbed

- Delete `backends/foundation_wasm_testbed/`
- Remove `"backends/foundation_wasm_testbed"` from workspace root `Cargo.toml` exclude list

### Step 6: Verify

- `cargo run -p foundation_testbed --features wasm --bin wasm-testbed -- browser backends/foundation_netio --headless`
- The two WebSocket tests run in Chromium, output `test result: ok`
- Confirm existing `deno` and `web` subcommands still work
- Confirm existing `test deno` owned path still works

## Open questions

1. **`wasm-bindgen-cli` install**: Should `wasm-testbed browser` auto-detect and
   suggest the install command, or should it just fail with a clear message?
   **→ Recommend:** fail with clear message + exact install command.

2. **`wasm_bindgen_test_configure!(run_in_browser)` per test module**: Each
   `tests/wasm/mod.rs` test target calls this once. If a crate has multiple
   browser-test modules, they share one `wasm_bindgen_test_configure!`. Fine —
   one module root per test target. **→ Confirmed.**

3. **Dev-dep split in foundation_netio**: Moving native-only dev-deps to
   `[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]` is mechanical
   but touches many lines. **→ Mechanical, low-risk; foundation_core already does it.**

4. **Existing `test bindgen-web` mode**: Keep or delete? **→ Keep for one
   release with a deprecation warning, delete in follow-up.**
