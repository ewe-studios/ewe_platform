---
feature: "HTTP Runtime"
description: "Deep foundation_http cases on embedded Deno, headless Chromium and workerd: the web bridge, the Cloudflare bridge, the serve-writer path, the response builders and the wasm streams, plus the native pure-Rust wasm module suite that CI never runs."
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
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# HTTP Runtime

_Status: **todo**, feature 02 of spec 88. Written in ASD-STE100 Simplified Technical English
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

The crate cannot use the owned harness: its wasm module imports wasm-bindgen glue through
`foundation_netio` (spec 88 section 2.7). Every case here is a `#[wasm_bindgen_test]`.

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

Order: T1 before the rest. T2, T3 and T4 are three groups of cases in one file; write them in
that order so a broken bridge is found before the cases that depend on it.

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
| 10 | `request_fork_keeps_ctx_inserts_private` | an insert in request A is absent in request B | expected outcome |
| 11 | `injected_clock_reaches_the_handler` | a simulated clock in `Ctx` is what the handler reads | expected outcome |

Case 9 needs care: a panic inside the dispatch either becomes a 500 or unwinds into the host.
Measure which. If it unwinds, that is a defect: a panicking handler must not take down the
host (rule 9: file it and fix it, or gate the case with the issue).

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

Case 19 builds a raw byte buffer — a status line, headers, a blank line, a body in three
writes — and asserts the parsed parts. `WasmStream` is a byte buffer with an optional parsed
result (`wasm/stream.rs:8-18`).

### 6.6 What this feature does not test

- **A streaming inbound body.** `request_from_web` and `request_from_cf` call `req.text()` and
  buffer the whole body (#560, high). Spec 85 task T4 replaces that with a stream. This
  feature's cases use small text bodies, and case 18 exercises the limit on the writer path,
  which is in-memory by design. When spec 85 lands, its own suite covers the streamed request.
- **A streamed response body.** `WebConn::set_body` takes a `Vec<u8>` today. Spec 85's sink is
  the path for that; this feature asserts the response's parts, not its chunk timing.

Both are recorded here so a reader does not think the suite forgot them.

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

### 6.8 Fault table

| Step that fails | What the runner does | What the contributor sees | What is left behind |
|---|---|---|---|
| the crate does not build for wasm32 | the build step fails | the compiler error | nothing |
| a handler panics | case 9 says what the host did | the case name and the panic | the module instance is dropped |
| a request is built with a bad URL | the case fails at construction | `Request::new` error and the URL | nothing |
| a case waits on its own response forever | the dispatch deadline fails it | the case name | the module instance is dropped |
| the worker has no binding | case 13 asserts `None` | nothing; the case passes | nothing |

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

- [ ] The crate builds for wasm32 and has one test target.
- [ ] Twenty cases pass on Deno, Chromium and workerd, except case 14, which passes on workerd
      and states why it does not run elsewhere.
- [ ] `cargo test -p foundation_http --features wasm-test` is a CI step (#570 closed).
- [ ] A body limit is a configuration field with a documented default (case 18).
- [ ] The R14 break is recorded per host and reverted.
- [ ] The two paths this feature does not cover are recorded in 6.6 with their owners.
- [ ] `progress.md` and `LEARNINGS.md` name what landed and what is next.

---

_Created: 2026-11-15_
