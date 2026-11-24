---
feature: "HTTP Runtime"
description: "Deep foundation_http cases on embedded Deno, headless Chromium and workerd: the web bridge, the Cloudflare bridge, the serve-writer path, the response builders and the wasm streams, plus the native pure-Rust wasm module suite that CI never runs."
status: "done"
priority: "high"
depends_on: ["00-executors"]
estimated_effort: "large"
created: 2026-11-15
last_updated: 2026-11-23
author: "Main Agent"
model: "sonnet"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# HTTP Runtime

_Status: **done and merged** as `c12e94aec`, feature 02 of spec 88. Written in ASD-STE100 Simplified Technical English
(STE). Code and identifiers stay as they are._

**Abbreviations.** Cloudflare (CF); JavaScript (JS); WebAssembly (wasm); Web standard (web);
Simplified Technical English (STE); continuous integration (CI); request for comments (RFC);
Cross-Origin Resource Sharing (CORS).

## 1. Goal

Prove `foundation_http` on the three hosts:

| Host | Harness | What it proves |
|---|---|---|
| embedded Deno | wasm-bindgen | the web bridge, the serve-writer path, the streams |
| headless Chromium | wasm-bindgen | the same, with a real DOM and a real `Request` |
| workerd | wasm-bindgen | the CF bridge with a real `env` object |

The transport cases use the wasm-bindgen harness, because their wasm module imports wasm-bindgen
glue through `foundation_netio` (spec 88 section 2.7). The pure-Rust cases also run on the owned
harness, from a second package built without the fetch client. The owner asked for both harnesses
where the crate allows it (spec 88 section 4, D2). T6 builds and measures that package.

