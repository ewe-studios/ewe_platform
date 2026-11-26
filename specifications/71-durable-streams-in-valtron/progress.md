# Spec 71 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-27: feature 07 auto mode and streaming are enabled. The adapter catches up, then
  waits once with a long-poll read. The client gate rises to 208 pass, 0 fail, 61 skip.
- 2026-11-27: feature 07 dynamic headers and params are enabled. The adapter evaluates a
  counter, timestamp or token value on each request and reports `headersSent` and `paramsSent`.
  The client gate rises to 202 pass, 0 fail, 67 skip.
- 2026-11-27: feature 08 has `ci/durable_conformance.sh`. It builds both examples, runs the
  server suite and the client suite, and fails on any failure. A clean run reports 332 server
  passes and 208 client passes.
- 2026-11-27: feature 08 records the verified conformance commands and their results in its
  feature file. The server run is 332 pass, 6 skip, 0 fail. The client run is 198 pass, 0 fail,
  71 skip. The harness task remains.
- 2026-11-27: the owner corrected two scope points. The client core and transport trait live in
  core. The reserved subscription APIs are in scope; feature 10 records the work. The six server
  skips are a gap, not an owner decision.
- 2026-11-27: the client layering is corrected by the owner. The client core and the
  `ClientTransport` trait live in `foundation_core::durable::client`. `foundation_netio` holds only
  the native transport. The 24 mock tests move with the code. The valtron client reader lives in
  `client/valtron.rs` and has a page-order test. The client gate stays at 208 pass, 0 fail.
- 2026-11-27: verified Rust suites. Core durable 81 pass. HTTP durable targets 40 pass (adapter
  30, parser 6, fallback 2, status 2) plus the server end-to-end target. Client core 24 pass.
- 2026-11-27: checkpoint. `ci/durable_conformance.sh` exits 0: server 332 pass, 6 skip, 0 fail;
  client 208 pass, 0 fail, 61 skip. The 61 skips are Server-Sent Events (SSE) only. Closed
  issues: #926 (405 flake, regression test), #974 (JSON page chunk limit), #975 (JSON VFS read
  window). Open: #963 (blocking SSE poll) and #947 (head with the event stream). The client core
  holds 21 mock tests, the HTTP adapter 28, the core 78.
- 2026-11-27: three small verified slices. A rate-limit error answers 429 with `Retry-After`. The
  status-table test names every `StoreError` variant. `DurableLimits` has a builder for each
  field. An architecture test proves the durable core names no `::valtron`, `foundation_http`,
  `hyper` or `tokio` code path.
- 2026-11-27: feature 07 batching is enabled. The adapter announces `batching: true`. The client
  gate rises to 198 pass, 0 fail, 71 skip. The batch path sends one array append for a JSON
  stream, concatenates bytes for a text stream, tracks the producer sequence and drops a
  repeated batch.
- 2026-11-27: feature 07 Server-Sent Events (SSE) core. `ClientTransport::open_sse` streams
  parsed events, the native transport maps the netio event stream, and the read builder turns an
  SSE response into chunks and a control result. Two mock-transport tests pass. The adapter still
  announces SSE, auto mode and streaming as unsupported, because the end-to-end path does not
  yet pass the whole suite. The client gate stays at 194 pass, 0 fail, 75 skip.
- 2026-11-27: feature 07 client conformance passes with the announced features. The reference
  client suite reports 194 pass, 0 fail, 75 skip. The skips are Server-Sent Events (SSE),
  automatic mode, batching and dynamic headers, which the adapter announces as unsupported.
  The adapter is `foundation_netio/examples/durable_client_adapter.rs`. The native transport is
  `foundation_netio::native::durable::NetioTransport`. The run found and fixed: a missing
  valtron pool, duplicate `content-type` keys, a missing default offset, JSON reads, producer
  sequence tracking, client-side batch deduplication and epoch auto-claim.
- 2026-11-27: feature 07 client core started. `foundation_netio::shared::durable` has the
  injected `ClientTransport`, the six verbs and a catch-up/long-poll read. The offset wire codec
  moved to `foundation_core::durable::wire`, so the server and the client share one codec.
  10 client tests pass and the wasm32 check passes. The native HTTP transport, SSE, retry,
  producer batching, dynamic headers, the adapter and the client conformance run remain.
