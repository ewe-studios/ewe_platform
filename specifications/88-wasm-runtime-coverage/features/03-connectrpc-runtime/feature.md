---
feature: "ConnectRPC Runtime"
description: "Deep foundation_connectrpc cases on embedded Deno, headless Chromium and workerd: the router and its four stream types, the response-sink contract, the error model, the deadline through an injected clock, and H1Transport against each host's real fetch."
status: "todo"
priority: "high"
depends_on: ["00-executors"]
estimated_effort: "large"
created: 2026-11-15
last_updated: 2026-11-15
author: "Main Agent"
model: "sonnet"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# ConnectRPC Runtime

_Status: **todo**, feature 03 of spec 88. Written in ASD-STE100 Simplified Technical English
(STE). Code and identifiers stay as they are._

**Abbreviations.** Remote procedure call (RPC); Connect protocol (Connect); JavaScript (JS);
WebAssembly (wasm); HTTP/1.1 (H1); Simplified Technical English (STE); continuous integration
(CI); Cloudflare (CF).

## 1. Goal

Prove `foundation_connectrpc` on the three hosts:

| Host | What it proves |
|---|---|
| embedded Deno | dispatch, the sink contract, the error model, the deadline, and `H1Transport` over the real fetch |
| headless Chromium | the same, over the browser's fetch and with a real origin |
| workerd | dispatch, the sink contract, the error model and the deadline; the transport only if the measurement in 6.5 allows it |

The crate cannot use the owned harness (spec 88 section 2.7). Every case is a
`#[wasm_bindgen_test]`. The crate builds for wasm32 with `default-features = false, features =
["rpc"]`; the default `rpc_multi` turns on `foundation_core/multi`, which is a hard error on
wasm (measured 2026-11-15: the check finishes clean with the `rpc` feature set).

## 2. Story

| ID | Actor | Trigger | Result |
|---|---|---|---|
| S1 | a case | dispatches one unary request through the router | 200, a Connect envelope and the handler's message |
| S2 | a case | calls a procedure the router does not have | `unimplemented`, and no handler runs |
| S3 | a case | a handler returns an error | the Connect error frame carries the code and the message |
| S4 | a case | a server-stream handler emits three frames | the sink receives open, three chunks, then finish, in that order |
| S5 | a case | the sink fails on the second chunk | dispatch returns the error and the handler is not polled again |
| S6 | a case | the call deadline passes while the handler waits | `deadline_exceeded` reaches the client, on the injected clock |
| S7 | a case | two services share one router | each procedure reaches its own handler |
| S8 | a case | `H1Transport` calls the host's endpoint | the response decodes and the status is 200 |
| S9 | a case | the endpoint answers a non-200 status | the client returns a typed `ConnectError` |
| S10 | a case | a server stream crosses the transport | every frame arrives, in order |
| S11 | a maintainer | change `dispatch_to` to call `finish` before the last chunk | `server_stream_frames_arrive_in_order` turns red on every host (R14) |

## 3. Bounded context

### 3.1 Inside

| Entity | Meaning | Owner (one writer) |
|---|---|---|
| `integration/wasm/` | the new standalone wasm-bindgen crate | this feature |
| `Router` and `ConnectRpcHandler` | route table, `dispatch` and `dispatch_to` | `foundation_connectrpc/src/shared/router/` |
| `ResponseSink` and `CollectingSink` | the response contract | `shared/router/dispatch.rs:375,406` |
| `CallHooks` and `DeadlineTimer` | the cancel signal and the deadline | `shared/router/dispatch.rs:167,181` |
| `Client` and `H1Transport` | the client over a `DynNetClient` | `shared/client.rs`, `shared/transport/h1.rs` |
| the `/connect` route | the real endpoint on the testbed server | `foundation_testbed/src/wasm/server.rs` |

### 3.2 Outside

| Thing | Owner | This feature touches it? |
|---|---|---|
| the runners and `--filter` | feature 00 | no |
| the `/echo` route and `__FWT_BASE_URL__` | feature 00 | no; it adds a second route |
| `foundation_netio`'s fetch client | spec 85 | no; it uses it |
| `foundation_http`'s bridges | feature 02 | no; it uses the native server path |

