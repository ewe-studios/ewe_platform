# Spec 85 Progress: Wasm HTTP Streaming

_Last updated: 2026-11-23._

**Merged into master as `e5801ee13` on 2026-11-22.**
**The follow-up fixes merged as `0831fe1f0` on 2026-11-23.**

## Status at a glance

| Task | State |
|---|---|
| T1 — `WasmByteStream`, the streaming response body of the wasm client, `body_queue_chunks` | ✅ Done |
| T2 — `open_exchange` passes `self`; the exchange task calls that client; `WASM_FETCH_CAPABILITIES` | ✅ Done |
| T3 — `streaming_response_with(status, headers, body)` | ✅ Done |
| T4 — the streaming inbound body in both bridges, and the connectrpc `ResponseSink` | ✅ Done |
| T5 — `StubHttpClient` over `worker::Stub`, behind the `cloudflare` feature | ✅ Done |
| T6 — the `mise.wasm.toml` netio task and its `scripts/ci/ci.py` step, the standalone `foundation_connectrpc` test crate, and the fix for #565 | ✅ Done |

**Commits.** `4e305e091` (T1 and T2, plus the version bumps); `2a063c762` (T3 to T6).

## Current state (2026-11-22)

The code for all six tasks is on branch `spec/85-wasm-fetch`. It is not merged into master.

Verified on macOS 27.2 (aarch64), with wasm-bindgen 0.2.126 and Deno 2.9.7:

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy` over the four changed crates, `--profile uat --all-targets --no-deps` | pass; only the three workspace-manifest warnings remain |
| `cargo test` over the four changed crates, `--profile uat` | pass, except #485 and #466: this host cannot make a hanging dial |
| `cargo check --tests --target wasm32` of `foundation_connectrpc/integration/wasm` | pass |
| `wasm-testbed test bindgen-deno` of `foundation_netio --features wasm-fetch` | 10 of 10 pass |
| `wasm-testbed test bindgen-deno` of `foundation_connectrpc/integration/wasm` | 5 of 5 pass |
| `cargo check --lib --target wasm32 -p foundation_netio --features wasm-fetch,cloudflare` | pass |

The verification found and fixed six defects:

1. The standalone wasm test crate named `foundation_core` but did not declare it.
2. `ConnectSink::whole` and `open` returned a future that is not `Send`: a valtron `Stream`
   item lived across an await.
3. `StubHttpClient::send` declared `+ Send` on a future that holds a JavaScript promise.
4. The reader task did not cancel the JavaScript reader when the consumer closed the queue.
5. `WASM_FETCH_CAPABILITIES.request_streaming` changed, but `transport_tests` still asserted
   the old value.
6. The websocket test and the Deno host disagreed about `window` and about a dropped closure.

The workspace manifest still warns about three unused dependencies (`anyhow`, `fs_extra`,
`reqwest`). Issue #709 records them.

### Linux CI

Run with `scripts/ci/linux.sh` on the `ewestudios/ewe-platform-ci:latest` image
(`linux/amd64` on an arm64 Docker engine):

| Package | Result |
|---|---|
| `foundation_netio` | pass |
| `foundation_connectrpc` | pass |
| `foundation_http` | pass |
| `foundation_testbed` | pass |
| `wasm-tests` | pass; includes netio 10 of 10 and connectrpc 5 of 5 |

The first run exposed test-harness defects, not product defects:

- The script mounted the shared Git directory read-only, and the submodule link step
  writes `.git/config` (#590). Fixed in `bb5ba308f`.
- The HTTP/3 drivers busy-polled, and the body tests timed the wait from the response
  head (#716). Fixed in `7a6880976` and `be17f02fb`.
- The wasm wake tests used wall-clock windows too tight for an emulated host (#717).
  Fixed in `7a6880976` and `be17f02fb`.

## The capability map (2026-11-15)

Before the plan is reviewed, the owner asked for a map of what already exists across the
relevant `foundation_*` crates. `whats_currently_available.md` now measures eleven crate
groups on native, wasm32 and a Cloudflare Worker, with the symbol and the file and line for
each claim, an explicit list of absences, and a final reuse table.

State: every section is written, and a fresh agent has checked every claim against the code.
It checked 340 claims across the eleven sections and the synthesis. 19 rows were wrong or
uncertain: line drift, a wrong symbol for `StorageProvider`, a misdescribed `WasmOAuth`
branch, a wrong feature name for the D1 session store, and a missing response builder. All
are corrected. The map is ready for the plan review.

The check also added three facts the map missed:

1. `foundation_netio`'s `response` and `client` modules need the `wasm-fetch` feature
   (`foundation_netio/src/wasm/mod.rs:8`).
2. `foundation_http` has a second response builder, `WasmResponse` with `build_response`
   and `from_wasm` (`foundation_http/src/wasm/response.rs:5`), beside `CfConn::into_response`.
   It buffers too.
3. A wasm dispatcher for `ServeWriter` exists (`foundation_http/src/wasm/serve_writer.rs:18`),
   but no `web_sys::Request` bridge calls it.

Two findings already change the plan's premises:

1. `foundation_netio` and `foundation_wireguard` test targets are compile-checked on wasm
   only (`scripts/ci/ci.py:749-768`). T1 adds a wasm path to `foundation_netio`, so the plan
   must add an execution step for its tests, or the new code ships untested on wasm.
2. There is no Node wasm runner. The wasm runners are the `wasm-testbed` binary (Deno,
   Chromium, and `wrangler dev --local`). T6 is rewritten to them, and it is now the first
   task.

## Design corrections (2026-11-15)

1. The first draft added a `Fetcher` trait, a `GlobalFetcher`, a `FetchTransport`, a
   `FetchServe`, a `StubFetcher` and a `DoStubTransport`. The owner rejected it: "the core
   idea is `DynNetClient` is the platform agnostic way, and connectrpc already builds on it."
   All six types are gone. The specification now fills the gaps inside `HttpClient`,
   `DynNetClient`, `H1Transport` and `foundation_http`.
2. The first draft said fetch cannot stream a request body and collected it. That was wrong.
   `SendSafeBody::Stream` already becomes a JS `ReadableStream` through
   `valtron::js_stream::iterator_to_readable_stream`
   (`foundation_netio/src/wasm/client/client.rs:395`). The request path needs no change, so
   `H1Transport` is right to report `request_streaming: true`, and the stale constant is
   `WASM_FETCH_CAPABILITIES`.
3. The first draft defended a `FetchTransport<F>` because `Transport` requires
   `Send + Sync + 'static` and a JS handle is not `Send`. `H1Transport` already solves that:
   it holds a `DynNetClient` in `shared/` and has no target gate. A new transport repeats the
   solution.
