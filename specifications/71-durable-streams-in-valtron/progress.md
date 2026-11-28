# Spec 71 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-11-28: spec 71 follow-ups on branch `spec71-followups` after the merge into master.
  #1218: kqueue no longer suppresses a socket read edge; #1152: producer `max_in_flight` and
  linger pipelining on the injected clock; #1219: parked readiness payloads are `Send + Sync`
  with no allow; #1220: gzipped event streams and line-delimited bodies are stream-decoded;
  #1222: `send` and `send_async` keep request extensions and `body_readiness`; #1167: durable
  HTTP golden oracle with 11 fixtures and a documented `UPDATE_GOLDEN` path. Feature 06 is
  done. The workspace dev check fails only on third-party `wasmtime-internal-fiber` on aarch64
  (#1241), so the criterion is amended to the durable crates with that evidence. Feature 08 is
  done. Feature 07 T6 is done; T12 (client oracles) remains. Open follow-ups: #1230 (a
  cancelled driving append stalls the queue), #1233 (final line terminator), #1237 (macOS
  `vfs-preload` clippy), #1229 (pipe race), #1241 (wasmtime aarch64).
- 2026-11-28: feature 01, 05, 06, 07 and 08 gaps closed. #1134: `DurableStream` keeps a forked
  `Ctx`; the fork shares the clock and copies the map, so the parent context holds the stream
  without an `Arc` cycle. The leak test and the nine TTL tests pass; the TTL tests advance the
  simulated clock after construction, which proves the stream reads the context clock.
  #1135: retry, producer, dynamic headers and the injected clock live in the client core; the
  adapter uses them. #858: the connectrpc `build.rs` emits the Cranelift cfg, so the panic test
  runs under LLVM and is ignored only under Cranelift. #1155/#1024: netio clippy is clean.
  #1153: the feature 07 record names the real test. Feature 08 T6: a `durable-valtron` gate with
  one job per group (reader, writer, wait, wake). T17: fault-injection and malformed-input fuzz
  oracles. Full core suite: 1,046 passed with default features and 1,134 with
  `test-clock,multi`; both 0 failed, 0 ignored. Feature 06 T11 stays open: no golden oracle
  (#1167). Open follow-ups: #1152, #1160, #1162.
- 2026-11-28: feature records checked against the code and the tests. Status: 01 in progress,
  02 done, 03 done, 04 done, 05 in progress, 06 in progress, 07 in progress, 08 in progress,
  09 done, 10 done, 11 done.

  Open items:
  - 01: the full `cargo test -p foundation_core` run is not recorded. The BDD harness runs on
    one mock driver only.
  - 05: the full `cargo test -p foundation_core` run is not recorded. The
    `foundation_nativeapis` and `foundation_db` runs are recorded.
  - 06: T11 has no `injected_error_is_typed` or `security_headers_present` test. The full
    `cargo test -p foundation_connectrpc` run has the pre-existing macOS failure #858. The
    workspace-wide `cargo check` is not recorded.
  - 07: the client core has no retry, producer, dynamic-header or clock API. Those behaviours
    live only in the conformance adapter. T5, T6 and T12 stay open. Issue #1135.
  - 08: no separate valtron job (T6). No protocol-level failure-injection or fuzz oracle (T17).
    The "every rule has a derived oracle" and "valtron tests pass separately" criteria stay
    open.
  - Issue #1134 records the `Ctx` and `DurableStream` `Arc` cycle.

  Ticked with evidence: 01 criteria 5 and 6; 05 criteria 2 to 8; 06 T8 and criterion 3; 07 T4;
  08 T8, T12 to T16 and T18 to T21, plus the runtime, property and VFS conformance criteria.
  Features 02, 03, 04 and 10 are done.

- 2026-11-28: feature 08 T12 Session proof landed as a document scenario suite. The stream
  `Driver` trait in `integration/durable_transport_matrix` is not used for the Session.
  `DurableDocumentStore` is document oriented: one session id is one stream and one document
  is one JSON chunk. A `Driver` mapping would encode the bytes again, move the chunk
  boundaries and hide the create status, the time to live and the `Stream-Seq` outcome, so it
  would be a fake. The proof is `backends/foundation_ai/tests/agentic/durable_store_scenario_tests.rs`.
  The thin top-level target `backends/foundation_ai/tests/durable_store_scenario_tests.rs` runs
  it with the default features, because the `agentic_tests` target needs the `testing` feature.
  It runs five Given/When/Then scenarios over `MemoryVfs` with `Ctx::host()` and an independent
  document model. Each test asserts the model first, then the store. The scenarios are: a
  write and read round trip; a replay from an offset returns the later documents in order; a
  branch forks at the parent tail, reads the prefix and its own document, and leaves the
  parent unchanged; a replay from the tail offset returns nothing and a later write is read
  from it; and a missing session reports no document. Issue #1092 is closed.

- 2026-11-27: strict gate, hosted stores and wasm proof. `bash ci/durable_conformance.sh` with
  live MinIO: server 338 passed (338); client 267 passed, 0 failed, 2 upstream-forced skips; DO
  matrix eight scenarios; local R2 block store; our own base64 SSE coverage; exit 0. The gate
  does not patch the upstream suite; our client tests cover the two skipped behaviours.
  MinIO has a local compose service in `docker-compose.services.yaml` (profile `object-store`).
  #1056 fixed: `turso` and `libsql` no longer pull `multi`, so the default `foundation_db` build
  works for wasm32. #1080 fixed: `durable_core_roundtrip_over_memory_vfs` runs over `MemoryVfs`
  on the `js-foundation-wasm` host. #1115 fixed: the local executor now waits on the nearest
  sleeper deadline, so the wasm deadline-order case passes.
- 2026-11-27: feature 08 T12 (Durable Object CI fixes). The gate uses the project wrangler pin,
  `npx --yes wrangler@4.156.2` (`WRANGLER_VERSION` in `mise.toml`), not a global npm install.
  `tests/do_driver.rs` resolves `WRANGLER_BIN`, then `npx --yes wrangler@<version>`, then
  `wrangler` on `PATH`. `worker-build` 0.8.7 installs with `cargo binstall` when present and
  `cargo install` otherwise. The readiness wait is 240 s and prints the tail of the wrangler log
  on failure. Measured `bash ci/durable_conformance.sh` on this host: 231.91 s real (3 min 52 s),
  exit 0, server 338 passed, client 267 passed, and the DO driver 1 test in 18.00 s. The
  20-minute job limit holds.
- 2026-11-27: feature 08 T12 (Durable Object driver, #1091 closed). The transport matrix has
  five drivers: embedded, HTTP, WebSocket, ConnectRPC and a Durable Object worker. The shared
  code (`driver`, `model`, `scenario`) builds for wasm32; the native drivers sit behind the
  `native-drivers` feature. The DO route is `GET /matrix?scenario=...` in
  `backends/foundation_db/integration/do_sql`. It runs the same eight scenarios over
  `SqlVfs<DoSqlExecutor>` with an in-memory block store, and its wait is a JavaScript
  `setTimeout`, so the TTL scenario runs. The native test
  `tests/do_driver.rs` starts `wrangler dev --local` on a free port and compares the DO reply
  with the model oracle and the embedded driver. The run is in `ci/durable_conformance.sh`,
  which installs wrangler 4.144.0 and worker-build when they are missing and adds the wasm32
  target. Verified: the eight scenarios match on the DO (1 test, 73 s); the normal suite runs
  9 tests; clippy is clean with and without `do-driver`; the wasm32 check of the shared code
  passes. #1092 stays open for the Session document-store reason.
- 2026-11-27: feature 08 T11 and T12 (transport matrix).
  `integration/durable_transport_matrix` holds one Given/When/Then list of eight scenarios:
  create, append and read; read from the first next offset; close and a refused append; fork at
  the tail; a missing read; a producer retry; a `Stream-Seq` repeat and regression; and a TTL
  expiry. One test function per scenario runs the same steps on four drivers. Each driver is
  checked against the model oracle first, then the drivers are checked against each other.
  Drivers: embedded (`DurableService` over `DurableStream` and `MemoryVfs`), HTTP, WebSocket and
  ConnectRPC. The model takes its `Ctx` in the constructor and reads the wall clock and the timer
  from that bundle. The TTL scenario uses a 1 s TTL and a bounded 1.5 s wait. T11 is done. T12 is
  open for two runtime drivers: the DO needs wasm32 and wrangler (#1091), and
  `DurableDocumentStore` is a document API, so a Session driver would be a fake (#1092). The
  package is in `ci/packages.toml`. `cargo test --profile uat -p durable_transport_matrix` runs
  9 tests: 8 scenarios and the list test. Clippy is clean and `foundation-ci plan` lists the
  crate.
- 2026-11-27: issue #1028 closed (medium). Webhook delivery leaves the append path. A create, a
  confirm, an append, an acknowledgement and a lease change only record the wake and put the
  subscription identifier in a bounded first-in-first-out (FIFO) queue. `SubscriptionEngine::tick`
  expires due leases, queues due retries and drains the queue; a host can also call
  `drain_deliveries` from its own executor after `DeliveryNotifier::delivery_queued`. The worker
  bounds one attempt with `delivery_timeout_ms` on the injected timer, so a stalled receiver
  cannot hold the worker. It extends the wake lease over the attempt and over each scheduled
  retry, so the lease cannot expire during delivery. The retry count stops at
  `delivery_retry_bound`; the subscription stays `failed`, and a new append starts a new wake
  with a new budget. The queue holds each subscription once and keeps FIFO order.
  `SubscriptionConfig` adds `delivery_queue_bound` (default 1 024), `delivery_retry_bound`
  (default 8) and `delivery_timeout_ms` (default 10 000), each with a builder method. The
  example server installs the real effect environment, so its tick enforces the timeout.
  `tests/subscriptions/t7_delivery_queue.rs` adds 4 tests: a clock-gated receiver does not
  delay create, confirm or append and the worker delivers after the clock moves; a failing
  receiver stops at the retry bound; two deliveries for one subscription arrive in order; and a
  stalled send times out on the injected clock. The crate suite passes 16 of 16. A bounded
  upstream run passed the 6 server subscription tests (6 passed, 332 skipped). The
  `foundation_event_stream` files add no time-source or clock-parameter hit.

  The crate gate is green. `cargo clippy --profile uat -p foundation_event_stream
  --all-targets --no-deps -- -D warnings` exits 0. This change also fixed the 24 warnings that
  the gate already reported: `let...else` to `?`, associated helpers for `log_persist` and
  `lease_ms`, checked integer casts in `retry_delay`, `write!` in place of `format!` on a
  `String`, a `SnapshotLine` enum in place of `Option<Option<T>>`, a smaller `handle` split into
  `handle_callback`, `handle_claim`, `handle_ack` and `handle_release`, and one
  `#[expect(clippy::redundant_closure_for_method_calls)]` with a reason, because the mutex poison
  type differs on wasm and the closure keeps that build. `cargo test --profile uat -p
  foundation_event_stream --no-fail-fast` passes 16 of 16, including `crate_builds_wasm`.

  Two medium issues record the remaining gaps: #1089, a full queue drops a wake with no count
  and no dedicated retry, and #1090, `send_bounded` runs unbounded when `Ctx` holds no `Env`
  timer. Both wait for later pickup.

- 2026-11-27: issue #1081 closed. The `durable_core_no_valtron` integration crate now runs the
  protocol conformance suite with `foundation_core` `default-features = false` and the features
  `std` and `alloc`: no `valtron`, `test-clock`, `multi` or `vfs-testkit`, no HTTP and no
  socket. `tests/durable_core.rs` has 11 scenarios: create and idempotent re-create, append and
  byte-exact read, bounded pages, close and a refused append, fork prefix and divergence,
  producer dedup, fencing and a sequence gap, `Stream-Seq` ordering, TTL expiry with a
  hand-written `Clock`, and soft-delete, cascade and a fork that reads a soft-deleted source.
  The HTTP gate server keeps the valtron executor for its socket surface. `foundation_http`
  submits connections with `foundation_core::valtron::send`, its durable adapter waits with
  `foundation_core::valtron::timer::sleep_for`, and `foundation_netio` native readiness returns
  `foundation_core::valtron::EventReadinessPtr`. Its dependency leaves the `foundation_core`
  default features on, and the default set includes `valtron`. A no-valtron HTTP server is
  therefore not feasible with the current crates. Requirement 8 and report section 5.7 state the
  contract. `cargo test -p durable_core_no_valtron` passes 11 of 11 and clippy is clean.

- 2026-11-27: feature 08 T8 WebSocket row landed. The transport is in
  `backends/foundation_http/src/native/durable_ws/`, above the core. `DurableWsHandler` upgrades
  a GET request and serves create, append, close, head, delete, fork and read over JSON text
  frames through `DurableService`. A live read pushes one frame per page and ends on `cancel` or
  when the stream closes. The frame size cap is `DurableWsConfig::max_frame_bytes`, default
  8 MiB. `tests/durable_websocket.rs` has 5 tests: `websocket_core_roundtrip`,
  `live_read_receives_an_append_after_open`, `live_read_ends_when_the_stream_closes`,
  `frame_cap_is_configuration` and `schema_round_trips`. The core keeps no WebSocket type. The
  clock-parameter and time-source guards report 0 violations. Issue #1077 is closed. The BDD
  driver matrix entry (T12) stays open, because `same_scenario_every_driver` has no test yet.

- 2026-11-27: feature 08 T9 `foundation_ai` Session log landed. `DurableDocumentStore` in
  `backends/foundation_ai/src/agentic/durable_store.rs` maps one session id to one durable
  stream and stores one `SessionRecord` per JSON chunk. It implements `DocumentStore`, so
  `AgentSession::builder(..).with_doc_store(store)` runs the session log on the core. A branch
  uses a stream fork at the parent tail. The store takes `DurableStreams<DurableStream>` and a
  `Ctx` from the host; it has no global and makes no HTTP call. `tests/durable_session_store_tests.rs`
  has 6 tests: append and replay round trip, order preserved, replay from a document id, fork
  prefix and divergence, reopen over the same VFS, and `session_uses_core`. The wasm32 check
  passes. Issue #1079 is closed.

- 2026-11-27: feature 08 runtime matrix, skip register and report landed. The report is
  `specifications/71-durable-streams-in-valtron/features/08-conformance/report.md`. It holds the
  gate command, the four-direction matrix, the runtime matrix, the skip register, the open issues
  and the environment caveat.

  Proofs added or confirmed:
  - Embedded: the new `backends/foundation_core/tests/durable/runtime_matrix.rs` has
    `embedded_core_roundtrip` and `service_runs_without_http` (2 pass). The separate
    `durable_core_no_valtron` crate runs its 1 test without the `valtron` feature.
  - wasm32: eight `cargo check --target wasm32-unknown-unknown` commands pass for
    `foundation_core` (`std`, `std,multi`, `std,js-foundation-wasm`), `foundation_event_stream`,
    `foundation_http` (`wasm`), `foundation_db` (`do-sql`, `wasm-bindgen-storage`) and
    `foundation_netio` (`wasm-fetch`).
  - Worker and DO: the recorded `wrangler dev --local` result in
    `backends/foundation_db/integration/do_sql/` returns all nine VFS kit scenarios on the
    top-level Worker route `/run-d1` (D1) and in the DO route `/run` (DO SQLite). The `/rollback`
    route passes. This change did not re-run the fixture.
  - Guard checks: `check-clock-params` and `check-time-sources` report 0 violations.

  Gaps: no WebSocket driver (#1077, medium), no `foundation_ai` Session log driver (#1079,
  medium) and no wasm32 runtime roundtrip test (#1080, low). The gate builds the server with the
  valtron feature on (#1081, medium). The two suite-forced client skips have issue #1082 (low).

  Tasks ticked: T1, T2, T5, T7 and T10. T6, T8, T9 and T11 to T21 stay open. The strict gate
  numbers stay as recorded: server 338 passed (338), 0 skip; client 267 passed, 0 failed, 2
  suite-forced skips. This change did not re-run the gate because the host load was 169. The
  report marks the counts as recorded evidence.

- 2026-11-27: feature 01 T4 landed. `foundation_core` now has a `valtron` Cargo feature. The
  feature is on by default and implies `std`. The gates on `valtron`, `env` and `synca::mpp`
  changed from `std` to `valtron`, and `durable::shared::valtron` has the same gate. `multi`,
  `test-clock`, `vfs-testkit`, `js-wasmbindgen` and `js-foundation-wasm` now imply `valtron`,
  so each existing tree compiles the same code. The new workspace member
  `integration/durable_core_no_valtron` depends on `foundation_core` with
  `default-features = false, features = ["std", "alloc"]`. Its test creates, appends and reads
  through the business API and the memory virtual file system (VFS). A successful build of the
  crate proves that the durable core has no valtron type, trait or executor.

  Verified: `cargo build -p foundation_core --no-default-features --features std,alloc` passes;
  the new crate test passes (1); `cargo check --target wasm32-unknown-unknown` passes with and
  without valtron; the durable run is 146 tests; clippy for `multi,test-clock,vfs-testkit`
  `--all-targets` is clean; `foundation-ci plan` selects `durable_core_no_valtron` with no macOS
  or Windows opt-out; both guards report 0 violations. The no-valtron tree found two old doc
  warnings in `trace/mod.rs`; the fix uses an inner module doc comment.

  Still open for feature 01: the full `cargo test -p foundation_core` criterion. This change
  runs the durable test filter only.

- 2026-11-27: feature 08 T4 (CI) changed to run on every CI run. Owner rule: a protocol
  mismatch must fail on any change. The `durable-conformance` job in
  `.github/workflows/check.yaml` no longer has a changed-path filter step or step conditions.
  It keeps the 20-minute timeout, the caches, the concurrency group with cancel-in-progress on
  pull requests, and its place in the required `checks-passed` job. The job runs on every pull
  request, every push to `master` and `platform/release`, the nightly schedule and manual
  dispatch. YAML parse check passes. No other job or the plan logic changed. The conformance
  suite did not run for this change.

- 2026-11-27: closed #1065 (medium). The SSE control event now writes `streamCursor` as a JSON
  string (for example `"streamCursor":"123"`), as PROTOCOL.md section 5.8 requires. Before, it
  wrote a number (for example `"streamCursor":123`). The reference client reads the field with
  `as_str`, so it dropped the cursor. Our client reads a string and still accepts a number
  from an old peer. The HTTP `Stream-Cursor` header did not change. `durable_adapter` asserts
  the quoted form, and a client test keeps the number fallback. `durable_reference_cross_run`
  has a sixth test: it reads the control event and asserts that the reference client keeps the
  cursor.

  The clippy gate also found 26 pre-existing pedantic lints in the `foundation_http` durable
  files. The fix clears them. The example moves its `use` items to the top of `main`, the test
  header helper takes `&SimpleHeader`, the route test allows the owned request its macro
  requires, and the adapter test compares the length. Counts: `foundation_http` durable tests
  30 + 5 + 6 + 7 + 4 pass, `foundation_core` durable tests 146 pass, clippy is clean in all six
  CI trees, and both guards report 0 violations.

- 2026-11-27: feature 08 T3 (cross-runs) landed. Two tests drive our in-process `HttpServer`
  with the Durable handler over the memory VFS. `foundation_http/tests/durable_cross_client.rs`
  runs our `DurableClient` over `NetioTransport`; `foundation_http/tests/durable_reference_cross_run.rs`
  runs the reference Rust client from `packages/client-rust`. Each file has 5 tests: create and
  append, byte-exact catch-up read, long-poll after an append, base64 Server-Sent Events (SSE),
  close and a read that reports the closure, fork creation and read. The 10 tests pass.

  The reference client is a dev-dependency on the public git repository, pinned to commit
  `461b40267aabd644558f9b19dbb9507dd5f691cf`, because the crate is not published on crates.io.
  `tokio` is a dev-dependency where a test needs it, not a workspace dependency: it is declared
  in `foundation_http/Cargo.toml` and the `foundation_core` native dev-dependencies.

  Verified: both cross-run test binaries pass (5 + 6), the other durable HTTP tests pass
  (`durable_adapter`, `durable_server`, `durable_parse`, `durable_status`,
  `durable_router_fallback`), `foundation_core` durable client tests pass (27), clippy reports no
  warning from the two new files, and both guards report 0 violations. The cross-run found issue
  #1065 (medium): the SSE control event sends `streamCursor` as a JSON number, and the reference
  client reads a string, so it ignores the cursor. Reads still pass. The full category matrix
  remains with the conformance suites (T1, T2 and the CI gate); the valtron suite, the skip
  register, the report and the runtime matrix are still open.

- 2026-11-27: features 02, 03 and 04 closed. The remaining tasks landed: `offset=now`, the
  multi-thread wake forwarding, the deterministic simulation test (DST), the full fork
  time-to-live (TTL) table and the oracle matrix. The wake test registers a `WakeCallback` on
  `DurableStreamReader`, appends, and asserts the callback fires. The DST runs one seeded
  scenario with create, append, producer retries, fork, TTL advance, reads and an expiry with a
  living fork. The same seed gives the same observation log.

  Tests: `tests/durable/` adds `now_catchup_empty`, `now_closed_returns_immediately`,
  `now_live_waits`, `wake_callback_fires_on_append`, `seeded_reader_replay_is_byte_exact`,
  `model_matches_driver`, `read_from_offset_is_suffix`, `append_read_is_byte_exact`,
  `producer_model_matches`, `drivers_agree`, `stream_seq_is_monotonic`,
  `ttl_slides_on_read_and_write`, `head_does_not_reset_ttl`, `fork_ttl_follows_table`,
  `expired_source_with_fork_is_soft_deleted`, `valtron_reader_spans_fork_boundary`,
  `fork_read_is_source_prefix_plus_tail`, `ttl_model_matches`, `fork_offsets_are_unique` and the
  golden `fork_stitching_has_a_golden_offset_layout`. The durable run is 145 tests, green 30
  times in a row. Clippy is clean for `multi,test-clock,vfs-testkit --all-targets`.
  `check-clock-params` and `check-time-sources` report 0 violations.

  T9 and T7: the crate has no feature that removes the valtron module. A new architecture test
  proves the core reader and producer tests never name the valtron wrapper. This is the closest
  available form of "valtron disabled". No cross-driver differential run exists in this crate.
  `drivers_agree` compares the store-trait path with the business-API path. A memory/SQL virtual
  file system (VFS) differential belongs to the feature 08 matrix.

  One durable run failed one test under host load. Thirty later runs passed and the name was not
  captured. The likely cause is a `valtron_test` 5 s timeout under load, not an assertion.

- 2026-11-27: feature 09 landed, all 16 tasks. The S3/R2 durable stream VFS
  lives in `foundation_nativeapis/src/shared/vfs/durable/` (object store
  contract, head manifest, spool, core) and
  `foundation_nativeapis/src/native/vfs/{s3,r2,object_http}.rs`. An append
  writes the local spool and fsyncs. The batcher seals one object per batch
  (segmented under `<root>/<stream hex>/seg/`, append-only under
  `<root>/<stream hex>/log/`). The head manifest is the authority. Its CAS uses
  `If-None-Match: *` and `If-Match`, and a stale ETag returns `412` and a
  retry. A store without conditional write uses a lease object. Reads load the
  head, range GET the segments, revalidate the head after the GET and cache the
  fetched ranges. The lifecycle is Live, Retired, DeleteIssued, Deleted with a
  24 h grace window; a live or retired object is never deleted.

  Configuration (`ObjectVfsConfig`, builder on every field): layout segmented
  (default), root `durable`, segment target 256 KiB (Rivet), batch cap 1 MiB,
  batch age 5 s, parallel PUTs 4 (S3 transfer manager), retry bound 3, publish
  policy batched, single PUT 8 MiB, part size 8 MiB, fork depth 64, head cache
  1 s, range cache 16 MiB, grace 24 h, lease timeout 30 s, revalidation
  retries 2, spool root required. R2 settings are region `auto`, path-style
  and CRC32. S3 defaults are `us-east-1`, virtual-host, no CRC32.

  Tests: `tests/durable_object_vfs` has 43 tests in four suites. Conformance
  runs the shared kit on S3 and R2 and on both layouts, and compares S3 with R2.
  Coverage covers one PUT per batch, multipart, cross-segment range reads,
  bounded reads, head revalidation, cache dedup, a missing object as an error,
  two writers with a CAS retry, crash after a segment PUT and idempotent
  replay, restart from objects, compaction, retention, orphan removal, the
  lifecycle, partial delete errors, capability probes, the weak-CAS lease,
  R2 settings, the manifest round trip, both key layouts, the measured PUT
  count, the batch age, a parallel flush of five streams, orphan cleanup for a
deleted head, and every `DurableStreamVfs` method on both VFS types. The HTTP tests
  shape S3 requests, sign with SigV4, parse a paged listing, run multipart,
  check every delete result, map 404/412 and assert the R2 region and CRC32.

  Verification: the full `foundation_nativeapis` test run with
  `--features vfs-object-http` passes. Clippy with the feature is clean.
  wasm32 checks pass for the default set and for `vfs,vfs-search`.
  `check-clock-params` and `check-time-sources` report 0 violations.

  Real service: MinIO and localstack are absent (`which minio localstack`).
  The HTTP tests use a stub transport. No real-service result is claimed.

  CI: `vfs-object-http` is a new run in `runs`, `clippy-runs`, `macos-runs`
  and `windows-runs`.

  Open: the socket transport under `ObjectTransport` is the host's injection
  point; this change ships the S3 REST client and SigV4, not a TCP/TLS client.
  A real MinIO/R2 run is still needed.

- 2026-11-27: feature 06 T12 landed. The body cap is configuration.
  `DurableLimits::max_body_bytes` holds the largest request body. It has
  `with_max_body_bytes` and an 8 MiB default. The doc comment names the wasm
  bridge default, the Cloudflare 100 MB cap and the nginx 1 MiB default. The
  HTTP adapter reads the value from the service limits. The constant
  `MAX_BODY_BYTES` is gone. A declared `Content-Length` over the cap still
  answers `413` and closes the connection. The bounded body read uses the same
  value.

  Tests: `tests/durable_server.rs` adds
  `oversized_body_answers_413_from_the_limit` and `body_at_the_limit_passes`.
  Each starts a server with a 64-byte cap from the context. A 65-byte declared
  body answers `413`. A 64-byte JSON body appends with `204`. The
  `durable_adapter` and `durable_server` suites pass. Clippy is clean for
  `foundation_core` and `foundation_http`. `check-clock-params` and
  `check-time-sources` report no violation.

  The conformance gate could not go green on this host. Other worktrees ran
  Android, iOS and wasm builds, so the host load was 300 to 800 on 10 cores.
  Three gate runs completed in 119, 124 and 151 s with 4, 3 and 8 failures. Every
  failure was a Vitest `Test timed out in 5000ms`, never an assertion or a wrong
  status. With `--testTimeout=60000` the server suite passed 337 of 338; the one
  failure was also a timeout. A live probe of a 10 MiB declared body answered
  `HTTP/1.1 413` in 11 ms, so the body cap is not the cause. A gate run on a
  quiet host is still needed, and the client suite did not run because the gate
  stops at the server step. The full `durable_server` binary also aborted after
  5 of 7 tests under this load; each test passes alone, and the file passed 7 of
  7 before the load rose.

  The change also cleared three old pedantic clippy lints that blocked the
  `foundation_http` library lint: `has_empty_offset`, the `serve_sse` length and
  the `handle_idle` length.

- 2026-11-27: feature 06 T9 landed. The Durable Streams `ConnectRPC` surface is
  `ewe.durable.v1.DurableStreamService` in
  `backends/foundation_connectrpc/proto/ewe/durable/v1/durable.proto`. The
  generated trait, register function, typed client and procedure paths land in
  `backends/foundation_connectrpc/src/shared/durable/`. `Create`, `Append`,
  `Read`, `Head`, `Close`, `Delete` and `Fork` are unary; `Read` returns one
  bounded page. `Subscribe` is a server stream: it sends one page at open, then
  one page per append or close, and ends after the close. It waits on the
  stream notifier before each read, so it never polls. It stops on the call
  cancel signal.

  The status table is one documented row for each `StoreError` variant. It
  reuses the HTTP intent: not found, gone, conflict, closed, bad request, rate
  limited and internal. The adapter is concrete over `DurableStream`; the host
  swaps the VFS, not the adapter.

  Tests: `tests/durable_stream_service_tests.rs` has 6 tests and they pass with
  the generated client over Connect on HTTP/2 cleartext. The names are
  `every_operation_round_trips_over_the_generated_client`,
  `refusals_map_to_connect_codes_over_the_wire`,
  `the_status_table_covers_every_core_refusal`,
  `subscribe_delivers_an_append_after_the_stream_opens`,
  `rpc_stream_resumes_from_offset` and `http_and_rpc_return_same_chunks`.
  Clippy is clean for the crate. The wasm32 check with
  `--no-default-features --features rpc` passes. `check-clock-params` and
  `check-time-sources` report no violation.

  Deviation: `requirements.md` section 3.9 lists `Read` as a server stream. The
  T9 brief defines `Read` as one bounded page and `Subscribe` as the live
  stream. This change follows the brief. The full crate test run has one
  pre-existing macOS failure, issue #858
  (`panic_in_handler_becomes_internal_via_recover`); it fails at the base commit
  too and is not related to this change.

- 2026-11-27: feature 05 T6 done. This closes issue #1052. The Durable Object
  (DO) SQLite executor is `foundation_db::DoSqlExecutor` in
  `backends/foundation_db/src/wasm/wasm_storage/do_sql_executor.rs`. It runs
  `exec` with bound parameters for a statement and a query, and
  `transactionSync` for one atomic transaction. The Workers Rust library
  (`workers-rs`) version 0.8 does not bind `transactionSync`, so
  `src/wasm/bindgen/cf/do_sql.rs` binds it. `workers-rs` version 0.8 also has no
  `durable_object` feature; the `durable` and `sql` modules compile
  unconditionally, so `do-sql` enables only the `worker` dependency. The
  `workers-rs` feature now also enables `wasm-bindgen-storage`, because its
  interop module needs the D1 binding type. The shared JavaScript value
  conversion for D1 and DO is in `wasm_storage/sql_value.rs`.

  Changed-row counts: the DO runtime's `rowsWritten` counter includes index
  rows, so an indexed insert reports two. The executor measures the rise of the
  SQLite `total_changes()` counter instead. That value matches the D1
  `meta.changes` count and stays zero for a query. The vendor `SqlCursor` also
  returns `rowsWritten` only after the cursor ends, so `execute` iterates the
  cursor to its end first.

  Real-service check: `backends/foundation_db/integration/do_sql/` is a
  standalone worker outside the workspace. `wrangler dev --local` runs it in
  workerd. Both `GET /run` (SQLite-backed Durable Object, migration
  `new_sqlite_classes`) and `GET /run-d1` (local D1 binding) return
  `{"ok":true,...}` with all nine VFS kit scenarios: `create`, `append`,
  `read_byte_exact`, `bounded_pages`, `fork_prefix`, `fork_append`,
  `soft_delete_cascade`, `ttl_slide` and `version_conflict`. `GET /rollback`
  returns `{"failed":true,"ok":true,"remaining":0}`: a failing second statement
  rolls the first insert back. No route is a stub.

  Checks: `cargo check -p foundation_db --target wasm32-unknown-unknown
  --no-default-features --features do-sql` and the same check with
  `wasm-bindgen-storage` (D1) are clean. A wasm check must disable the default
  features: the default set pulls `turso` → `multi`, which needs threads and
  does not build on wasm (issue #1056). `ci/packages.toml` `wasm-runs` now
  includes `--no-default-features --features do-sql`. `cargo clippy -p
  foundation_db` for the lib and for the wasm `do-sql` tree is clean.
  `cargo test -p foundation_db` passes. `check-clock-params` and
  `check-time-sources` report no violation.

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

- **01-durable-stream-types** — 🟡 in progress. All tasks are ticked. Open: the full
  `cargo test -p foundation_core` run is not recorded, and the BDD harness runs on one driver
  only.
- **02-resumable-reader** — 🟢 done. All tasks and success criteria are ticked.
- **03-idempotent-producer** — 🟢 done. All tasks and success criteria are ticked.
- **04-fork-and-retention** — 🟢 done. All tasks and success criteria are ticked.
- **05-storage-backends** — 🟡 in progress. All tasks are ticked. Open: the full
  `cargo test -p foundation_core` run is not recorded. The other success criteria have evidence.
- **06-http-rpc-surface** — 🟡 in progress. Open: T11 oracles (`injected_error_is_typed`,
  `security_headers_present`), the full crate test runs (ConnectRPC has #858 on macOS) and the
  workspace-wide `cargo check`.
- **07-durable-streams-client** — 🟡 in progress. T4 is ticked. Open: T5 retry, T6 producer and
  T12 oracles. The client core lacks the retry, producer, dynamic-header and clock APIs; those
  live only in the conformance adapter. Issue #1135.
- **08-conformance** — 🟡 in progress. T1 to T5, T7 to T16 and T18 to T21 are ticked. Open: T6
  (no separate valtron job) and T17 (no protocol-level fault-injection or fuzz oracle). The
  matching success criteria stay open. All other success criteria have evidence.
- **09-s3-r2-vfs** — 🟢 done. All tasks and success criteria are ticked.
- **10-subscriptions** — 🟢 done. All tasks and success criteria are ticked. The six server
  subscription tests pass and the gate has 0 skips.
- **11-hosted-object-transports** — 🟢 done. All tasks and success criteria are ticked.

Issue #1134 records the `Ctx` and `DurableStream` `Arc` cycle found in this review.

## Next steps

1. Feature 01 (core types and store trait) first.
2. Then 02 and 03 in parallel. Add 04, 05 and 06.
3. Run the conformance spike after 01 and the first HTTP adapter slice.
4. Add feature 07 (client). Then feature 08 (conformance gate).
5. Add the valtron wrapper tests after the core passes conformance.
6. Wire `foundation_log` (issue #155) and spec 83 `01-agent-rpc` / `05-cloudflare` to consume the
   core once 01–03 land.

_Created: 2026-10-30_
