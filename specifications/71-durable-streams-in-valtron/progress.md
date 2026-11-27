# Spec 71 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-27: closed #1050 (medium). Three valtron test files read the global thread RNG at 26
  sites: `single_executor.rs` (5), `self_wake_fairness.rs` (1) and `local_executor.rs` (20).
  Each file now draws the executor seed from a local `ChaCha8Rng` with a fixed seed constant,
  so a run replays and no host entropy is read. `foundation_compact::rng::rng` is now in
  `backends/foundation_core/clippy.toml` `disallowed-methods`, with the five earlier entries
  kept. Counts: the three modules pass (20 + 1 + 6). Clippy is clean for `multi` and for
  `multi,test-clock,vfs-testkit`. The CI check tests pass (32). The full `valtron` filter shows
  one failure, the known #608, which is not related to this change. One call stays in the
  standalone `integration/wasm_tests` crate; it tests the host-ABI seeding of the global RNG
  at the host boundary.

- 2026-11-27: feature 05 T6 (partial): the SQL VFS engine and the D1 executor.
  `foundation_core::durable::shared::sql` holds `SqlExecutor` (execute, query and
  an atomic transaction that returns each statement's changed-row count),
  `SqlVfs<E: SqlExecutor>` over the `stream_meta` and `pending_data` tables,
  the immutable block format, the `BlockStore` trait, the batcher and the
  vacuum. The block key is `durable/<hex(stream)>/<first>-<last>`. An append
  writes the chunk row and the metadata row in one transaction. A read serves
  SQL rows with `substr` slices; a vacuumed range is fetched from the block
  store with three range GETs (header, index, payload). `durability` is `Hot`
  (default) or `Object`. The limits are `SqlVfsConfig` fields: 256 KiB block
  cap, one second batch age, 64 delivered rows to vacuum, three retries,
  fork depth 64. Two engine columns extend the design and are documented:
  `stream_meta.blocks` is the delivered-block index that a vacuumed read needs,
  and `pending_data.appended_ms` is the append time that the injected clock
  needs to measure the batch age. The D1 executor is
  `foundation_db::D1Executor` in `src/wasm/wasm_storage/d1_executor.rs`; it adds
  the `batch` and `raw` D1 calls and compiles for wasm32. Tests: 11 in
  `foundation_core/tests/durable/sql.rs` against libsql (a real SQLite engine),
  including the shared VFS kit (`sql_vfs_passes_conformance`), batching, batch
  age, crash replay, vacuum plus block read, bounded range read, retry bound,
  object durability, a missing block and every VFS method. Counts: core durable
  126 pass. The DO SQLite half is blocked on issue #1052: the repository has no
  Durable Object SQLite binding, so no executor was stubbed. The clock guard is
  clean because the engine stores `Ctx`. Clippy is clean in all four CI trees.

- 2026-11-27: closed #1043 (medium). The durable files are now free of clippy lints. Before:
  19 library lints in `src/durable` and 16 test lints in `tests/durable`. After: 0 in both.
  The doc comments take backticks for `ConnectRPC` and `ETag`. The `expire` match merges its
  arms into one. `create` moves the fork path to the new `resolve_fork` helper, and `append`
  moves the closed-stream path to the new `closed_append_outcome` helper, so both functions
  fit the 100-line limit. The redundant `continue` becomes an empty arm. The private
  `client::client` module is renamed to `client::core`. `parse_offset` uses `as_chunks::<2>()`
  and one remainder check. The memory VFS read uses `saturating_sub`, and the append uses
  `clone_from`. The client test mock stores plain `SseEvent` values. The time-to-live (TTL)
  test clock carries one `#[expect]` with a reason, because the test reads only wall time.
  Counts: core durable 115 pass, file VFS 17 pass, time source guard 0 violations. The
  `clippy.toml` `rand::thread_rng` entry still warns; it is not a durable file and does not
  fail the gate.

- 2026-11-27: fixed #1042 (low). A direct `read_at` on a soft-deleted entry returns
  `VfsError::Gone` on both VFS implementations. `MemoryVfs::read_at` checks the entry before the
  chain; the internal fork chain still reads a soft-deleted source. The contract sentence is in
  `requirements.md` section 3.7. New tests: `durable::soft_delete_read` (2) and file
  `soft_delete_read` (2). Counts: core durable 115 pass, file VFS 17 pass. The time source guard
  reports 0 violations. The changed files add no clippy lints. Issue #1043 already records the 19
  pre-existing clippy lints in the durable module.

- 2026-11-27: feature 08 CI wiring landed. `.github/workflows/check.yaml` has a
  `durable-conformance` job. It runs `ci/durable_conformance.sh` on a runner with Rust and
  Node 20 when a pull request touches a durable path, and on every full run. The job sets a
  concurrency group, cancels a superseded pull request run, and has a 20-minute timeout.
  The gate parses the reporter output. The server suite must be 338 passed (338) with no
  skip. The client suite must be 267 passed, 0 failed and 2 suite-forced Swift skips.
  `foundation_core` adds `multi,test-clock,vfs-testkit` to `runs` and `clippy-runs`, so the
  memory VFS conformance test runs in CI. The wasm checks from issue #1030
  (`foundation_netio` default and `wasm-fetch`; `foundation_core` `std,multi`) were already
  in `ci/packages.toml`, and the plan accepts them.

- 2026-11-27: feature 05 T7, T9 and T10 landed. The shared kit lives in
  `foundation_core::durable::vfs_testkit` behind the `vfs-testkit` feature. It runs nine scenarios:
  create, append, read byte-exact, bounded pages, fork prefix, fork append, soft-delete and
  cascade, time-to-live (TTL) slide with an injected clock, and version conflict. A
  `StoreFactory` builds a fresh store and clock per scenario; `run_all` returns
  `ScenarioObservation` values that the differential oracle compares.
  `backends_match_model` compares the memory and file VFS. `restart_preserves_stream` drops and
  reopens the file root. `read_is_bounded` reads with page limits of 1, 3, 7 and 20 bytes.
  Coverage tests call every `DurableStreamVfs` method on both VFS implementations: `create`,
  `append`, `append_with_meta`, `read_at`, `meta`, `touch`, `trim`, `close`, `remove`, `list` and
  `notifier`. Counts: core durable 113 pass, file VFS 15 pass. Clippy is clean for the new and
  changed code. Closed bug #1041 (high): the file VFS allocated `vec![0; usize::MAX]` when a read
  had no page limit. Opened #1042 (low): the memory and file VFS differ on `read_at` for a
  soft-deleted entry.
  Uncovered branches: both `read_chain` fork-depth limits (a chain deeper than 64 or the
  configured file limit); the file VFS input/output (I/O) error branches (permission, read,
  rename, directory removal); `StoredMeta::parse` malformed-record branches; the file `cascade`
  skip branches; the file create-rollback branches; the `usize` conversion fallbacks (unreachable
  on a 64-bit host); the poisoned-mutex fallbacks; and `VfsError::Unsupported`, which neither VFS
  returns.

- 2026-11-27: feature 07 Server-Sent Events (SSE) landed. The client core returns an `SseOpen`
  head (status, headers, stream) from `ClientTransport::open_sse`, so a 404 or 410 answers from
  the head and `stream-sse-data-encoding` decides base64. The SSE read is a `futures_core::Stream`
  (`SseRead`), not a collection loop. The native event task stops blocking: its `ReadingStream`
  state parks on the connection readiness source with `TaskStatus::Depends`. The client body
  reader de-chunks a chunked SSE response. The adapter announces every feature and escapes U+2028
  and U+2029 in its output. The client suite is 267 pass, 2 suite-forced skips, 0 fail; the server
  suite stays 338 pass, 0 skip. Closed #963, #947 and #1033.

- 2026-11-27: issue #1034 fixed. `DurableStream::new` and `DurableStream::with_ctx` take
  `&Ctx`. The store reads the clock with `ctx.clock()` and no longer builds a
  `MonotonicClock`. `SubscriptionStore` and every call site pass the context. The clock
  parameter guard reports no durable or event-stream line. Core durable tests pass 106,
  event-stream tests pass 12, HTTP durable adapter tests pass 30, native file VFS tests
  pass 8, and clippy is clean for the touched crates.

- 2026-11-27: issue #1030 fixed. `foundation_core` compiles for wasm32 with `multi`. The
  native `idle_wake` call in the `multi` executor now has a target gate, and netio's
  `ExchangeContinuation` follows the `multi` feature for its `Send` bound. CI runs the
  default-feature `foundation_netio` wasm check and a `foundation_core` `std,multi` wasm
  check. The default-feature wasm check, the native `multi` tests and both clippy runs pass.

- 2026-11-27: issue #1019 fixed, feature 07 T11. Both wasm bridges
  (`foundation_http/src/wasm/bridge/{cf,web}.rs`) now call `with_preserved_headers()` on the
  request builder, so a parsed fetch request keeps its wire `Content-Length` and gains no
  `Transfer-Encoding` for a streamed body. The default builder behaviour is unchanged for a
  fresh request. Two wasm-bindgen cases prove it on the Deno host:
  `web_bridge_preserves_content_length_for_a_streamed_body` and
  `cf_bridge_preserves_content_length_for_a_streamed_body`. Both failed on the old code and
  pass now. The client core wasm32 build was already recorded.

- 2026-11-27: feature 10 (subscriptions) landed. The new `foundation_event_stream` crate owns
  the reserved `__ds` control endpoints, the configuration hash, the generation fence, the lease
  and the wake snapshot. It delivers signed webhooks with retry and the `failed` status, runs the
  pull-wake claim, ack and release flow, and serves the JSON Web Key Set (JWKS) endpoint. It
  re-exports `foundation_core::durable` and reuses the `foundation_auth` key model.
  `ci/durable_conformance.sh` sets `subscriptions: true` now: the server suite is 338 pass,
  0 skip, 0 fail, and the client suite is 208 pass, 0 fail, 61 skip. The crate suite is 12 pass.
  The example server mounts the handler and runs a tick for lease expiry and retries.

- 2026-11-27: issue #926 root cause fixed. The request builder dropped `Content-Length` for a
  streamed parsed body, so the adapter's 8 MiB cap left a 2 MiB tail on the connection and the
  next keep-alive GET parsed as an unknown method. The builder keeps parsed headers now, and the
  adapter answers 413 and closes for a body over the cap. A regression test and a `DURABLE_LOG`
  subscriber land with it. `Router::dispatch_result` also propagates `MethodNotAllowed` again,
  which the durable adapter commit had swallowed.- 2026-11-27: feature 05 T4 landed. `FileVfs` in `foundation_nativeapis` has 8 tests:
  create, append, exact read, restart, stale-version conflict, fork prefix, invisible
  source append, soft-delete cascade, missing fork source, closed append and notifier.
  The crate suite with `vfs-native` is 238 pass, 0 fail, 5 ignored.
- 2026-11-27: the valtron wrappers moved to `foundation_core::durable::valtron`. The
  compatibility path is removed; every caller uses the new path. The durable suite is
  106 pass, 0 fail.
- 2026-11-27: `block_on_future_scoped` has 4 tests: a borrowed non-`Send` future, a
  cross-thread wake, a self-wake and a panic. They pass with and without `multi`.
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
- 2026-11-27: queued issue #1019 after feature 10. The wasm bridge must preserve the parsed
  request headers when it builds a streamed fetch body. Close the issue with the fix and a wasm
  test.
- 2026-11-27: process update. The handoff lives in `handoff.md` in this spec. A fresh agent
  continues from it. The file VFS first implementation is in
  `foundation_nativeapis/src/native/vfs/durable_file.rs` and compiles under `vfs-native`; it has
  no tests, so it is uncommitted. `block_on_future_scoped` replaces `smol` in the durable tests
  (106 pass).
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
  `foundation_core::durable::valtron` as the required wrapper. Added feature 07 (Rust client) and
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
- **05-storage-backends** — 🟡 in progress (5/6). T1 to T5 and T7 to T10 are
  done. T6 is partial: the SQL engine, the tiering and the D1 executor landed;
  the DO SQLite executor waits on issue #1052 (no DO SQLite binding in the
  repository).
