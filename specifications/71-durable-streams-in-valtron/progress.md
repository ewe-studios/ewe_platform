# Spec 71 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

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
- **06-http-rpc-surface** — 🟡 in progress (7/11). `DurableService`/`DurableLimits`, JSON mode,
  the ETag/cache/cursor helpers, the error-status/header table, the header/offset parsers, the
  request adapter, `304`, long-poll, SSE, `nosniff` and CORS resource policy landed. ConnectRPC,
  `429`, CORS exposure, the oracles and the conformance run remain.
- **07-durable-streams-client** — ⏳ not started (0/10)
- **08-conformance** — ⏳ not started (0/7)
- **09-s3-r2-vfs** — ⏳ not started (0/16)

## Next steps

1. Feature 01 (core types and store trait) first.
2. Then 02 and 03 in parallel. Add 04, 05 and 06.
3. Run the conformance spike after 01 and the first HTTP adapter slice.
4. Add feature 07 (client). Then feature 08 (conformance gate).
5. Add the valtron wrapper tests after the core passes conformance.
6. Wire `foundation_log` (issue #155) and spec 83 `01-agent-rpc` / `05-cloudflare` to consume the
   core once 01–03 land.

_Created: 2026-10-30_