4. `RuntimeFetch` left `CloudflarePlatform` (spec 77 section 6.0). The wrapper holds no HTTP
   client; a caller holds a `DynNetClient`.
5. The streaming read is not new work. `fetch_sse` already takes `resp.body()` and
   `spawn_stream_reader` already reads it into a bounded queue. T1 extracts that reader into
   `WasmByteStream` and uses it from `send_async`.

Issue #556 records the first three corrections, with the audit that found them. It is closed.
Issue #557 (high) records the whole-body read and closes with T1. Issue #558 (medium) records
the stale capability constant and closes with T2. Issue #559 (medium) records that
`open_exchange` ignores its client and closes with T2.

## Where this specification came from

The design lived in spec 83 feature 05 (`worker-and-web.md` sections 5.4, 5.5 and 6.2) as
tasks T19 and T21. Spec 77 feature 07 also planned its own body pump (about 80 lines) and
said it did not wait for spec 83.

On 2026-11-15 the owner gave the direction: one specification owns the plumbing, and both
consumers build on it. This specification is that owner, and it now owns four gaps rather
than a transport.

| Before | After |
|---|---|
| Spec 83 feature 05 T19 (`FetchServe`, `FetchTransport`) | Spec 85 T1 to T4 |
| Spec 83 feature 05 T21, transport part (`DoStubTransport`) | Spec 85 T5 (`StubHttpClient`) |
| Spec 77 feature 07 own body pump | nothing: `H1Transport` already streams |
| Spec 77 feature 08 own Worker entry details | `foundation_http`'s `HttpApp` plus spec 85's sink |

## Clear-eyes review (2026-11-15)

After the map was validated, the plan got a second read against it. The result:

| Was in the plan | Is true |
|---|---|
| `HttpClientBuilder::build()` at `network_client.rs:83` | `:440` |
| `streaming_response` at `wasm/response.rs:31` | `:33`; `sse_response` is at `:91` |
| `FetchHttpClient` at `client.rs:113` | `:56` |
| `NativeHttpClient` at `src/native/` | `src/http/http_client_impl.rs:50` |
| The wasm tests run under Node | There is no Node runner; the testbed runs Deno, Chromium and `wrangler dev --local` |
| `cargo test --target wasm32-unknown-unknown` runs them | It builds only; no runner executes a wasm `#[test]` binary |
| `foundation_connectrpc/tests/wasm/` exists | The directory does not exist, and the package cannot build its tests for wasm32 (`foundation_core` with `multi` is a hard error) |
| T1's code ships with tests that run | `foundation_netio`'s wasm suite is compile-checked only (`scripts/ci/ci.py:751-753`) |
| T6 is last | T6 is first: without the task and the CI step, T1 and T2 ship untested on wasm |

## The Deno runner, measured (2026-11-15)

The owner asked for a survey of the Deno runner in `foundation_testbed`. It has four modes, and
two of them are Deno:

| Mode | What it runs | CI |
|---|---|---|
| `deno <crate>` | the owned `#[wasm_test]` harness on the embedded Deno runtime (`deno_core` with `deno_web`, `deno_fetch` and `deno_websocket`), in process | `test:wasm-testbed:deno` |
| `test bindgen-deno <crate>` | `#[wasm_bindgen_test]` and `#[valtron_bindgen]` under the pinned Deno 2.9.7, with `--allow-read --allow-net` | `test:wasm-bindgen:deno` |

The netio suite ran on the second one. `wasm-testbed test bindgen-deno ./backends/foundation_netio
--features wasm-fetch` builds the package, finds five tests, and runs them. Four pass:

```
sse::sse_read_error_surfaces_failed_not_a_clean_end ........ ok
http_exchange_task::wasm_http_exchange_task_constructs ..... ok
websocket::fetch_client_is_a_websocket_connector ........... ok
websocket::open_websocket_refused_surfaces_close ........... ok
```

The fifth, `websocket::open_websocket_task_yields_pending_connecting_on_wasm`, hangs, and in the
full run it fails with an allocation error in the harness's result recording. #565 records it.

So netio needs a task and a CI step, not a new crate. `foundation_connectrpc` still needs a
standalone crate: its test build for wasm32 stops on the `multi` guard in
`foundation_core/src/lib.rs:20`, and it has 18 autodiscovered test targets.

The review also removed a risk of a third response builder. `foundation_http` has two wasm
builders (`CfConn::into_response` at `wasm/cf_conn.rs:64`, `WasmResponse` at
`wasm/response.rs:5`), and both buffer. The plan uses `foundation_netio`'s
`streaming_response_with`, the only one that takes a stream, and says so in section 4.4 of the
plan and in `fetch.md`.

Three issues came out of the review:

- #563 (medium): `foundation_netio`'s wasm suite never executes, and
  `foundation_connectrpc` has no wasm test target. T6 closes it.
- #564 (low): `mise.toml`'s `test:wasm:node` and `test:wasm:node-single` point at
  `./tests/integrations/`, which does not exist. Delete them.
- #565 (medium): one netio wasm test hangs under the Deno runner
  (`websocket::open_websocket_task_yields_pending_connecting_on_wasm`). T6 settles it.

Issue #557 and issue #558 got a correction comment: both said spec 85 adds a `FetchTransport`.
It does not.

### Follow-up fixes (2026-11-22)

Commit `4098f8259` closes the six open items:

1. #561: the JS stream pull is one future that awaits each `Pending` item. A
   `ReadableStream` does not pull again when a pull promise resolves without data, so the
   pull owns the wait loop. A new wasm test `js_stream_park` proves it; the core, netio and
   connectrpc wasm suites pass.
2. #485 and #466: the full-backlog dial helper tries 8, 64 and 256 fillers and returns
   `None` when the host cannot make a hanging dial. The test prints a skip line.
3. #564: the two Node wasm tasks are gone from `mise.toml`.
4. #709: the three unused workspace dependencies are gone.
5. #710: the Durable Object stub client test runs under `wrangler dev --local` and passes
   1 of 1. The D1/R2/KV suite still passes 7 of 7.

## Next steps

1. Merge the follow-up branch into master.
2. No open issue remains for this specification.

## Open questions

None. Section 12 of `requirements.md` records the five decisions.