### 3.3 Verbs

| Verb | Kind | Effect |
|---|---|---|
| `dispatch` | story verb | route one request and produce one response, in memory |
| `dispatch_to` | story verb | route one request into a sink, frame by frame |
| `open` / `chunk` / `finish` | entity verbs | write a streamed response |
| `whole` | entity verb | write a complete response |
| `call` | story verb | send one request through a transport and decode the answer |
| `sleep_until` | entity verb | resolve when a deadline passes |

## 4. Rules

### 4.1 Law worksheet

```
Entity: a dispatched call
Conserved value:    one request -> one response; frames written == frames produced
Bound:              every call ends by its deadline; every case has its own bound
Inverse pairs:      open / finish; send / receive
Idempotent verbs:   none (a call has one response)
Ordering pairs:     head before chunk; every chunk before finish; open exactly once
Monotone values:    the frame index
Freeze points:      the sink is closed after finish and refuses a later chunk
Links to other entities: the handler's frames, the sink's calls, the client's decode

Entity: a response sink
Conserved value:    bytes handed to the sink == bytes handed to the peer
Bound:              a slow sink slows the handler; it never buffers without limit
Inverse pairs:      nothing
Idempotent verbs:   none
Ordering pairs:     open, chunk*, finish OR one whole
Monotone values:    the byte count
Freeze points:      a failed sink stops the handler
Links to other entities: the handler's poll count, the sink's recorded calls
```

### 4.2 Invariants

- **I1.** A streamed response is `open`, then zero or more `chunk`, then `finish`, in that
  order, and never a `whole` after an `open`.
- **I2.** A failed sink stops the handler: no later frame is produced.
- **I3.** A missing procedure never runs a handler.
- **I4.** A deadline is enforced by a `DeadlineTimer` the caller supplies, against the clock in
  `Ctx` (root rule 11), never against a wall clock read inside the layer.
- **I5.** The transport advertises only what the host's fetch can do (`WASM_FETCH_CAPABILITIES`).
- **I6.** Every case names its host, its case and the first unexpected value (R3).

### 4.3 Must have

- the standalone crate with one test target (R10, R11);
- the `/connect` route on the testbed server, on the same origin as the suite;
- a recording `ResponseSink` in the crate, which keeps the call order and the bytes;
- the deadline case on an injected clock;
- the transport cases on the hosts whose fetch can reach the endpoint;
- the R14 break recorded.

### 4.4 Must do

- build the client with `ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec)`, as
  `tests/client_core_tests.rs:298` does, so no schema is needed;
- use the real `DynNetClient` from `HttpClientBuilder::new().build()`;
- give each case its own router and its own client;
- measure, do not assume, what workerd allows (6.5).

### 4.5 Must not

- must not stub `fetch` (R1);
- must not assert a wall-clock deadline; use a simulated clock (root rule 11);
- must not add a second `tests/*.rs` file (R10);
- must not add `#[cfg(test)] mod tests` to a source file (root rule 7);
- must not make a private item public for a test (R13).

## 5. Acceptance scenarios

```gherkin
Scenario: A unary call reaches its handler
  Given a router with one unary procedure
  When the case dispatches a request for it
  Then the response status is 200
  And the body is one Connect envelope with the handler's message

Scenario: A missing procedure runs no handler
  Given the same router
  When the case dispatches a request for "missing.Procedure"
  Then the response carries the "unimplemented" code
  And the handler's poll count is zero

Scenario: A handler error keeps its code
  Given a unary procedure that returns an error with code "invalid_argument"
  When the case dispatches it
  Then the Connect error frame carries "invalid_argument"
  And the message is the handler's message

Scenario: A server stream writes in order
  Given a server-stream procedure that emits three frames
  When the case dispatches it into a recording sink
  Then the sink's calls are open, chunk, chunk, chunk, finish
  And the concatenated chunks are the three frames

Scenario: A failed sink stops the handler
  Given a sink that fails on the second chunk
  When the case dispatches the server stream
  Then dispatch returns the sink's error
  And the handler produced no fourth frame

Scenario: A deadline ends the call
  Given a simulated clock and a handler that waits forever
  When the call's deadline passes on that clock
  Then the response carries "deadline_exceeded"

Scenario: The transport reaches the endpoint
  Given the testbed's /connect route and a real DynNetClient
  When the case calls the unary procedure through H1Transport
  Then the decoded response holds the handler's message

Scenario: The suite proves it can fail
  Given dispatch_to is changed to finish before the last chunk
  When each host runs the suite
  Then "server_stream_frames_arrive_in_order" fails on that host
```

