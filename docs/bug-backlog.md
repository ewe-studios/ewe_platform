# Bug and limitation backlog

This is the single list of known bugs and limitations that are still open.

CLAUDE.md rule 9 says every bug found gets tracked and scheduled. This file is where they are tracked. Each entry names the crate and file, says what is wrong, and gives the likely fix.

How to use it:
- When you find a new bug, add it here in the same change, even if you are not fixing it.
- When you fix one, delete its entry in the PR that fixes it and name the PR in the commit message.
- Group entries by crate. Mark limits that are still hard-coded constants with **(rule 8)**.

Last refreshed: 2026-10-21, on `ai/async-stores` (foundation_ai on the async store traits; in-memory stores async-only). Before that: 2026-10-21, on `db/async-stores` (foundation_db async-only stores); 2026-10-20, after PR #96 (spec 65 feature 06 T5 updates). In flight on branches:
- `fix/connectrpc-cancel-and-limits`: connectrpc §1.
- `fix/limits-and-core-sweep`: the Docker limits cleanup and the foundation_core leftovers.

---

## 1. foundation_connectrpc

- **Ctx is never cancelled on the server** *(in progress: `fix/connectrpc-cancel-and-limits`)*.
  - h2 `run_streaming_h2` treats `H2IncomingFrame::Reset` as a clean end of body.
  - Nothing on the server fires `ctx.cancel_signal()` on a peer reset, a client disconnect, or a missed deadline.
- **Decompression bomb** *(in progress)*. Connect unary calls `c.decompress(&body, 0)`, where 0 means unlimited, in `shared/protocol/connect.rs`. Cap it at the read limit and return `ResourceExhausted` when it's exceeded.
- **No default body limit** *(in progress)*.
  - Server `read_max_bytes` defaults to 0 (unlimited). Pick a documented default (nginx 1 MiB, gRPC 4 MiB).
  - Reject an envelope whose length prefix is over the limit before allocating for it.
- **Client loses the response head and trailers on streams** *(in progress)*. Streaming `new_conn` drops them, so streaming clients never see the HTTP status or headers.
- **gRPC over h3 sends no status.** `native/h3_serve.rs` `send_h3_response` drops `response.trailers`, and the streaming path sends no `grpc-status`.
- **HTTP/1.1 RPC holds a worker for its whole life.**
  - `foundation_http`'s `Serve` trait is synchronous, so a long-lived h1 stream occupies a valtron worker until it ends. Enough of them use up the pool.
  - Needs an async or pending `Serve` contract.
- **Transports with no pollable fd fail.** `native/conn_io.rs` readiness waits return `Unsupported` for WireGuard overlay, io_uring completion sockets and openssl/native-tls.
  - With rustls, the fd is polled in the application's direction, so a TLS session that needs the opposite direction mid-stream waits until the timeout.