- 2026-11-27: feature 06 server conformance passes. The reference
  `@durable-streams/server-conformance-tests` suite reports 332 pass, 6 skip, 0 fail against
  `DurableHandler` over the memory VFS. The run found and fixed: composite offset input, the
  absent `Stream-Expires-At` parser, producer-before-closed ordering, close as a producer
  write, fork content-type inheritance and mismatch, `409` for a soft-deleted path, the source
  reference of a soft-deleted fork, SSE `data:` framing, and the `Location` host. The example
  server sets a 3 s long-poll timeout, because the suite waits inside a 5 s test limit.
- 2026-11-27: feature 06 HTTP surface landed its live reads. A GET answers catch-up, long-poll
  and Server-Sent Events (SSE). Long-poll and SSE wait on the stream notifier, and the injected
  clock gives the timeout. SSE sends `data` and `control` events and base64-encodes a binary
  stream. The adapter renders an internal offset as lowercase hexadecimal, so the wire text keeps
  the byte order. 22 adapter and parser tests pass. ConnectRPC, `429`, CORS exposure and the
  server conformance run remain.
- 2026-11-27: feature 06 HTTP request adapter landed. The native `Serve` handler maps create,
  append, close, read, head and delete to `DurableService`. A catch-all route sends every path to
  the adapter. The adapter is tested from `tests/durable_adapter.rs` without a socket. 15 tests
  pass.
- 2026-11-27: feature 06 JSON mode landed in the core. Appends validate JSON, flatten one array
  level and reject an empty append array. Creates accept an empty array. Reads return one chunk
  per message. 45 tests pass.
- 2026-11-27: feature 04 TTL sliding landed. The core takes an injected clock, stores
  `last_access_ms` in VFS metadata, and expires on read, append, fork and head. A read slides the
  window; `HEAD` does not. 39 tests pass.
- 2026-11-27: feature 04 fork and soft-delete landed. Fork reads span the boundary, source
  appends stay invisible, soft-delete keeps forks readable, cascade cleans up, and fork
  creation is idempotent. 38 tests pass.
- 2026-11-27: feature 03 core landed. Producer state and `Stream-Seq` persist in VFS metadata.
  Epoch fencing, sequence gaps, dedup, `Stream-Seq` ordering and versioned atomic append are
  tested, including two concurrent producers. 33 tests pass.
- 2026-11-27: feature 02 live tail landed. The memory VFS notifier fires on append and close.
  The valtron reader waits at an open tail and yields data after an append. 24 tests pass.
- 2026-11-27: feature 02 started. Core page reader state rules and the valtron catch-up reader
  land as a `#[task]`. 22 durable tests pass; clippy and wasm32 are clean. Live tail, wake,
  trim and the oracle matrix remain.
- 2026-11-27: S3/R2 layout is configuration. Segmented objects are the default. An append-only
  file layout is supported. Batching is local: one batch becomes one object PUT. Segment size,
  batch size, linger, parallel PUTs and flush policy are configuration.
- 2026-11-27: durable core moved to `foundation_core::durable::shared`. Added the
  `shared/`, `wasm/`, `cf/` and `native/` layout rule and the per-feature path tables. 21
  durable tests pass.
- 2026-11-27: feature 09 designed from the Rivet study. The S3/R2 VFS uses immutable segment
  keys, a local spool, one PUT per batch, conditional-write CAS, publish-after-PUT, read
  revalidation, a lifecycle with a grace window, and explicit R2 settings. No feature started
  yet (0/16).
- 2026-11-27: storage design settled. `DurableStream` is a struct over a boxed
  `DurableStreamVfs`. The memory VFS lives in `foundation_core`. The file VFS lives in
  `foundation_nativeapis`. `Ctx` holds `Arc<DurableStream>`; swapping storage changes one
  construction line.