## 6. Technical breakdown

### 6.1 Tasks

| Task | What | Files | Done when |
|---|---|---|---|
| T1 | Create the standalone crate and one unary case | `backends/foundation_connectrpc/integration/wasm/{Cargo.toml,src/lib.rs,tests/wasm.rs}` | the unary case passes on Deno, Chromium and workerd |
| T2 | The router, error and sink cases | `tests/wasm.rs` | cases 1-10 pass on all three |
| T3 | The deadline cases on an injected clock | `tests/wasm.rs`, `src/lib.rs` | cases 11-13 pass on all three |
| T4 | The `/connect` route on the testbed server | `foundation_testbed/src/wasm/server.rs`, `Cargo.toml` | `curl http://127.0.0.1:<port>/connect/…` returns a Connect response |
| T5 | The transport cases and the workerd measurement | `tests/wasm.rs`, `foundation_testbed/src/wasm/server.rs` | cases 14-17 pass on the hosts that can reach the endpoint, and the reason is recorded for the others |
| T6 | CI: the crate's checks and the three runs | `scripts/ci/ci.py`, `ci/packages.toml`, `mise.toml` | CI builds the crate and runs it on each host |

Order: T1, T2, T3 need no endpoint. T4 then T5. T6 last.

### 6.2 T1: the crate

```
backends/foundation_connectrpc/integration/wasm/
├── Cargo.toml
├── src/lib.rs        # codec, procedures, recording sink, request builder
└── tests/wasm.rs     # every case, one target
```

`Cargo.toml`:

```toml
[package]
name = "foundation_connectrpc_wasm_tests"
version = "0.0.1"
publish = false
edition = "2024"

[dependencies]
foundation_connectrpc = { path = "../..", default-features = false, features = ["rpc"] }
foundation_netio = { path = "../../../foundation_netio", default-features = false, features = ["wasm-fetch"] }
foundation_core = { path = "../../../foundation_core", default-features = false, features = ["std", "js-wasmbindgen"] }
foundation_context = { path = "../../../foundation_context", features = ["host-abi"] }
foundation_compact = { path = "../../../foundation_compact", default-features = false, features = ["host-abi"] }
wasm-bindgen = "=0.2.126"
js-sys = "=0.3.103"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
wasm-bindgen-test = "=0.3.76"

[lib]
crate-type = ["cdylib", "rlib"]

[workspace]
```

`src/lib.rs` holds:

- `IdentityCodec` — a `CodecFor<Vec<u8>>` that passes bytes through, copied in shape from
  `tests/client_core_tests.rs:298`;
- `RecordingSink` — a `ResponseSink` that appends `("open", bytes)`, `("chunk", bytes)`,
  `("finish", bytes)` and `("whole", bytes)` to a shared `Vec`, and can be told to fail on the
  n-th call;
- `request(service, procedure, method, body)` — a `SimpleIncomingRequest` with the Connect
  content type and a body;
- `ctx()` — `Ctx::host()` for the cases that do not read a clock, and a simulated bundle for
  the deadline cases.

Every case is `#[wasm_bindgen_test] async fn`. The dispatch paths return a `BoxFuture`; the
case awaits it. On wasm the future needs no executor beyond the host's own microtask loop,
which `foundation_core`'s `js-wasmbindgen` backend already drives for `#[valtron_bindgen]`;
if a case needs to drive a future explicitly, use `valtron::execute(...).into_collect_future()`
and await it, as the valtron skill shows.

### 6.3 T2: the router, the error model and the sink

`ConnectRpcHandler::dispatch(&self, app_ctx, request) ->
BoxFuture<'static, SimpleOutgoingResponse>` (`shared/router/dispatch.rs:97`) collects the
response in memory. `dispatch_to(&self, app_ctx, request, body, sink, hooks)` streams it
(`:145`).