- **06-http-rpc-surface** — 🟡 in progress (10/11). `DurableService`/`DurableLimits`, JSON mode,
  the ETag/cache/cursor helpers, the error-status/header table, the header/offset parsers, the
  request adapter, `304`, long-poll, SSE, forks, `Expires-At`, CORS, `nosniff` and the full
  server conformance run landed. ConnectRPC and `429` remain.
- **07-durable-streams-client** — 🟢 done (11/11). The client core, transport injection,
  catch-up, long-poll, auto and SSE reads, the native transport, the adapter, the client
  conformance run, batching, dynamic headers, validation features and the SSE head/stream all
  landed. The client suite is 267 pass, 2 suite-forced skips, 0 fail.
- **08-conformance** — 🟡 in progress. The CI gate landed: a Linux job runs both suites on a
  durable change and on full runs, and the memory VFS conformance test runs in CI. The
  valtron suite, the cross-runs, the skip register, the report and the runtime matrix remain.
- **09-s3-r2-vfs** — ⏳ not started (0/16)
- **10-subscriptions** — 🟢 done (7/7). The `foundation_event_stream` crate holds the
  transport-neutral engine, the native handlers and the webhook sender. The six server
  conformance tests pass and the gate shows 0 skips.

## Next steps

1. Feature 01 (core types and store trait) first.
2. Then 02 and 03 in parallel. Add 04, 05 and 06.
3. Run the conformance spike after 01 and the first HTTP adapter slice.
4. Add feature 07 (client). Then feature 08 (conformance gate).
5. Add the valtron wrapper tests after the core passes conformance.
6. Wire `foundation_log` (issue #155) and spec 83 `01-agent-rpc` / `05-cloudflare` to consume the
   core once 01–03 land.

_Created: 2026-10-30_