- **Conformance coverage.** Add an 8-thread variant of the conformance suite (task #27).
- **A streaming handler can't await its own cancellation.** `exchange_until_aborted`
  (`shared/router/dispatch.rs`) drops `response_side` — which holds the handler
  stream — before `run_streaming_h2` fires `cancel.cancel()` on
  `Abort::PeerGone`/`Abort::BodyFailed`, so a handler's `ctx.cancelled().await`
  never completes on a peer disconnect. Only `ctx.is_canceled()` (read from a
  parked `Ctx`) is observable. Fire the cancel before dropping the handler, e.g.
  cancel from a `Drop` guard on the handler future or restructure the select so
  the handler is polled once more. Surfaced by spec 63 T13.
- **The client's `cancel()` does not emit a wire-level reset.** `CancelSignal::cancel`
  stops the local signal but keeps the H2 socket open, so the server keeps
  streaming into a peer that has "cancelled" locally and never learns it was
  aborted. A cancelled client call should send `RST_STREAM` (or half-close) so
  the server observes the abort. Surfaced by spec 63 T13 — its e2e test drives a
  raw `RST_STREAM` because the typed client cannot.

## 2. foundation_netio

- **QUIC streams register no waker.** Every H3 future must wake itself, so it busy-polls a worker while idle. `h3_serve.rs` works around this. The quinn driver should wake stream tasks when they become ready.
- **`FramedRecv` buffers whole DATA frames.** It holds up to `DEFAULT_MAX_FRAME_SIZE` (16 MiB) before yielding. Stream within a frame, and make the size config **(rule 8)**.
- **`H3Connection::poll_accept_webtransport` loses streams.** It drops the accepted stream if its headers aren't ready yet. It also drops a non-WebTransport request whose headers it has already read.
- **`AsyncSendSafeBody` polls sync body iterators inline**
  (`shared/client/body_reader.rs`). `poll_next` calls the inner iterator's
  `next()`. Unless that iterator returns a "not ready" item (`Data::Retry`), a
  read that waits for the socket waits inside `poll_next` and holds the worker.
  Every async caller that reads a streamed body (the deployment API clients,
  `TurboPufferVectorStore`) has this. Fix: give the body iterators a
  readiness-based async form that returns `Poll::Pending` and wakes on socket
  readiness.
- **`SendSafeBodyBytesIterator` reports would-block inconsistently.** It returns `Err` for chunked bodies but `Ignore` for sized bodies, so its ~15 users can't tell "not ready" apart from bookkeeping items.
- **`BatchReader` logs every `WouldBlock` at `error!`.**
- **Case-sensitive `chunked` check.** The request reader's `transfer_encodings.iter().position(|n| n == CHUNKED_VALUE)` compares case-sensitively, unlike the lowercase set check just before it.
- **Non-blocking work not done yet:**
  - non-blocking connect and TLS handshake (task #23);
  - a pushed request body should wait on pipe readiness (task #24);
  - the remaining transports (task #25).
- **Unix-socket sends fail on macOS.** The request-intro / send path calls
  `set_read_timeout_as`, which sets `SO_RCVTIMEO`; macOS rejects `SO_RCVTIMEO`
  on a Unix domain socket with `EINVAL`, so `unix_socket_builder_send_reaches_server`
  fails with `Invalid argument (os error 22)`. Skip the read timeout for unix
  sockets, or treat `EINVAL` as "timeout unsupported" rather than failing the intro.
- **`HttpClientConnection::connect` no longer fails eagerly.** The non-blocking H1
  work made `connect` park on readiness, so it returns `Ok` before the TCP
  handshake. `test_connection_with_mock_resolver` asserts an eager
  `ConnectionFailed`/`ConnectionTimeout` and is now stale. Restore eager connect,
  or update the test to the lazy contract.
- **Ignored tests.**
  - 14 network-dependent tests are ignored: `http_external_validation`, `tls_integration`, `http_server_integration`. Make them hermetic (task #31).
  - 33 doctests are ignored (task #33).
- **Wasm SSE parser errors still stall instead of terminating the stream.**
  `wasm/client/stream.rs` `WasmSseIterator::next` now distinguishes `WouldBlock`
  (a real "wait" signal, kept in `EventSourceError::ReadError(io::Error)`) from a
  genuine read or parse error and logs the latter with `tracing::error!`. What is
  left: a non-`WouldBlock` parse error (a malformed event) still returns
  `Stream::Wait` rather than ending the stream, so a permanently malformed feed
  stalls the consumer instead of failing. Surface it as
  `Stream::Pending(SseProgress::Failed(msg))` then `None`, like a fetch failure.
- **Wasm `send_sse_async` returns a non-parking stream.** It boxes via
  `into_future_stream()` (`StreamAsFutureStream`, a raw lens whose `poll_next` is
  `Poll::Ready(iter.next())`), so `Wait`/`Pending` are forwarded as items and the
  stream busy-polls instead of parking. Fix in spec 65 feature
  `06-wasm-async-streaming`: rename the raw lens to `into_pure_future_stream()` and
  make `into_future_stream()` an await-friendly lens that parks on `Wait`/`Pending`
  (with `impl WakeableStream for WasmSseIterator` — a `WakeRegistry` fired by its
  `spawn_local` reader on each queue push). `send_sse_async` keeps the full-enum
  item type, so it stays on `into_future_stream()` (now parking) rather than
  switching to `into_next_stream()`. *(Fixed in spec 65 feature 06 T3.)*

## 3. foundation_http

- **h2 connection polls on a 10 ms timer** (the "47b gap" in its docs). `ConnectionHandler`'s idle path spins and delays instead of waiting for TCP readiness.
- **Ignored write errors, and leftover body bytes on keep-alive.**
  - The middleware-response and 404 paths ignore render and write errors (`let _ = …`).
  - They also leave an unread request body on a kept-alive connection, so the leftover bytes are read as the next request. This was fixed for ConnectRPC in #96.
- **`H3AcceptTask` problems:**
  - ignores the result of `valtron::send(FutureTask)`;
  - polls on a fixed 5 ms delay, which is not readiness-driven and is a hard-coded limit **(rule 8)**;
  - drops a request whose first `poll_accept` errors.

## 4. foundation_core / valtron

- **`StreamIteratorExt::collect` shadows `Iterator::collect`.** Any iterator
  whose items are `Stream<D, P>` gets two `collect` methods, so a plain
  `.collect()` into a `Vec` is ambiguous (`streams/{sendable,non_sendable}.rs`).
  Likely fix: rename the valtron one (e.g. `collect_stream`), keeping the old
  name deprecated.
- **A stream's first empty pull still blocks once.**
  `NotifyQueueStreamIterator::next` stops blocking only after a consumer
  registers a wake, so the first pull on an empty queue by `.values()` /
  `TaskStatus::from_stream` still waits `max_spins` timed waits
  (`executors/local.rs`). Likely fix: the async conversions register their
  wake before the first pull.
- **Cross-thread wakes don't wake an idle single-threaded executor.** The native idle wait is a `park_timeout` that nothing unparks, so tasks woken from another thread still poll at up to 100 ms.
- **Hard-coded valtron limits (rule 8):** `MAX_YIELD_DURATION`, `DEFAULT_YIELD_WAIT_TIME` (4 s), `BACK_OFF_*`, and `ThreadedIterFuture`'s queue size of 16 and its 10 ms sleep.
- **`ThreadedIterFuture` (multi) busy-polls.**
  - It polls with a no-op waker plus `yield_now`.
  - It uses `thread::sleep` for backpressure.
  - It ignores the result of `let _ = worker_queue.push(Err(e))`.
- **`BackgroundJobRegistry`:** the queue is unbounded (it needs a capacity setting), and `new` panics if it can't spawn a thread instead of returning an error.
- **`local_executor` scenario tests only run under `multi`.** 14 of them collect `TaskStatus` values; rewrite them to run in both configurations.
- **Misleading docs.** `send_any_task` / `send_typed_task` are documented as delivering to the global queue, but they differ from `any_task` / `typed_task` only in their trait bounds.
- **`BytesPool` has no bounds.**
  - It is unbounded and keeps returned buffers of any size, so it needs a capacity and a maximum buffer size **(rule 8)**.
  - `acquire_with_capacity` counts both a hit and an allocation when the pooled buffer is too small.
  - `PooledBuffer::owned` builds a throwaway pool for every buffer.
  - `PooledBuffer` has no `Debug`.
- **`read_pooled_buffer` returns `None`** (the EOF signal) when `max_len == 0`.
- **A readiness signal that never asks for a resume stalls on wasm.** Since spec 65
  feature 06 T5 the JS wasm `run_until` hands control back to the event loop when
  every task is parked on `TaskStatus::Depends` (instead of spinning), and relies
  on the code that flips the signal to call `request_executor_resume()`
  (`WakeSignal` does). `BoolSignal`, `QueueReadiness` and `QueueVacancyReadiness`
  do not, so a wasm task parked on one whose producer is JS-side async work is
  never re-run (on native the executor re-checks readiness itself). Fix: have
  their producer side (`ConcurrentQueue` push wrappers, `BoolSignal::set`) call
  `request_executor_resume()`, or replace them with wake-registry-backed signals.
- **A blocking helper called from inside a task spins forever on wasm.**
  `executors/local.rs`: while a task is being polled its slot still counts as
  active (`has_active_tasks`), so a nested `run_until` (any sync "block on"
  helper reached from a task's `next_status`, such as the old wasm
  `HttpClient::send` via `block_on_collect`) never sees "nothing runnable" and
  loops on `CanProgress(None)` without yielding to JS, freezing the isolate.
  `block_on_collect` (`non_sendables.rs`/`sendables.rs`) has no wasm callers left
  but still compiles there. Gate `block_on_collect`/`block_on_future` to
  `not(target_family = "wasm")`, and make the nested-run case return instead of
  spinning.
- **Release coupling.** `foundation_macros` calls `foundation_core::valtron::initialize_current_thread`, so the two crates need a coordinated version bump when published.
- **`sleep_async` as a `select` timeout under `block_on_future` can fail to fire.** When the pool is contended (e.g. during shutdown), the `SleepingTask` timer may never resolve, hanging the future raced against it. Surfaced by the proxy's gRPC health probe (§9), which works around it with an outer `recv_timeout`.
- **`SharedByteBufferStream::peek` conflates EOF with `WouldBlock`.** The
  `PeekableReadStream` impl (`io/ioutils/mod.rs`) maps `PeekState::EndOfFile`
  (and `NoNext` / `EndOfBuffered`) to `Ok(0)` — the same result as "no bytes
  yet" — so a peeker cannot tell a peer that closed the socket from one that is
  just slow. `foundation_http`'s h2 server therefore parks forever on a
  half-closed peer instead of detecting the disconnect. Spec 63 T13 works around
  it by having the connectrpc client pump send an explicit `RST_STREAM(CANCEL)`;
  the peeker itself should return a distinct EOF signal.

## 5. foundation_nostd / foundation_wasm / foundation_iogate

- **`SpinWaiter`** (foundation_nostd, still used on wasm) rounds sub-millisecond waits down to zero, and its spin calibration is arbitrary.
- **Unused import** at `foundation_wasm/src/capability.rs:24` on wasm32.
- **Interval callbacks run under the registry lock.** `host_runtime.rs`
  `internal_api::run_interval_callback` calls the callback while holding
  `RECURRING_INTERVAL_CALLBACKS`, so a callback that registers or cancels an
  interval re-locks that mutex (a std `Mutex` on wasm: panic or deadlock). The
  schedule registry had the same bug and was fixed in spec 65 feature 06 T5 by
  taking the callback out before running it; intervals recur, so clone the
  `WrappedItem` (an `Rc`/`Arc`) out of the registry, drop the lock, then call.
- **iogate's undeclared `uring` feature.**
  - `native/completion_socket.rs` gates code on `feature = "uring"`, but `Cargo.toml` declares no such feature, so that code can never compile in.
  - `ReadHalf`, `peer_addr` and others are dead as a result.
  - Declare the feature or remove the code.

## 6. foundation_deployment_docker / foundation_deployment

- **Seven client methods build requests by hand.**
  - The methods: `upload_to_container`, `get_container_archive_info`, `exec_inspect`, `system_info`, `system_ping`, `system_data_usage`, `network_inspect`.
  - They read success bodies whole with no cap, throw away error bodies, ignore `DockerLimits`, and carry stale docs.
  - `upload_to_container` takes the whole tar as a `Vec<u8>`; it should stream.
  - `system_data_usage` returns untyped JSON. Route all seven through the generated functions, fixing the spec conversion where needed (rule 5).
- **`send_json` has no body cap.** It reads a successful JSON body whole. Add `max_json_body` to `BodyLimits` **(rule 8)**.
- **`LogFrameDecoder::decode` returns `None` for two cases:** "need more bytes" and "skipped an unknown frame type". Callers have to compare buffer lengths to tell them apart.

## 7. foundation_deployment_platform

- **`wait_for_stdout`:** re-reads the whole log on every poll, misses a message split across frames, and scans stderr too.
- **`docker_container_macro_supports_async_functions`** fails instead of skipping when no Docker daemon is running.
- **Docker-gated tests check only that the socket file exists.** `docker_available()` in `tests/container_integration.rs`, `tests/ssh_backend_tests.rs`, `tests/runner_integration_tests.rs` and `tests/image_build_integration.rs` returns true when `/var/run/docker.sock` exists, also when no daemon listens on it. Then `--ignored` runs fail with "Docker daemon not reachable" instead of skipping. Fix: add one shared test helper that connects with `DockerClient::connect_with_defaults` and pings the daemon, and use it in all Docker-gated tests (foundation_proxy has the same helper, see §9).
- **Vagrant box lookup runs a `curl` subprocess** (`src/import/mod.rs`, near line 512). The comment says this avoids "tokio runtime conflicts", but no tokio runtime is used now. The subprocess needs `curl` installed and returns no structured errors. Fix: use the valtron-based HTTP client from `foundation_netio` with a bounded, streamed read of the JSON response, and remove the comment.
- **Inline tests remain** in `docker/wireguard.rs` (rule 7).
- **Warnings:**
  - deprecated `SimpleHttpClient`, used in `wait_for.rs` and in `foundation_deployment/src/provider_client.rs`;
  - unused import `PortMapping` in `docker/wireguard.rs:21`.

## 8. foundation_openapi / foundation_codegentools

- **Boolean query parameters are generated as `Option<String>`** (for example `ContainerStatsArgs.stream` / `one_shot`), probably for every endpoint.
  - Generate `Option<bool>` and serialise it.
  - Related: Docker `image_build` can only send one tag, because `t` isn't treated as an array.
- **Inline tests remain** in `unified/emit.rs` and `unified/analyzer.rs` (rule 7).
- **genapi's flags are ignored in two places.**
  - Its post-step `cargo fix` always runs in the dev profile, ignoring `--profile`.
  - A plain `generate hetzner` ignores the crate's declared operation selection and generates the full API.

## 9. foundation_proxy

- **gRPC health probe leaks a thread + H2 connection when its inner timeout hangs.**
  - `health.rs::probe_grpc_health` drives `grpc_health_check` on a throwaway thread and bounds it with `recv_timeout` as a backstop, because the inner `select` + valtron `sleep_async` timer can fail to fire (see §4) and `H2Transport`'s pump parks on the socket read with no read timeout.
  - When the backstop trips, the inner `block_on_future` (and its H2 pump task) is abandoned — a per-occurrence thread/connection leak. Rare (the timer only fails under pool contention, e.g. proxy shutdown), but a flapping or non-responsive backend could repeat it every probe interval.
  - Likely fix: give `H2Transport` a real read/keepalive timeout so the pump self-terminates, then drop the backstop thread.
- **Backend-state persistence can lose an update.** Each control-socket
  connection runs on its own thread, and `ProxyStateStore::save_backend_state`
  / `save_tls_cert` (`src/persistence.rs`) load the whole persisted state,
  change one entry and save it back. Two admin commands at once can each save
  over the other's change. Fix: serialise the read-modify-write behind a
  mutex in `ProxyStateStore` (or store each backend's state as its own record).
- **Docker-gated tests check only that the socket file exists**
  (`tests/proxy_integration_tests.rs`, `docker_available()`). When the socket
  exists but no daemon listens on it, the `--ignored` runs fail instead of
  skipping. Fix: use a shared helper that pings the daemon (see §7).
- **The `mut` on `h3_local_addr` warns without the `h3` feature**
  (`src/server.rs`, `ProxyServer::start`). Fix: build it from a `cfg`-gated
  expression instead of assigning inside the `cfg` block.

## 10. foundation_ai

- **Non-streaming wasm `generate` is unsupported.** The providers' sync
  `generate` path calls `HttpClient::send`, which on wasm now returns an error
  (spec 65 feature 06 T5: a sync send cannot wait for `fetch` without freezing the
  isolate), and its retry backoff `backend_utils::sleep` is a no-op on wasm (T4).
  So one-shot generation fails fast on wasm instead of deadlocking, but does not
  work. Likely fix: an async `generate` seam (e.g. `generate_async`) over
  `send_async`, or route wasm `generate` through the lazy `send_sse`/`stream`
  machinery and collect its result async.
- **The `agent` tool busy-loops on `Stream::Wait`.**
  `src/agentic/tools/agent.rs` (~l. 492) drains a sub-agent's stream
  synchronously and `continue`s on `Init | Ignore | Wait | Delayed`, so a
  sub-agent waiting on I/O spins the caller's thread (and on wasm never yields
  to the event loop, so the I/O can never complete). Likely fix: drive the
  sub-agent as its own task and park on it (`valtron::park_on`), or report
  `Working` and return on `Wait` the way it already does on `Pending`.
- **HTTP providers read whole error bodies.** Every provider's non-2xx path
  (`openai_provider`, `openai_responses_provider`,
  `anthropic_messages_provider`, `backends::model_info`) collects the full
  body to extract the vendor message. Likely fix: read at most a configured
  number of bytes (a field on each provider config, default 64 KiB like Go
  `net/http`'s error-drain limit) before parsing.
- **`ResponsesProvider::fetch_model_info` has no success test.**
  `tests/models/provider_model_info_tests.rs` defines `GPT_JSON` (a model
  object) but no test uses it, so only the error and placeholder paths of the
  Responses provider are tested. Fix: add the success test (a `GET` with the
  bearer token that parses `GPT_JSON`), as the Anthropic one does.

- **Embeddings are generated synchronously inside async code.**
  `EmbeddingProvider::embed` is sync, and `CachedEmbeddingProvider` calls the
  model's sync `generate` on a cache miss (`src/agentic/embedding.rs`,
  `embed_single`). `ContextProvider::search` (semantic recall, the
  `search_context` tool) and the `shed` tool's embedding search call it from
  async code, so an HTTP embedder blocks the worker, and on wasm the sync
  `generate` fails (see the first entry). Fix: an async `embed` /
  `embed_batch` over an async generate, awaited by the callers.
- **A flush that finds another flush running returns `Ok(0)` at once.**
  `MessageApi::flush` (`src/agentic/message_api.rs`) guards with an
  `AtomicBool`; a second caller doesn't wait. The agent loop's `Flushing`
  state then resumes with the buffer still at the threshold, so it re-enters
  `Flushing` every step until the other flush (e.g. a concurrent
  `AgentSession::end`) finishes. Fix: let the second caller await the running
  flush (an async lock or a completion signal) instead of returning early.
- **A cancelled flush can write a record twice.** A record stays in
  `MessageApi`'s `retry` slot until its write returns, so a flush future
  dropped after the store accepted the write but before it returned writes the
  record again on the next flush. Fix: make the writes idempotent (upsert on
  the document id for `append_with_id`, and give memory records a stable id),
  or clear the slot through a store acknowledgement.

## 11. foundation_db and its sync callers

Follow-ups from making the I/O backends async-only (branch `db/async-stores`).
- **`StateStore::set` ignores its `resource_id`.** Every backend (file,
  libsql, D1, R2) keys the write by `state.id`, so `set("a", &state_b)` writes
  `b` (`src/core/state/*.rs`, `src/native/{libsql_store,d1_kvstore,r2_blobstore}.rs`).
  `NamespacedStore` now rewrites `state.id` itself (fixed in F04 T8). Fix:
  drop the parameter, or key by it and set `state.id` from it in one place.
- **`R2Store` parses the R2 REST JSON by hand** (`src/native/r2_blobstore.rs`).
  The generated Cloudflare types live in `foundation_deployment_cloudflare`,
  which depends on `foundation_db`, so they can't be used here (rule 5). Fix:
  move the generated R2 types to a crate both can depend on. Related:
  `worker-stub.js` has no object-listing endpoint, so the R2 listing is only
  tested against the in-process mock in `tests/r2_state_store_tests.rs`, not
  against the wrangler stub.
- **R2 state listings hold every matching ID before streaming.** The listing
  pages through R2's cursor, sorts the IDs, then fetches each state lazily;
  the ID list itself is in memory. Fine for deployment state; a store with
  very many resources would want a cursor-driven stream (R2 already returns
  keys in order).
- **D1's state `init` sends several statements in one `/query` call**
  (`src/native/d1_kvstore.rs`). It (and libsql's) was missing the `;` after
  `CREATE TABLE`, so `init` always failed; fixed in F04 T8 and covered for
  libsql by `tests/libsql_state_store_tests.rs`, but the D1 path is only
  exercised by the wrangler integration tests (`CF_INTEGRATION_TEST=1`).
- **Auth tables mix timestamp units.** The `created_at` defaults in the core
  migrations are milliseconds (`strftime('%s','now') * 1000`), while
  `AsyncStorageCleanup` compares `expires_at` with Unix seconds and the IdP
  server writes milliseconds for its own rows. Fix: pick one unit per column,
  document it in the migrations, and make cleanup follow it.
- **`KVWasmStorage`'s rate limiter isn't atomic**
  (`src/wasm/wasm_storage/kv_wasm.rs`): a record reads the entry, then
  writes it back, so two concurrent hits can count once (and KV is
  eventually consistent across locations). Fix: use a Durable Object or D1
  for rate limits on Workers, and document KV as best-effort.
- **Databases created by the old rate-limit code have the wrong table.**
  Before F04 T11 a store that wasn't migrated created
  `rate_limits (key PRIMARY KEY, count, window_start)`; the store now uses the
  migration 014 shape (`id`, `key UNIQUE`, `count`, `window_start`,
  `window_end`), which `CREATE TABLE IF NOT EXISTS` won't add to an existing
  old-shape table. Fix: a migration that rebuilds an old-shape table (rate
  limit rows are disposable), or drop it on detection.
- **No CI job builds the example workspaces** (`examples/cf-*`,
  `apps/answerme-agent`); `examples/cf-login-app`'s manifest had stopped
  loading unnoticed. Fix: an examples job (wasm32 for `cf-*`). The feature
  runs that had rotted (foundation_db `libsql`, foundation_nativeapis
  `vfs-native,vfs-d1,vfs-r2`, foundation_vectors `sql-store`, foundation_auth
  `server-test`) are in `ci/packages.toml` since F04 T12.

## 12. foundation_auth

- **`PowService` checks leading zero bits against a byte count.**
  `verify_solution` (`src/server/services/pow_service.rs`) compares
  `count_leading_zero_bytes(&hash)` — which counts *bits* — with
  `difficulty / 8`, so a difficulty of 22 bits only needs 2 zero bits. Its
  `solve_validity_secs` field is never read either. Fix: compare bits with
  `difficulty`, rename the helper, and either enforce the solve window or
  drop the field (update the tests, which solve at difficulty 8).
- **`register` answers a duplicate email with `200 {"error": "user_exists"}`**
  (`src/server/handlers/core.rs`); spec 48 F03 says `409`. Fix: return
  `IdpError::Conflict`.
- **`header_name_eq` compares header names through `Debug` formatting**
  (`src/server/handlers/core.rs`), so it depends on `SimpleHeader`'s `Debug`
  output. Fix: compare with the header's canonical name.
- **IdP integer columns are cast with `as`** (`interval_secs`, `counter`,
  `failed_login_attempts` in `src/server/storage.rs`): a negative or oversized
  value wraps silently. Fix: `u32::try_from` and report a `Parse` error.
- **`--features server-native` tests don't link here.** `webauthn-rs` needs
  OpenSSL 3 symbols (`EVP_Q_digest`, `OPENSSL_sk_*`) that the build host
  doesn't provide. Fix: vendor OpenSSL (`openssl/vendored`) or document the
  system dependency.
- **Unused dependencies on wasm.** `cargo check --target wasm32-unknown-unknown
  -p foundation_auth --no-default-features --features wasm` warns that
  `argon2`, `foundation_errstacks`, `pbkdf2`, `time` and `uuid` are unused
  there. Fix: make the native-only ones target-specific dependencies (or gate
  them behind the features that use them).
- **`foundation_db::wasm::{WasmCredentialStore, session}` duplicate
  `CredentialStorage`.** Their docs say they exist because the sync
  `CredentialStore` routed D1 through blocking calls; that trait is gone (spec
  50 F04), and `CredentialStorage` over a D1 `StorageProvider` already serves
  `AsyncCredentialStore` on Workers. Fix: fold callers onto
  `CredentialStorage` and delete the duplicates.
- **`NativeOAuth` and `PasswordAuthClient` read an empty body.**
  `src/native/oauth.rs` (`extract_body_text`, `error_body_text`) and
  `src/native/password_auth.rs` (`extract_body`) read the response through
  `get_body_ref()`. The client gives a lazy `SendSafeBody::Stream`, which
  these helpers read as an empty string, so a token or login answer fails to
  parse and an error answer has no message. Fix: take the body and read it as
  a stream (status first; an error body only up to a configured limit), as the
  `shared` clients do.
- **The `shared` HTTP clients read bodies synchronously and whole.**
  `discovery`, `jwks`, `introspection`, `userinfo` and `upstream_client`
  (`src/shared/`) call the sync `try_collect_bytes` inside `async fn`s (a
  blocking read on the executor; on wasm it cannot wait for `fetch`), and
  read an error body whole before they use the status. Fix: read with
  `collect_bytes_async` (`AsyncSendSafeBody`), check the status first, and
  read an error body only up to a limit on a config (64 KiB, as
  `foundation_deployment`'s `BodyLimits`).
- **`NativeOAuth` keeps sync twins** (`exchange_code`, `client_credentials`,
  `refresh_token`) next to the `*_async` methods (`src/native/oauth.rs`),
  against the async-only API rule. Fix: keep only the async methods (as the
  wasm client now does) and let callers `.block()`.

## 13. foundation_nativeapis

- **The blob-backed D1/R2 deltas can't list.** `BlobDelta`
  (`src/native/vfs/blob_delta.rs`, behind `D1Delta`/`R2Delta`) keeps each
  file, its metadata and its whiteout as separate blobs, and a blob store has
  no key listing: `list_async` and `list_whiteouts_async` return nothing and
  `reset_async` is unsupported. Fix: keep a per-directory index blob (or use
  D1's key-value listing, `list_keys_async`, for the D1 delta).
- **A blob-delta write rewrites the whole file.** Each `write_at_async` reads
  the blob, splices the bytes in and writes it back, so a large file written
  in small pieces is O(n²). Fix: chunked storage (the unused
  `d1_delta::chunking` module sketches it) with a configurable chunk size.
- **The VFS shim never uses the D1/R2 deltas.** `src/native/vfs/shim/mod.rs`
  logs "impls pending" and falls back to the memory delta for
  `DeltaBackend::D1` / `R2`; the deltas now exist (as `SyncFs<D1Delta>` /
  `SyncFs<R2Delta>` for the sync overlay). Fix: build them from the shim
  config.
- **`FjallDocumentStore` scans buffer their rows.** `scan_async`,
  `scan_all_async` and `scan_from_async` collect the matching rows into a
  `Vec` before streaming them (`src/shared/vfs/fjall_document_store.rs`);
  `scan_all_async` is unbounded. Fix: walk the fjall index range lazily.

## 14. foundation_cronjobs

- **`CronJob::next_fire` ignores the cron expression.** It always returns five
  minutes from now (`src/scheduler.rs`, marked TODO), so every job runs on the
  same fixed interval. Fix: parse `cron_expr` with a cron crate (`cron` or
  `saffron`) and compute the real next fire time; reject an invalid
  expression at `register`.
- **`CronScheduler` never runs a job.** The crate docs (`src/lib.rs`)
  describe tasks parked on `Delayed` until each fire time, execution, result
  recording and missed-job catch-up (`MissedJobPolicy`); none of it exists —
  the scheduler only registers jobs and saves/loads their state. Fix:
  implement the run loop (one valtron task per job) and the missed-job
  policy, or cut the docs down to what exists.

## 15. foundation_cedar

- **`SqlPolicyStore` reads its row once per field.** `load_from_store` calls
  `load_schema`, `load_policies` and `load_entities`, and each runs the same
  `SELECT` (`src/storage/sql_store.rs`). Fix: a `load_all` on `PolicyStore`
  (default: the three calls) that the SQL store overrides with one query.
- **`derive_more` is an unused dependency** (`cargo` warns). Fix: drop it
  from `Cargo.toml`.
- **`unreachable call` warnings in `src/core/request.rs`.** The three
  `EntityId::from_str(id).map_err(..)` calls map an error type that can't
  occur (`Infallible`), so the compiler warns on each `map_err`. Fix: drop the
  `map_err` (or use `EntityId::new`) so the build is warning-free.

## 16. Workspace / process

- **Shared target dir clobbering.** When parallel worktrees share one `CARGO_TARGET_DIR`, crates such as foundation_core and foundation_macros build with the same hashes and overwrite each other's test binaries and proc-macro artifacts mid-run. That gives stale results and `E0460` doctest failures.
  - Give each worktree its own target dir, or `RUSTFLAGS=-Cmetadata=<id>`. Note that the latter rebuilds all dependencies.
- **`crates/ewe_deployables` doesn't build.** It isn't a workspace member, so
  nothing compiles it, and it still implements an older `Deployable` shape
  (sync `TaskIterator`s, sync `store(&client).get_typed(..)`), which no longer
  exists. Fix: port it to the current async `Deployable` / `StateStore` and
  add it to the workspace (or delete it if superseded by the per-provider
  deployment crates).
- **Cloudflare crate size.** Split the generated Cloudflare groups into per-group crates (task #28).
- **Wasm runtime tests never run in CI** (spec 68 T4). CI's wasm step is a
  compile gate only (`scripts/ci/ci.py --step wasm` → `cargo check --target
  wasm32-unknown-unknown`). The runtime suites (`foundation_core/integration/wasm_tests`
  via `wasm-testbed deno`/`web`, the bindgen-deno path, `cf-cloud-provider` under
  miniflare) are local mise tasks, and `integration/wasm_tests` is a standalone
  crate outside the root workspace, so not even its compile is gated. That is how
  the feature 06 T5 isolate freeze shipped behind green CI, and how the
  `wasm-embedded-js` testbed stopped building (deno_v8 had no JS engine selected)
  unnoticed. Add a CI job that runs `mise run test:wasm-testbed:deno` (embedded
  Deno; no browser needed) and `mise run test:wasm-bindgen:deno`.
- **`wasm-testbed` warns on `Command::Bindgen(args)`** (`foundation_testbed/src/bin/wasm-testbed.rs:37`):
  `args` is unused unless the `wasm-bindgen-test` feature is on (e.g. under
  `--no-default-features --features wasm-embedded-js`). Gate the binding with the
  same `cfg` as its use, or prefix it with `_` in the other arm.
- **Running a wasm mise task installs every tool in `mise.toml`.** `mise exec`/`mise run`
  first installs all pinned tools (bacon, cargo-nextest, cargo-audit, python…),
  building several from source. On a small disk that fills it, and where the
  GitHub attestation API is blocked the python install fails, so
  `test:wasm-bindgen:deno` cannot start even though it only needs deno and
  wasm-bindgen. Split the wasm tools into their own mise env/profile, or have the
  task invoke the two binaries via `mise which`.

## 17. foundation_vectors

- **`FjallVectorStore` runs fjall inline in its async methods**
  (`src/fjall_store.rs`). fjall has only a sync API. The calls read and write
  the page cache without `fsync`, but the first use of a namespace scans all
  of its vectors to rebuild the index, and `flush` (`fsync`) is sync. Both can
  hold a worker for a long time. Fix: run them on a blocking-work seam when
  valtron has one (or in a task of their own), and make `flush` async.
- **`SqliteVecVectorStore` ships only a linux-x86_64 `vec0.so`**
  (`vendor/vec0.so`). `open` fails on every other platform. Fix: vendor the
  extension for each supported target, or build sqlite-vec from source.
- **`SqliteVecVectorStore` tables from before spec 50 F05 T1 do not work.**
  T1 changed the `vec0` schema (text primary key, `metadata` column, distance
  metric). An old `vec0_{ns}` table gives SQL errors. Fix: detect the old
  schema on `open` and migrate it, or fail with a clear message.
- **`TurboPufferVectorStore` is tested only against a mock.** The mock
  follows the request and response types of the official Python client
  (`turbopuffer` 2.14.0), because the documentation site was not reachable.
  Two details are not proven against the live service: that `deleted_ids`
  lists only ids that existed (`delete_async` reports `NotFound` from it), and
  that `include_attributes: ["vector"]` returns the vector. Fix: an opt-in
  test against a real namespace (API key from the environment).
- **`TurboPufferVectorStore` does not retry HTTP 429.** `TurboPuffer` throttles
  writes with 429 (backpressure). The store reports it as an error. Fix: retry
  with backoff, with the attempts and delays on a config.
- **The `sql-store` feature does not build for wasm32.**
  `cargo check --target wasm32-unknown-unknown -p foundation_vectors
  --features sql-store` fails in `foundation_core` (`idle_wake` missing, a
  `NoopMutex` `PoisonError` mismatch). The crate takes `foundation_db` with
  its default features on every target, so `foundation_core/multi` is on for
  wasm. This was so before spec 50 F05 T1. CI builds `sql-store` only for
  native, so it does not see it. Fix: take `foundation_db` without default
  features on wasm32, then add `sql-store` to the crate's `wasm-runs` in
  `ci/packages.toml`.
- **`hybrid_search` gives the reranker no texts** (`src/fusion.rs`). It calls
  `Reranker::rerank(query, fused, &[])`, so a reranker cannot read the
  candidate documents. Fix: take a text lookup (id → text) as an argument, or
  change `Reranker` to fetch the texts it needs.
- **The SQL-backed stores scan a whole namespace per search**
  (`SqlVectorStore::search_async`): memory is bounded (`TopK`), but time is
  O(n·d) per query. Expected for the portable fallback; large namespaces
  should use `LibsqlVectorStore` (native index) or a dedicated vector backend.

## 18. foundation_testbed

- **The embedded JavaScript runner uses a tokio runtime** (`src/wasm/embedded_js.rs`, `tokio_runtime()`, and the optional `tokio` dependency behind `wasm-embedded-js` in `Cargo.toml`). The event loop and timers of `deno_core` (`reactor_tokio.rs`) need a tokio runtime, and `deno_websocket` uses tokio for its TCP connection. This is the only tokio runtime in `backends` outside the cross-runtime tests of valtron. The project rule is "valtron always". Options:
  - keep it as a documented third-party boundary: a current-thread runtime, local to this module and behind the feature flag;
  - or drive the event loop of `deno_core` another way (poll `JsRuntime::poll_event_loop` from a valtron task with a valtron timer), if `deno_core` lets us replace its reactor.

## 19. Apps (tokio and axum)

- **The apps still use tokio and axum, not valtron and `foundation_http`.** This is `bin/platform` (and its templates `api-app`, `static-html-app`, `web-app`), `examples/devbinary`, `examples/web/wello`, `examples/todo`, `crates/channels` and `crates/domain`: tokio/axum servers and tokio tests. `crates/channels` also has inline `#[cfg(test)]` modules (`src/broadcast.rs`, `src/mspc.rs`, `src/executor.rs`), which go against rule 7. Tracked in [#137](https://github.com/ewe-studios/ewe_platform/issues/137).

## Paused

- Windows/macOS VM images (#69).