The feature has a second job: `foundation_http`'s `wasm-bindgen-http` and `wasm-test` features
are named by no CI run (#570), so the bridge and the three pure-Rust wasm test files never
compile or report. This feature makes them run.

## 2. Story

| ID | Actor | Trigger | Result |
|---|---|---|---|
| S1 | a case | sends a GET through `WasmHttpApp::handle_request` | status, headers and body are the ones the handler set |
| S2 | a case | sends a POST with a body | the handler reads the body text and its length |
| S3 | a case | sends a request that matches no route | 404, with no panic |
| S4 | a case | sends the wrong method for a known path | 405 |
| S5 | a case | sends `/items/a%2Fb` | the handler sees `/items/a/b` decoded |
| S6 | a case | a middleware returns a response | the handler never runs and the response is the middleware's |
| S7 | a case | dispatches through `CfHttpApp::handle_request` with `env` undefined | the route answers, and a handler that wants `DB` finds no binding |
| S8 | a case | runs on workerd with the real `env` | a handler that wants `DB` finds the real local D1 |
| S9 | a case | writes a body over the configured limit | 413, and the limit is a config field |
| S10 | a case | writes a response over many chunks through `WasmStream` | the chunks parse in order |
| S11 | a maintainer | change the router's 405 fallback to 404 | `wrong_method_returns_405` turns red on every host (R14) |

## 3. Bounded context

### 3.1 Inside

| Entity | Meaning | Owner (one writer) |
|---|---|---|
| `integration/wasm/` | the new standalone wasm-bindgen crate | this feature |
| `WasmHttpApp` and `CfHttpApp` | turn a `web_sys::Request` into a dispatch and a `Response` | `foundation_http/src/wasm/bridge/` |
| `wasm::dispatch` | the two dispatch traits | `foundation_http/src/wasm/dispatch.rs` |
| `serve_writer` | the handler-buffer path (`handle_request`) | `foundation_http/src/wasm/serve_writer.rs` |
| `WasmResponse`, `WasmStream` | response building and raw-chunk parsing | `foundation_http/src/wasm/{response,stream}.rs` |
| the router, middleware, handlers | the shared request path | `foundation_http/src/shared/` |

Neither surface is private; every case calls the public path. This feature writes only
`integration/wasm/` and the CI or mise lines it needs. A defect the cases prove gets fixed in
`foundation_http`, with a case that fails on the old code.

### 3.2 Outside

| Thing | Owner | This feature touches it? |
|---|---|---|
| the runners | feature 00 | no; it calls them |
| the `/echo` endpoint | feature 00 | no; these cases dispatch in memory |
| the streaming inbound body (#560) | spec 85 task T4 | no; see 6.6 |
| the connectrpc dispatch over http | feature 03 | no |

### 3.3 Verbs

| Verb | Kind | Effect |
|---|---|---|
| `handle_request` | story verb | dispatch one `web_sys::Request` and return a `web_sys::Response` |
| `dispatch` | story verb | dispatch a `SimpleIncomingRequest` with an explicit context |
| `route` | entity verb | add one method and path to the app's router |
| `set` | entity verb | write a status, a header or a body on a connection |
| `parse` | entity verb | split a raw byte stream into a status, headers and a body |
| `answer` | entity verb | produce one response for one request |

## 4. Rules

### 4.1 Law worksheet

```
Entity: one dispatch
Conserved value:    one request -> exactly one response
Bound:              every case ends within its stated limit; no case waits on the network
Inverse pairs:      request / response; route add / dispatch
Idempotent verbs:   a GET that reads no state
Ordering pairs:     middleware registration before dispatch; route add before dispatch
Monotone values:    the recorded middleware order
Freeze points:      the response does not change after the handler returns
Links to other entities: the handler's writes, the connection's reads, the response's status

Entity: a response body
Conserved value:    bytes written == bytes read (no loss, no duplication)
Bound:              the body limit is a config field; a body over it is refused
Inverse pairs:      open / finish on a stream
Idempotent verbs:   finish twice is not a second write
Ordering pairs:     every chunk in write order
Monotone values:    the byte count
Freeze points:      the body closes once it is finished
Links to other entities: the chunk order, the parse order, the content length
```

### 4.2 Invariants

- **I1.** One request produces exactly one response, and its status matches the handler's.
- **I2.** A handler sees the decoded path and the method the caller sent.
- **I3.** The middleware chain runs in registration order, and a short circuit stops it.
- **I4.** Every body limit is a configuration field with a documented default (root rule 8).
- **I5.** A case that needs a CF binding runs only where the binding is real; elsewhere it
  prints why it did not run and does not fail (see 6.6).
- **I6.** A `Ctx` insert made during one request is absent in the next (context fork).
- **I7.** Every case names its host, its case and the first unexpected value on failure (R3).

### 4.3 Must have

- the new standalone crate with one test target (R10, R11);
- a case for each row of the table in 6.3;
- the CF bridge on all three hosts, with the real-binding case on workerd;
- the three native `wasm-test` files run by CI (#570);
- the `wasm-bindgen-http` lib check for wasm32 in CI;
- the R14 break recorded.

### 4.4 Must do

- build each app with `HttpApp::with_ctx` and a deterministic bundle where the case reads the
  clock (root rule 11);
- use `web_sys::Request` and `web_sys::Response`, not a hand-made struct (R1);
- bound every stream read and every dispatch;
- give each case its own app, so route tables never leak between cases.

### 4.5 Must not

- must not stub `fetch`, `Request` or `Response`;
- must not assert on a body it reads with a method that buffers the whole stream when the case
  is about streaming;
- must not add a second `tests/*.rs` file to the bindgen crate (R10);
- must not add a `#[cfg(test)] mod tests` to a source file (root rule 7);
- must not make a private item public only for a test (R13); both surfaces are reachable.

## 5. Acceptance scenarios

```gherkin
Scenario: A GET reaches its handler
  Given an app with a GET route on "/hello" that sets status 200 and body "hi"
  When the case sends a real web_sys::Request for "/hello"
  Then the response status is 200
  And the response body text is "hi"

Scenario: An unknown path is a 404
  Given the same app
  When the case sends a GET for "/missing"
  Then the response status is 404

Scenario: A known path with the wrong method is a 405
  Given the same app
  When the case sends a POST for "/hello"
  Then the response status is 405

Scenario: A middleware short-circuit stops the handler
  Given an app whose first middleware returns 401
  When the case sends any request
  Then the response status is 401
  And the handler's counter is still zero

Scenario: A handler reads its request body
  Given an app with a POST route that echoes the body length
  When the case sends a 5-byte body
  Then the response body says the length is 5

Scenario: The CF bridge answers without bindings
  Given a CfHttpApp with a GET route that needs no binding
  When the case calls handleRequest with env set to undefined
  Then the response status is 200

Scenario: The CF bridge sees the real D1 binding
  Given the workerd host
  When the case calls handleRequest with the worker env
  Then a handler that asks for D1Database finds it

Scenario: A body over the limit is refused
  Given a ServeWriter app whose body limit is 16 bytes
  When the case dispatches a 17-byte body
  Then the response status is 413

Scenario: The suite proves it can fail
  Given the router's wrong-method fallback is changed to 404
  When each host runs the suite
  Then "wrong_method_returns_405" fails on that host
```

## 6. Technical breakdown

### 6.1 Tasks

| Task | What | Files | Done when |
|---|---|---|---|
| T1 | Create the standalone crate, one target, and a smoke case per host | `backends/foundation_http/integration/wasm/{Cargo.toml,src/lib.rs,tests/wasm.rs}` | the smoke case passes on Deno, Chromium and workerd |
| T2 | The web-bridge cases | `tests/wasm.rs` | cases 1-11 pass on all three hosts |
| T3 | The CF-bridge cases | `tests/wasm.rs`, `src/lib.rs` (the env helper) | cases 12-15 pass on Deno and Chromium, and 14 passes on workerd |
| T4 | The serve-writer, stream and response cases | `tests/wasm.rs` | cases 16-20 pass on all three hosts |
| T5 | CI: the crate's checks and the native `wasm-test` run | `scripts/ci/ci.py`, `ci/packages.toml`, `mise.toml` | CI builds the crate for wasm32 and runs `cargo test -p foundation_http --features wasm-test` |
| T6 | The owned-harness package: gate `wasm-fetch` behind a crate feature, build a second package without it, measure the imports, run the pure-Rust cases on the owned harness | `backends/foundation_http/Cargo.toml`, `backends/foundation_http/src/wasm/serve_writer.rs`, `backends/foundation_http/integration/wasm_owned/{Cargo.toml,src/lib.rs,tests/owned.rs}`, `scripts/ci/ci.py`, `ci/packages.toml`, `mise.toml` | the module has no `__wbindgen_` import and its cases pass on Deno and Chromium; a case that keeps a glue import stays on the wasm-bindgen harness, and T6 records why |

Order: T1 before the rest. T2, T3 and T4 are three groups of cases in one file; write them in
that order so a broken bridge is found before the cases that depend on it. T5 is CI. T6 extends
CI with the second package.

### 6.2 T1: the crate

Copy the layout of `foundation_db/integration/wasm_cf`, which is the working precedent:

```
backends/foundation_http/integration/wasm/
├── Cargo.toml
├── src/lib.rs        # shared helpers only, no cases
└── tests/wasm.rs     # every case, one target
```

`Cargo.toml`:

```toml
[package]
name = "foundation_http_wasm_tests"
version = "0.0.1"
publish = false
edition = "2024"

[dependencies]
foundation_http = { path = "../..", features = ["wasm-bindgen-http"] }
foundation_netio = { path = "../../../foundation_netio", default-features = false, features = ["wasm-fetch"] }
foundation_core = { path = "../../../foundation_core", default-features = false, features = ["std", "js-wasmbindgen"] }
wasm-bindgen = "=0.2.126"
js-sys = "=0.3.103"
web-sys = { version = "=0.3.103", features = ["Request", "RequestInit", "Response", "Headers", "ReadableStream", "ReadableStreamDefaultReader", "console"] }

[dev-dependencies]
wasm-bindgen-test = "=0.3.76"

[lib]
crate-type = ["cdylib", "rlib"]

[workspace]
```

The pins match the CLI the testbed installs (`mise.wasm.toml`: wasm-bindgen 0.2.126). The
crate is outside the root workspace for the reason R11 gives: the parent's dev-dependencies do
not build for wasm32.

`src/lib.rs` holds three helpers:

- `app_with_routes()` — builds `HttpApp::<Arc<dyn WebServe>>::new_web()`, adds the routes, and
  returns `WasmHttpApp::from_app(app)`. Routes are added **before** the wrap, because
  `WasmHttpApp::app()` returns a shared reference. For a case that needs a deterministic
  bundle, build `HttpApp::with_ctx(ctx)` the same way; `from_app` keeps the app's context.
- `request(method, url, headers, body)` — a `web_sys::Request` built with
  `Request::new_with_str_and_init` and a `RequestInit`.
- `body_text(resp)` — `JsFuture::from(resp.text())`, awaited.

Every case is `#[wasm_bindgen_test] async fn`. Each case builds its own app.

### 6.3 T2: the web bridge

`WasmHttpApp::handle_request(&self, req: Request)` (`bridge/web.rs:84`) turns the request into
a `SimpleIncomingRequest` (`request_from_web`), forks the app context, dispatches and returns a
`web_sys::Response`. The cases:

| # | Case | What it proves | Oracle |
|---|---|---|---|
| 1 | `get_route_returns_status_headers_and_body` | status, header and body round trip | example |
| 2 | `post_body_reaches_the_handler` | the handler reads the body text and its byte length | example |
| 3 | `unknown_path_returns_404` | the router's miss path | expected outcome |
| 4 | `wrong_method_returns_405` | the router's method miss | expected outcome |
| 5 | `path_parameter_is_decoded` | `%2F` arrives as `/` | example |
| 6 | `middleware_short_circuit_returns_401` | the handler is not called | expected outcome + counter |
| 7 | `middleware_runs_in_registration_order` | the recorded order equals the registration order | model |
| 8 | `duplicate_response_headers_keep_their_order` | two `Set-Cookie` values stay in order | example |
| 9 | `handler_panic_becomes_a_500` | a panicking handler does not abort the host | fault |
| 9c | `handler_panic_after_a_yield_becomes_a_500` | a panic in a later poll is caught too | fault |
| 10 | `request_fork_keeps_ctx_inserts_private` | an insert in request A is absent in request B | expected outcome |
| 11 | `injected_clock_reaches_the_handler` | a simulated clock in `Ctx` is what the handler reads | expected outcome |

Case 9 needs care: a panic inside the dispatch either becomes a 500 or unwinds into the host.
Measure which. If it unwinds, that is a defect: a panicking handler must not take down the
host (rule 9: file it and fix it, or gate the case with the issue).

**Measured 2026-11-23.** A panic does not become a 500. It aborts the wasm module
(`RuntimeError: unreachable`), and the wasm-bindgen-test host reports the case as failed. A
`wasm32-unknown-unknown` build uses `panic = "abort"`, so `catch_unwind` cannot answer 500.

**Fixed 2026-11-24 (#742).** The bridge polls each dispatch inside a guarded JavaScript call
(`wasm/panic_guard.rs`). The guard catches the trap, answers 500, and a second request still
reaches the app. The guard leaks the trapped future: dropping it corrupts the allocator, so
the next allocation traps. Cases 9, 9b and 9c need no feature gate.

Case 10 uses `Ctx::store`/`get` on a value the handler inserts, then a second request that must
not see it.

### 6.4 T3: the CF bridge

`CfHttpApp::handle_request(&self, req: Request, env: JsValue)` (`bridge/cf.rs:93`) calls
`request_from_cf`, forks the context, runs `extract_cf_bindings` (which stores `env.DB` when it
exists, `cf.rs:61-65`) and dispatches. Cases:

| # | Case | What it proves | Oracle |
|---|---|---|---|
| 12 | `cf_dispatch_routes_and_answers` | a route answers with `env` undefined | example |
| 13 | `cf_dispatch_without_a_binding_stores_none` | a handler that asks for `D1Database` finds none, and does not panic | expected outcome |
| 14 | `cf_dispatch_sees_the_real_db_binding` | on workerd, a handler finds the real local D1 | example |
| 15 | `cf_conn_into_response_preserves_status_and_headers` | `CfConn::into_response` keeps both | example |
| 9b | `cf_handler_panic_becomes_a_500` | the Cloudflare entry has the same panic boundary (issue #742) | fault |

Case 14 reads `globalThis.__CF_TEST_ENV__`, the global the testbed's wrangler worker sets
(`bindgen-wrangler/worker.js`; the same contract `foundation_db/integration/wasm_cf/src/lib.rs`
uses). On a host without it, the case prints
`skipped: __CF_TEST_ENV__ is unset (not the workerd host)` and returns. That is the one
host-conditional case (I5); say so in its doc comment.

`CfConn::into_response` returns `Result<web_sys::Response, JsError>` (`wasm/cf_conn.rs:64`).
Build a `CfConn` from a handler that sets a status, a header and a body.

### 6.5 T4: the writer path, the streams and the response

`serve_writer::handle_request(app, req)` (`wasm/serve_writer.rs:18`) runs the middleware chain
and the router on a `HttpApp<Arc<dyn ServeWriter>>` and returns a `WasmResponse`.
`HttpApp::new_writer()` and `route_writer::<H>(method, path)` build it.

| # | Case | What it proves | Oracle |
|---|---|---|---|
| 16 | `serve_writer_round_trip` | status, header and body arrive as a `WasmResponse` | example |
| 17 | `run_middleware_writer_sees_mutations` | a middleware's header insert reaches the handler | expected outcome |
| 18 | `serve_writer_body_over_the_configured_limit_returns_413` | the limit is a field, and 413 is its answer | example (limit 16, body 17) |
| 19 | `wasm_stream_parse_raw_chunks_in_order` | `WasmStream::parse_raw` splits status, headers and body, in order | example |
| 20 | `wasm_response_copies_status_headers_and_body` | the builder's values are the response's values | example |

Case 18 needs a body limit on the writer path. If `foundation_http` has none, add one: a field
on the writer app's config with a builder method and a documented default (root rule 8), and
the case proves it. Name the default's source (nginx `client_max_body_size`, 1 MiB, is a good
comparison). This is the only production change the feature expects; keep it small.

**Measured 2026-11-23.** There was none. The field is `HttpApp::max_writer_body_bytes`, with
`with_max_writer_body_bytes`. Its default is 1 MiB, the nginx `client_max_body_size` default.
A streaming body keeps `max_request_bytes`, because its length is unknown before it is read.

Case 19 builds a raw byte buffer — a status line, headers, a blank line, a body in three
writes — and asserts the parsed parts. `WasmStream` is a byte buffer with an optional parsed
result (`wasm/stream.rs:8-18`).

### 6.6 What this feature does not test

- **A streamed response body.** `WebConn::set_body` takes a `Vec<u8>` today. Spec 85's sink is
  the path for that; this feature asserts the response's parts, not its chunk timing.

**Correction, 2026-11-23.** The streaming inbound body landed before this feature (spec 85):
`request_from_web` and `request_from_cf` now build a `WasmByteStream` and do not call
`req.text()`. The suite therefore reads a **streamed** request body, one host turn per
`Data::Retry`, with a 1,000-turn bound. The first attempt used `collect_string_async`. It
passed on Deno and Chromium but hung the workerd host until the 300 s request timeout, because
`AsyncSendSafeBody` with no read-wake source self-wakes and the reader task never ran. Issue
#749 records the netio defect and the fix (attach a `ReadWakeSource` from `WasmByteStream`).
The suite does not read a whole stream into memory for a streaming assertion: case 2 sends a
5-byte body and checks its text and length.

Both limits stay recorded here so a reader does not think the suite forgot them.

### 6.7 T5: CI and the native suite

Three `foundation_http` test files are gated on `wasm-test` and run natively:
`tests/wasm_response_tests.rs`, `tests/wasm_stream_tests.rs` and `tests/wasm_server_tests.rs`
(#570). Add to the `wasm-tests` package build step:

```
cargo check --target wasm32-unknown-unknown -p foundation_http --features wasm-bindgen-http
cargo check --tests --target wasm32-unknown-unknown \
  --manifest-path backends/foundation_http/integration/wasm/Cargo.toml
cargo fmt --manifest-path backends/foundation_http/integration/wasm/Cargo.toml --check
```

The first is a **lib** check: a package-level `--tests` check cannot compile for wasm32
because the dev-dependency `foundation_testing` pulls `socket2` (#576). The new crate is
checked with `--tests`, because it has its own workspace.

And to the `test` step of the package, or to `foundation_http`'s Linux run:

```
cargo test -p foundation_http --features wasm-test
```

Add the two `#[wasm_bindgen_test]` runs:

```
mise run test:wasm-bindgen:http:deno     # test bindgen-deno backends/foundation_http/integration/wasm
mise run test:wasm-bindgen:http:web      # test bindgen-web  backends/foundation_http/integration/wasm
```

The wrangler run is `test bindgen-wrangler backends/foundation_http/integration/wasm`.

### 6.8 T6: the owned-harness package

The owned harness needs a module with no wasm-bindgen import (spec 88 section 2.7). The import
enters through `foundation_netio`'s `wasm-fetch` feature, which the wasm target dependency turns
on (`Cargo.toml:46-49`). Six steps, in order:

1. Add the crate feature `wasm-fetch` to `foundation_http`. It turns on
   `foundation_netio/wasm-fetch`. Keep it on inside `wasm-bindgen-http`, so every current build
   keeps the fetch client.
2. Put each use of a netio type behind `#[cfg(feature = "wasm-fetch")]`. The request type at
   `src/wasm/serve_writer.rs:8` is the first one. A type that cannot compile without netio stays
   in the gated code. Do not stub it.
3. Create the second package. Features unify inside one package, so the second module needs its
   own manifest. Copy the layout of `foundation_core/integration/wasm_tests`, the working owned
   crate:

   ```toml
   [package]
   name = "foundation_http_wasm_owned_tests"
   version = "0.0.1"
   publish = false
   edition = "2021"

   [dependencies]
   foundation_http = { path = "../..", features = ["wasm-test"] }
   foundation_wasm = { path = "../../../foundation_wasm", features = ["web"] }
   foundation_macros = { path = "../../../foundation_macros" }
   foundation_context = { path = "../../../foundation_context", features = ["host-abi"] }
   foundation_compact = { path = "../../../foundation_compact", default-features = false, features = ["std", "host-abi"] }

   [lib]
   crate-type = ["cdylib"]

   [workspace]
   ```

   The package names no `wasm-bindgen` and no `foundation_netio`. A host function that the cases
   need comes from `abi::web::register_function` (spec 88 section 2.7).

4. Measure the module's imports before you promise a case:

   ```
   cargo build --manifest-path backends/foundation_http/integration/wasm_owned/Cargo.toml \
     --target wasm32-unknown-unknown --profile uat
   wasm2wat <artifact> | rg '\(import '
   ```

   The second command must print no `__wbindgen_` line. A line that stays keeps its case on the
   wasm-bindgen harness, and this section records the symbol.
5. Write the cases under `#[wasm_test]` in the crate lib. They hold the same assertions as
   `tests/wasm_response_tests.rs`, `tests/wasm_stream_tests.rs` and `tests/wasm_server_tests.rs`,
   which run natively today. The cases sit in `src/lib.rs`, not `tests/owned.rs`: the owned
   runner builds the package lib and reads its `__fwt_` exports (section 5 and rule R9), so a
   case in `tests/` never runs (measured 2026-11-23).
6. Run the package on the owned harness, and add both to the tasks and to CI:

   ```
   wasm-testbed deno    backends/foundation_http/integration/wasm_owned
   wasm-testbed browser backends/foundation_http/integration/wasm_owned --headless
   cargo check --tests --target wasm32-unknown-unknown \
     --manifest-path backends/foundation_http/integration/wasm_owned/Cargo.toml
   ```

A step that fails stops T6 with a report: the cases stay on the wasm-bindgen harness, and the
reason goes into this section and `LEARNINGS.md`. Do not fake a module with a shim.

**Measured 2026-11-23.** Every step passed. The module imports only four `abi` functions
(`host_fill_random`, `host_wall_clock_ms`, `host_monotonic_now_ms`, `host_report`); no
`__wbindgen_` import is present. `wasm2wat` is not installed on the host, so the import list
came from a small reader of the wasm import section; the method is recorded in `LEARNINGS.md`.
The owned suite runs 40 cases on Deno and 40 on Chromium, all green. The fetch client is off
through the new `wasm-fetch` crate feature, which `wasm-bindgen-http` keeps on. No source file
needed a `#[cfg(feature = "wasm-fetch")]` gate: the only netio types the pure-Rust modules use
(`shared::http`) build without the feature.

### 6.9 Fault table

| Step that fails | What the runner does | What the contributor sees | What is left behind |
|---|---|---|---|
| the crate does not build for wasm32 | the build step fails | the compiler error | nothing |
| a handler panics | the guarded poll answers 500 and logs the trap | the case name and the trap | the trapped future and its discarded frames stay allocated |
| a request is built with a bad URL | the case fails at construction | `Request::new` error and the URL | nothing |
| a case waits on its own response forever | the dispatch deadline fails it | the case name | the module instance is dropped |
| the worker has no binding | case 13 asserts `None` | nothing; the case passes | nothing |

**#578 is closed.** The review of feature 00 fixed the MIME type: `StaticFileHandler` answers
`.mjs` with `text/javascript` (`backends/foundation_http/src/native/handlers/static_file.rs`),
and `static_file_serves_mjs_as_a_module_script` in `tests/static_file_tests.rs` proves it. A
browser host may stage a module script again; the name `runner.js` is not a contract.

### 6.10 Defects the cases proved in `foundation_http`

Each fix keeps the case that failed on the old code. A fixed defect is closed in the change
that lands it.

| Case | Defect | Fix | Issue |
|---|---|---|---|
| 3 | A route miss rejected the JS promise; no 404 `Response` reached the caller | `dispatch_web` / `dispatch_cf` build a 404 `Response` | #744 |
| 4 | A wrong method could not be 405: the router returned `None` for both misses | `RouteOp::MethodNotAllowed`, `Router::dispatch_result` | #745 |
| 5 | `%2F` never decoded | `bridge::parse_request_uri` decodes only the path | #746 |
| 16 | The writer path routed on the full URL, so every route missed | route on `request_uri.path()` | #747 |
| 18 | No writer-path body limit | `HttpApp::max_writer_body_bytes` (1 MiB) | none (a new field) |
| 9 | A panic aborts the module | a guarded host poll answers 500 (`wasm/panic_guard.rs`) | #742 |
| - | The native HTTP/1 server answers 404 for a wrong method | `dispatch_result` plus `send_method_not_allowed` with `Allow`; the wasm entries carry `Allow` too | #743 |
| 2 | `AsyncSendSafeBody` self-wakes without a host yield on workerd | the suite yields per `Data::Retry`; netio keeps the defect | #749 |

## 7. Tests

| Law or scenario | Test | Oracle |
|---|---|---|
| I1, S1 | cases 1, 16, 20 | example |
| I2, S5 | case 5 | example |
| I3, S6 | cases 6, 7, 17 | expected outcome + model |
| I4, S9 | case 18 | example (17 > 16) |
| I5, S8 | case 14 | example, workerd only |
| I6 | case 10 | expected outcome |
| I7 | every failure message | review |
| R14 | the break in 6.3 | expected outcome per host |

In-memory dispatch has no atomicity to test. The fault table covers what is left: a failed
case names what happened and leaves nothing behind.

## 8. Verify

```bash
# build
cargo check -p foundation_http --target wasm32-unknown-unknown --features wasm-bindgen-http
env -u CARGO_PROFILE_DEV_CODEGEN_BACKEND -u CARGO_PROFILE_UAT_CODEGEN_BACKEND \
  cargo check --tests --target wasm32-unknown-unknown \
  --manifest-path backends/foundation_http/integration/wasm/Cargo.toml

# the native pure-Rust wasm module suite (#570)
cargo test -p foundation_http --features wasm-test

# the three hosts
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-deno backends/foundation_http/integration/wasm
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-web backends/foundation_http/integration/wasm
cargo run -p foundation_testbed --no-default-features --features wasm --profile uat \
  --bin wasm-testbed -- test bindgen-wrangler backends/foundation_http/integration/wasm

# hygiene
cargo fmt --check -p foundation_http
cargo fmt --manifest-path backends/foundation_http/integration/wasm/Cargo.toml --check
cargo clippy -p foundation_http --all-targets -- -D warnings
```

## 9. Done when

- [x] The crate builds for wasm32 and has one test target.
- [x] Twenty-two cases pass on Deno, Chromium and workerd. Case 14 passes on workerd and states
      why it does not run elsewhere. Cases 9, 9b and 9c answer 500 after a handler panic and
      prove that the host keeps serving (#742 closed).
- [x] `cargo test -p foundation_http --features wasm-test` is a CI step (#570 closed).
- [x] The owned package builds without the fetch client, the import measurement shows no
      `__wbindgen_` import, and its 40 cases pass on Deno and Chromium.
- [x] A body limit is a configuration field with a documented default (case 18).
- [x] The R14 break is recorded per host and reverted.
- [x] The two paths this feature does not cover are recorded in 6.6 with their owners.
- [x] `progress.md` and `LEARNINGS.md` name what landed and what is next.

---

_Created: 2026-11-15_