- 2026-11-27: feature 01 started. Added `foundation_core::durable` with the core types, the
  store trait, `DurableDriver`, the `DurableStreams` business API, the boxed VFS, the memory
  VFS, a Given/When/Then harness and a `proptest` property test. 17 tests pass. The wasm32
  check passes. T4 (valtron-disabled build) remains.
- 2026-11-27: spec restructured to version 2.0. The protocol core moves to
  `foundation_core::durable` and has no valtron, HTTP or socket dependency. The HTTP and
  ConnectRPC adapters call `DurableService`. The valtron reader, writer and producer task move to
  `foundation_core::valtron::durable` as the required wrapper. Added feature 07 (Rust client) and
  feature 08 (conformance). Server and client conformance are a hard gate. The runtime matrix
  covers local embedded, DO, Worker, WebSocket and the `foundation_ai` Session. The owner
  reviewed and approved the 716-line scope for `requirements.md` on 2026-11-27. No feature
  started yet (0/8).
- 2026-11-27: spec amended after the protocol review. Added `Stream-Seq` to the contract, the
  core types, the producer, the HTTP surface and the tests. Recorded the async store trait and
  `DurableStream` enum injection decision. Added the structured `StoreError` and the HTTP status
  mapping. Recorded conformance as a hard gate for both server and client.
- 2026-10-30: spec written. The Durable Streams research note
  (`specifications/research/edge-data/durable-streams.md`) is the input; the features turn it into
  `foundation_core::durable` and the required valtron layer. No feature started yet.

## Feature status

- **01-durable-stream-types** — 🟡 in progress (6/7). Core types, store trait, `DurableDriver`,
  BDD harness, memory VFS, business API, property test and model comparison landed. 17 tests
  pass. The wasm32 check passes. T4 (valtron-disabled build) remains.
- **02-resumable-reader** — 🟡 in progress (6/10). Core page reader, state rules, closure
  discovery, the valtron catch-up reader, the live tail wait/notifier, the `into_next_stream`
  bridge and trim refusal landed. Wake forwarding under `multi`, DST and the oracle matrix
  remain.
- **03-idempotent-producer** — 🟡 in progress (6/8). Core writer, producer ladder, dedup/fencing,
  `Stream-Seq`, versioned atomic append, the valtron producer task and restart dedup landed.
  Core-independence and the oracle matrix remain.
- **04-fork-and-retention** — 🟡 in progress (6/9). Fork read stitching, creation edge cases,
  idempotent creation, soft-delete, cascading cleanup, sliding TTL, TTL inheritance and
  sub-offset resolution landed. The full TTL table, retention, valtron boundary and oracles
  remain.
- **05-storage-backends** — ⏳ not started (0/6)
- **06-http-rpc-surface** — 🟡 in progress (10/11). `DurableService`/`DurableLimits`, JSON mode,
  the ETag/cache/cursor helpers, the error-status/header table, the header/offset parsers, the
  request adapter, `304`, long-poll, SSE, forks, `Expires-At`, CORS, `nosniff` and the full
  server conformance run landed. ConnectRPC and `429` remain.
- **07-durable-streams-client** — 🟡 in progress (8/11). The client core, transport injection,
  catch-up, long-poll and auto reads, the native transport, the adapter, the client conformance
  run, batching, dynamic headers and validation landed. The adapter-level SSE path and the
  valtron wrapper remain.
- **08-conformance** — ⏳ not started (0/7)
- **09-s3-r2-vfs** — ⏳ not started (0/16)
- **10-subscriptions** — ⏳ not started (0/7). The owner corrected the scope on 2026-11-27: the
  reserved subscription APIs are in scope. The six skipped server tests wait for this feature.

## Next steps

1. Feature 01 (core types and store trait) first.
2. Then 02 and 03 in parallel. Add 04, 05 and 06.
3. Run the conformance spike after 01 and the first HTTP adapter slice.
4. Add feature 07 (client). Then feature 08 (conformance gate).
5. Add the valtron wrapper tests after the core passes conformance.
6. Wire `foundation_log` (issue #155) and spec 83 `01-agent-rpc` / `05-cloudflare` to consume the
   core once 01–03 land.

_Created: 2026-10-30_