| # | Case | What it proves | Oracle |
|---|---|---|---|
| 1 | `unary_round_trip_through_the_router` | 200 and one Connect envelope | example |
| 2 | `unknown_service_returns_unimplemented` | code `unimplemented`, handler never polled | expected outcome + counter |
| 3 | `wrong_method_reports_the_documented_status` | the status the layer documents for a bad method | example, measured first |
| 4 | `handler_error_keeps_its_code_and_message` | the error frame's code and message | example |
| 5 | `server_stream_frames_arrive_in_order` | open, chunk×3, finish; bytes equal the frames | model |
| 6 | `server_stream_stops_when_the_sink_fails` | dispatch returns `Err`, no fourth frame | fault |
| 7 | `unary_uses_whole_and_never_open` | the sink's call list is exactly one `whole` | model |
| 8 | `client_stream_reads_every_frame` | three request frames reach the handler in order | example |
| 9 | `bidi_echoes_in_order` | three echoes in request order | example |
| 10 | `two_services_keep_their_procedures_apart` | each procedure reaches its own handler | example |

Case 3: measure the answer first and write it down (R7). Do not guess a status.

The recording sink implements `whole`, `open`, `chunk` and `finish` with
`Box::pin(std::future::ready(Ok(())))` bodies, like `CollectingSink` does, and records before
it returns. `ResponseSink: Send` is satisfied on wasm32 because the target is single-threaded.

### 6.4 T3: the deadline on an injected clock

`CallHooks::with_timer(&dyn DeadlineTimer)` attaches a timer (`:199`). `DeadlineTimer::sleep_until`
receives the deadline and the `Ctx` (`:167-172`), so the test supplies a timer that reads the
simulated clock from the context. The frame:

1. build a `Ctx` with the deterministic bundle (simulated clock), as
   `specifications/84-context-injection` describes;
2. build a `DeadlineTimer` whose `sleep_until` awaits that clock;
3. set a deadline already in the past for case 11, and one in the future for case 12;
4. dispatch a handler that never resolves;
5. assert `deadline_exceeded` for the past deadline, and that the future deadline lets the
   handler finish.

| # | Case | Oracle |
|---|---|---|
| 11 | `past_deadline_ends_the_call` | expected outcome (`deadline_exceeded`) |
| 12 | `future_deadline_lets_the_handler_finish` | example |
| 13 | `handler_sees_the_remaining_deadline` | example (the value the handler reads) |

If `DeadlineTimer` cannot read the simulated clock (for example it uses `Instant::now()`
inside), that is a defect against root rule 11: file it and fix it.

### 6.5 T4 and T5: the endpoint and the transport

**T4 — the route.** `foundation_testbed` already serves the suite's files from a native
`HttpApp::new_serve()` with `StaticFileHandler` on `/*` (`wasm/server.rs:70-76`). Add a route
on `/connect/*` before it:

- `RpcRoute` implements `foundation_http::native::serve::Serve`;
- its `serve` returns `ConnectionResult::Async(Box::pin(async move { … }))`;
- inside, it calls `ConnectRpcHandler::dispatch(ctx, req)`, renders the response with
  `foundation_http::native::writer::render_message` (`native/writer.rs:349`), writes the bytes
  to the connection and returns `Keep`;
- the handler holds one service with three procedures: `Unary`, `ServerStream` and `Fail`. The
  codec is JSON, so a client can read the response without a schema.

Add `foundation_connectrpc` to `foundation_testbed`'s dependencies (native only; the wasm
fallback feature does not need it). Bump `foundation_testbed`'s version for the new public
route type if it is exported.

Same origin matters: the browser suite loads its files from this server, so a fetch to
`/connect/…` on the same origin needs no CORS header. Do not add a second server.

**T5 — the client.** `H1Transport::new(client)` takes a `DynNetClient`
(`shared/transport/h1.rs:47`); `HttpClientBuilder::new().build()` gives the wasm
`FetchHttpClient` (`network_client.rs:440-453`). Build the client:

```rust
let codecs = ProcedureCodecs::<Vec<u8>, Vec<u8>>::only(IdentityCodec);
let client = Client::new(
    Arc::new(H1Transport::new(HttpClientBuilder::new().build())),
    &format!("{base}/connect/case.CaseService/Unary"),
    codecs,
    ClientOptions::default(),
)?;
```

