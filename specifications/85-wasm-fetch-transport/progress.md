# Spec 85 Progress: Wasm HTTP Streaming

_Last updated: 2026-11-15._

## Status at a glance

| Task | State |
|---|---|
| T1 — `WasmByteStream`, the streaming response body of the wasm client, `body_queue_chunks` | ⬜ Pending |
| T2 — `open_exchange` passes `self`; the exchange task calls that client; `WASM_FETCH_CAPABILITIES` | ⬜ Pending |
| T3 — `streaming_response_with(status, headers, body)` | ⬜ Pending |
| T4 — the streaming inbound body in both bridges, and the connectrpc `ResponseSink` | ⬜ Pending |
| T5 — `StubHttpClient` over `worker::Stub`, behind the `cloudflare` feature | ⬜ Pending |
| T6 — the Node test runner and the `ci/packages.toml` entry | ⬜ Pending |

## No product code exists

This specification is a plan in a pull request for the owner's review. No file under
`backends/` changes until the owner approves it.

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
#557 (high) records the whole-body read and closes with T1. #558 (medium) records the stale
capability constant and closes with T2. #559 (medium) records that `open_exchange` ignores its
client and closes with T2.

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

## Next steps

1. T1 first. It is the only gap that needs a wasm test harness, and it makes the rest work.
2. T2 next, and correct the capability constant in the same change.
3. T3, which T4 needs.
4. T4 after T3.
5. T5 after T2, behind the `cloudflare` feature.
6. T6 last, because the Node runner needs the tests of T1 to T5.

## Open questions

None. Section 12 of `requirements.md` records the five decisions.
