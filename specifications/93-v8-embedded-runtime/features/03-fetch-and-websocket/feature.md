---
feature: "Fetch and WebSocket on foundation_netio"
status: "pending"
priority: "high"
model: "sonnet"
phase: "3"
depends_on: ["02-web-apis"]
crates: ["foundation_testbed", "foundation_netio"]
---

# Feature 03: Fetch and WebSocket on foundation_netio

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Add `fetch`, `Headers`, `Request`, `Response` and `WebSocket` to the V8 runtime, backed by the
project's own network client, `foundation_netio`. After this feature, all 106 measured
`#[wasm_test]` cases pass on V8, including the one case that uses `fetch`.

The existing network tests stay. Do not delete a test (issue #139).

## 2. What the tests use

| Use | Where | Contract |
|---|---|---|
| `fetch(url, init)` | `backends/foundation_testbed/integration/module/src/lib.rs:72` (the case is `:98-116`) | `method: "POST"`, a headers object, a string body, and `response.status` plus `response.text()` |
| `fetch("data:…")` | `tests/embedded_js_tests.rs:64` | A `data:` URL with no network, with `Response` and `res.text()` |
| `WebSocket` constructor | `tests/embedded_js_tests.rs:85` | `typeof WebSocket === "function"`, `readyState === WebSocket.CONNECTING` |
| A refused WebSocket dial | `tests/embedded_js_tests.rs:110` | An `error` event then a `close` event with code 1006, within 5 seconds |
| `response.ok`, `status`, `body.cancel()` | `backends/foundation_wasm/runtime/foundation-wasm.js:2327-2347` | The host ABI checks the response before it streams a module |

The testbed starts its local echo server before the first case (`fwt_runner.rs:196`) and
writes its base URL into `harness.json` (`:197`, `:169-177`). The echo case reads
`globalThis.__FWT_BASE_URL__`.

## 3. Tasks

| Task | Work |
|---|---|
| T1 | Add `v8_runtime/net.rs`, or `v8_runtime/web/fetch.rs` and `v8_runtime/web/websocket.rs`. Keep the network code out of `event_loop.rs`. |
| T2 | `Headers`: `get`, `set`, `has`, `delete`, `append`, `entries`, `keys`, `values`, and iteration. Header names are case-insensitive. |
| T3 | `Request`: `url`, `method`, `headers`, `body`. Accept the `fetch` init fields the tests use: `method`, `headers`, `body`. |
| T4 | `Response`: `ok`, `status`, `statusText`, `headers`, `url`, `text()`, `json()`, `arrayBuffer()`, and a `body` object with `cancel()`. Read the status before the body. Stream the body; do not read it whole into memory (root `CLAUDE.md` rule 4). A body cap is configuration (decision D6). |
| T5 | `fetch(input, init)`: call the native client (`foundation_netio::HttpClientBuilder`, `backends/foundation_netio/src/network_client.rs:57`). Handle a `data:` URL in the fetch layer, with no network, so the test needs no external service. |
| T6 | `WebSocket`: the global class on the `foundation_netio` WebSocket client (`backends/foundation_netio/src/websocket/shared/client.rs:245`). Constants `CONNECTING` (0), `OPEN` (1), `CLOSING` (2), `CLOSED` (3). Properties `readyState`, `url`, `protocol`. Methods `send` (text and binary) and `close`. Events `open`, `message`, `close`, `error`, through `on*` properties and `addEventListener`. A failed dial sends `error` and then `close` with code 1006. A message cap is configuration. |
| T7 | `v8_runtime/ops.rs`: add an asynchronous op path. An op returns a promise, stores its resolver in the promise table, and completes when a result arrives on the channel. The event loop drains the channel and resolves the promise on the isolate thread. No V8 handle crosses a thread. |
| T8 | `v8_runtime/event_loop.rs`: add the pending-op count to the idle check. A pending fetch or WebSocket keeps the loop alive. |
| T9 | `src/wasm/fwt_runner.rs`: keep `server::start_serving_with_echo` (`:196`) and `write_harness_json` (`:197`) for the V8 path. |
| T10 | Port the three network tests unchanged: `fetch_data_url_round_trips_in_process` (`tests/embedded_js_tests.rs:64`), `websocket_global_present_in_process` (`:85`) and `websocket_connection_refused_delivers_close` (`:110`). |
| T11 | Add `v8_fetch_posts_the_echo_body` to `tests/v8_web_tests.rs`: a local server, a POST with a header and a body, and assertions on `status`, `text()` and the echoed header. |
| T12 | Add `v8_response_works_with_instantiate_streaming`: pass the runtime `Response` to `WebAssembly.instantiateStreaming` (`foundation-wasm.js:2342`). If V8 rejects a hand-written `Response`, record the result and keep the runner path on `WebAssembly.instantiate`, which already works (`runner.js`). |

## 4. The tests that prove the stage

| Test | What it proves |
|---|---|
| `fetch_data_url_round_trips_in_process` | `fetch`, `Response` and `text()` on a `data:` URL, with no network |
| `v8_fetch_posts_the_echo_body` | The real request path: method, headers, body, status and text against the local server |
| `websocket_global_present_in_process` | The `WebSocket` global and the `CONNECTING` state |
| `websocket_connection_refused_delivers_close` | A failed dial sends `error` then `close` with code 1006 |
| `v8_all_106_owned_cases_pass` | The full sample run, including `echo_endpoint_answers` (`integration/module/src/lib.rs:98-116`), with 106 of 106 cases |

The last test runs the four measured files: `foundation_core` (47), `foundation_http` (40),
`foundation_connectrpc` (13) and the sample (6). It asserts the same verdicts as the Deno run.
Build the comparison with both features: `--features "wasm-embedded-js wasm-embedded-v8"`.

## 5. Verify

```bash
cargo test -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --test v8_web_tests
cargo run -p foundation_testbed --no-default-features --features wasm-embedded-v8 \
  --profile uat --bin wasm-testbed -- deno backends/foundation_testbed/integration/module
MISE_ENV=wasm mise run test:wasm-testbed:v8
```

## 6. Done when

- The three ported network tests pass unchanged.
- `v8_fetch_posts_the_echo_body` passes against the local server.
- All 106 cases pass on V8, with the same verdicts as on Deno.
- A failed WebSocket dial gives code 1006 within 5 seconds.
- The fetch body is streamed and the status is checked before the body is read.
- No network test is deleted or marked ignored.
- The result of the `instantiateStreaming` test is recorded in `progress.md`.
