---
feature: "HTTP Runtime"
this_file: "specifications/88-wasm-runtime-coverage/features/02-http-runtime/start.md"
spec: "specifications/88-wasm-runtime-coverage/requirements.md"
model: "sonnet"
branch: "spec/split-65"
created: 2026-11-15
---

# Start: HTTP Runtime

You own feature 02 of spec 88. Feature 00 must be merged before you start: you run its
runners and its `--filter`.

## Mission

Prove `foundation_http` on embedded Deno, headless Chromium and workerd through the
wasm-bindgen harness. Build one standalone crate, write twenty contract cases that use the
real `web_sys::Request`, `web_sys::Response` and the real worker `env`, and make CI run the
native pure-Rust wasm module suite that it never runs today (#570).

## Required reading, in this order

1. `requirements.md` sections 2.6, 2.7, 4 (D2), 5, 6 and 9.
2. `features/02-http-runtime/feature.md` in full.
3. `backends/foundation_db/integration/wasm_cf/` — the working precedent for a standalone
   wrangler test crate. Copy its shape, its pins and its `src/lib.rs` helper style.
4. These files, before you change them:
   - `backends/foundation_http/src/wasm/bridge/web.rs`
   - `backends/foundation_http/src/wasm/bridge/cf.rs`
   - `backends/foundation_http/src/wasm/serve_writer.rs`
   - `backends/foundation_http/src/wasm/{response,stream}.rs`
   - `backends/foundation_http/src/shared/app/mod.rs`
   - `backends/foundation_http/tests/wasm_web_serve_tests.rs`
   - `scripts/ci/ci.py:737-780`

## Owned paths

- `backends/foundation_http/integration/wasm/` — the new crate
- `backends/foundation_http/src/` — only for a defect a case proves, or the body limit of case
  18
- `scripts/ci/ci.py`, `ci/packages.toml`, `mise.toml` — only the steps this feature adds

You do not write the runners, `foundation_testbed` or `foundation_browser`. Report a needed
change there.

## Task order

| Task | One line | Run before you continue |
|---|---|---|
| T1 | the crate, its helpers and one smoke case per host | the smoke case green on all three |
| T2 | the eleven web-bridge cases | all green on all three |
| T3 | the four CF-bridge cases | 12, 13, 15 green everywhere; 14 green on workerd |
| T4 | the five writer, stream and response cases | all green on all three |
| T5 | CI: the two checks, the three runs, the native suite | `python3 scripts/ci/ci.py plan --package wasm-tests` shows them |

## Rules that decide your choices

| Rule | What it means for you |
|---|---|
| R1 | no fake host: the real `Request`, `Response` and worker `env` |
| R2 | every case names its oracle |
| R3 | a failure names the host, the case and the first unexpected value |
| R4 | every dispatch and every stream read has a bound |
| R6 | a case covers a full cycle and at least one edge |
| R8 | unique state per case; each case builds its own app |
| R10, R11 | one test target; own `[workspace]`, `cdylib` and `rlib`, pinned pins |
| R13 | no new public API for a test; both surfaces are reachable |
| R14 | the 405 break turns one named case red per host |
| root 7 | no inline `#[cfg(test)] mod tests` |
| root 8 | the body limit is a config field, never a `const` |
| root 11 | use `HttpApp::with_ctx` when a case reads the clock |
| root 12 | write in ASD-STE100 Simplified Technical English (STE) |
| root 13 | no AI attribution anywhere |

## Facts that save you time

- `WasmHttpApp` lives at `foundation_http::wasm::bridge::web::WasmHttpApp`; `CfHttpApp` at
  `...::bridge::cf::CfHttpApp`. Neither is re-exported at the root.
- Routes are added on `HttpApp::<Arc<dyn WebServe>>::new_web()` with
  `route_web::<H>(method, path)`, then the app is wrapped with `WasmHttpApp::from_app(app)`.
  `WasmHttpApp::app()` is a shared reference, so add routes first.
- `HttpApp::with_ctx(ctx)` is the deterministic constructor; `from_app` keeps that context.
- The bridge's request body is buffered with `req.text()` today (#560, spec 85 task T4). Use
  small text bodies; the limit case uses the writer path.
- `CfHttpApp::handle_request` stores `env.DB` when it exists (`bridge/cf.rs:61-65`); a missing
  binding is not an error at the bridge.
- A package-level `cargo check --tests --target wasm32` for `foundation_http` cannot build
  (#576: `foundation_testing` pulls `socket2`). The lib check is the gate.
- The wasm-bindgen CLI is 0.2.126. Pin `wasm-bindgen` and `web-sys` to match.

## Commands

```bash
cargo check -p foundation_http --target wasm32-unknown-unknown --features wasm-bindgen-http
cargo test -p foundation_http --features wasm-test
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-deno backends/foundation_http/integration/wasm
```

A crate outside the workspace needs the codegen variables unset:

```bash
env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND -u CARGO_PROFILE_UAT_CODEGEN_BACKEND cargo …
```

## Do not

- do not commit or push.
- do not stub a binding to make case 14 pass everywhere.
- do not read a whole stream to test streaming (R1 and the case's own oracle).
- do not add a case that needs the network. Every case dispatches in memory.

## Report

1. Per task: files changed, the command run, its result (one line each).
2. The test count per host and per runner.
3. Every defect a case proved, with the failing case and the fix.
4. The R14 record: the break, the case, the red output per host.
5. Anything you added to `foundation_http` beyond the crate, and why.
6. `git diff --stat` for your owned paths.

---

_Created: 2026-11-15_
