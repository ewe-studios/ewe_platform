---
feature: "ConnectRPC Runtime"
this_file: "specifications/88-wasm-runtime-coverage/features/03-connectrpc-runtime/start.md"
spec: "specifications/88-wasm-runtime-coverage/requirements.md"
model: "sonnet"
branch: "spec/split-65"
created: 2026-11-15
---

# Start: ConnectRPC Runtime

You own feature 03 of spec 88. Feature 00 must be merged before you start: you use its
runners, its `--filter` and its `__FWT_BASE_URL__` contract. If features 01 and 02 run at the
same time, they write different crates; you share only `foundation_testbed/src/wasm/server.rs`
with feature 00, which is already merged.

## Mission

Prove `foundation_connectrpc` on embedded Deno, headless Chromium and workerd. The router, the
response-sink contract, the error model and the deadline are pure in-memory paths. The
transport needs a real endpoint, so you add a `/connect` route to the testbed's server and make
`H1Transport` reach it over each host's real fetch. Measure what workerd allows before you
decide its coverage.

## Required reading, in this order

1. `requirements.md` sections 2.4, 2.6, 2.7, 4 (D2), 5, 6 and 9.
2. `features/03-connectrpc-runtime/feature.md` in full.
3. `specifications/85-wasm-fetch-transport/fetch.md` sections 1 and 5. Spec 85 may create this
   crate first. If `backends/foundation_connectrpc/integration/wasm/` exists when you start,
   extend it; do not create a second one, and say which spec created it in your report.
4. `backends/foundation_db/integration/wasm_cf/` — the working standalone-crate precedent.
5. These files, before you change them:
   - `backends/foundation_connectrpc/src/shared/router/dispatch.rs` (lines 97-210, 375-440)
   - `backends/foundation_connectrpc/src/shared/router/mod.rs`
   - `backends/foundation_connectrpc/src/shared/transport/h1.rs`
   - `backends/foundation_connectrpc/src/shared/transport/wasm.rs`
   - `backends/foundation_connectrpc/tests/client_core_tests.rs` (lines 280-400)
   - `backends/foundation_netio/src/network_client.rs` (lines 430-462)
   - `backends/foundation_testbed/src/wasm/server.rs`
   - `backends/foundation_http/src/native/writer.rs` (line 349)
   - `backends/foundation_http/src/native/handlers/static_file.rs`

## Owned paths

- `backends/foundation_connectrpc/integration/wasm/` — the new crate (or the one spec 85 made)
- `backends/foundation_connectrpc/src/` — only for a defect a case proves
- `backends/foundation_testbed/src/wasm/server.rs` and `foundation_testbed/Cargo.toml` — the
  `/connect` route only
- `scripts/ci/ci.py`, `ci/packages.toml`, `mise.toml` — only the steps this feature adds

You do not write the runners' templates, the browser driver or `foundation_http`'s wasm
bridges. Report a needed change there.

## Task order

| Task | One line | Run before you continue |
|---|---|---|
| T1 | the crate, the codec, the recording sink, one unary case | green on all three hosts |
| T2 | the router, error and sink cases (1-10) | all green on all three |
| T3 | the deadline cases on a simulated clock (11-13) | all green on all three |
| T4 | the `/connect` route on the testbed server | `curl` returns a Connect response |
| T5 | the transport cases (14-17) and the workerd measurement | green on Deno and Chromium; workerd either green or recorded |
| T6 | CI: the crate's checks and the three runs | `python3 scripts/ci/ci.py plan --package wasm-tests` shows them |

## Rules that decide your choices

| Rule | What it means for you |
|---|---|
| R1 | no fake host and no stubbed `fetch`; the transport cases hit the real server |
| R2 | every case names its oracle |
| R3 | a failure names the host, the case and the first unexpected value |
| R4 | every call has a bound |
| R5 | no case needs the network; the endpoint is on the host's own loopback |
| R10, R11 | one test target; own `[workspace]`, pinned `wasm-bindgen` |
| R13 | no new public API for a test |
| R14 | the sink-ordering break turns one named case red per host |
| root 4 | stream the frames; never buffer a streamed response to test it |
| root 7 | no inline `#[cfg(test)] mod tests` |
| root 11 | the deadline reads the injected clock; no `Instant::now()` in layer code |
| root 12 | write in ASD-STE100 Simplified Technical English (STE) |
| root 13 | no AI attribution anywhere |

## Facts that save you time

- The wasm feature set is `default-features = false, features = ["rpc"]`. The default
  `rpc_multi` turns on `foundation_core/multi`, which is a hard error on wasm.
- `foundation_connectrpc --target wasm32-unknown-unknown --no-default-features --features rpc`
  checks clean (measured 2026-11-15).
- `ConnectRpcHandler::dispatch` is a sync fn returning `BoxFuture<'static,
  SimpleOutgoingResponse>`; `dispatch_to` streams into a `Box<dyn ResponseSink>` and returns
  `BoxFuture<io::Result<()>>`, where `Err` is the sink's failure and cancels the call.
- `in_memory_body(request.body.take())` gives the body a dispatch can read without a
  connection (`dispatch.rs:241`).
- `ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec)` needs no schema
  (`tests/client_core_tests.rs:298`).
- `HttpClientBuilder::new().build()` returns the wasm `FetchHttpClient` on wasm32 with the
  `wasm-fetch` feature (`network_client.rs:440-453`).
- The testbed's server binds `127.0.0.1:<port>` in a thread and serves `/*`; add `/connect/*`
  before that route.
- `render_message(response) -> Result<Vec<u8>, String>` turns a `SimpleOutgoingResponse` into
  wire bytes (`foundation_http/src/native/writer.rs:349`).

## Do not

- do not commit or push.
- do not use the default feature set; it cannot build for wasm.
- do not fake the endpoint for workerd. Measure, then record.
- do not assert on a status you did not measure first (case 3).

## Report

1. Per task: files changed, the command run, its result (one line each).
2. The case count per host.
3. The workerd answer: allowed or refused, with the error text.
4. Whether spec 85 created the crate first.
5. Every defect a case proved, with the failing case and the fix.
6. The R14 record: the break, the case, the red output per host.
7. `git diff --stat` for your owned paths.

---

_Created: 2026-11-15_