| # | Case | Hosts | Oracle |
|---|---|---|---|
| 14 | `h1_transport_unary_over_the_host_fetch` | Deno, Chromium, workerd if allowed | example (200 and the handler's message) |
| 15 | `h1_transport_error_status_is_typed` | the same | expected outcome (`ConnectError` with the code) |
| 16 | `h1_transport_server_stream_delivers_frames` | the same | example (three frames, in order) |
| 17 | `wasm_fetch_capabilities_match_the_transport` | all | expected outcome (the constant equals `transport.capabilities()`) |

`__FWT_BASE_URL__` (feature 00 T6) is the base. For workerd, feature 00 sets it to the
worker's own origin, so the transport cases need a second global: the testbed passes the
Rust endpoint as `__FWT_RPC_BASE_URL__` in the worker template, and the case reads it.

**Measure the workerd path first.** A workerd `fetch` to `127.0.0.1:<port>` may be refused.
Run one probe case and record the answer:

- **allowed**: run cases 14-16 on workerd too, and say so in `LEARNINGS.md`;
- **refused**: run 14-16 on Deno and Chromium only, print
  `skipped: workerd cannot fetch the host loopback (<error text>)` on workerd, and file an
  issue with `severity: medium` that says what a future service binding would need.

Do not fake the endpoint to make workerd green. A green case on a stub is the defect this
whole specification exists to remove.

### 6.6 Fault table

| Step that fails | What the runner does | What the contributor sees | What is left behind |
|---|---|---|---|
| the crate does not build for wasm32 | the build step fails | the compiler error | nothing |
| the default feature set is used | the build fails on `foundation_core/multi` | the `compile_error!` text | nothing |
| a handler panics | the case fails | the case name and the panic | the module instance is dropped |
| the sink fails | dispatch returns `Err`; case 6 asserts it | the sink's error | nothing |
| the endpoint is not up | the transport case fails | the URL and the fetch error | the server thread is stopped by feature 00's guard |
| the deadline never fires | the case's own bound fails it | the case name and the bound | nothing |

## 7. Tests

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, S4 | cases 5, 7 | model (the recorded call list) |
| I2, S5 | case 6 | fault |
| I3, S2 | case 2 | expected outcome + counter |
| I4, S6 | cases 11-13 | model (simulated clock) |
| I5 | case 17 | expected outcome |
| I6 | every failure message | review |
| R14 | the break in 6.3 | expected outcome per host |

## 8. Verify

```bash
# build (the rpc feature set is the wasm one)
cargo check -p foundation_connectrpc --target wasm32-unknown-unknown \
  --no-default-features --features rpc
env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND -u CARGO_PROFILE_UAT_CODEGEN_BACKEND \
  cargo check --tests --target wasm32-unknown-unknown \
  --manifest-path backends/foundation_connectrpc/integration/wasm/Cargo.toml

# the route
cargo test -p foundation_testbed

# the three hosts
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-deno backends/foundation_connectrpc/integration/wasm
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-web backends/foundation_connectrpc/integration/wasm
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-wrangler backends/foundation_connectrpc/integration/wasm

# hygiene
cargo fmt --check -p foundation_connectrpc -p foundation_testbed
cargo fmt --manifest-path backends/foundation_connectrpc/integration/wasm/Cargo.toml --check
cargo clippy -p foundation_connectrpc -p foundation_testbed --all-targets -- -D warnings
```

## 9. Done when

- [ ] The crate builds for wasm32 with the `rpc` feature set and has one test target.
- [ ] Cases 1-13 pass on Deno, Chromium and workerd.
- [ ] The `/connect` route answers on the testbed server, on the same origin as the suite.
- [ ] Cases 14-16 pass on Deno and Chromium; on workerd they pass or carry a named reason and
      an issue.
- [ ] Case 17 passes on all three.
- [ ] The deadline cases run on a simulated clock, and no layer code reads a wall clock.
- [ ] The R14 break is recorded per host and reverted.
- [ ] `progress.md` and `LEARNINGS.md` name what landed and what is next.

---

_Created: 2026-11-15_
