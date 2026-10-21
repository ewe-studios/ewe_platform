# Bug and limitation backlog

This is the single list of known bugs and limitations that are still open.

CLAUDE.md rule 9 says every bug found gets tracked and scheduled. This file is where they are tracked. Each entry names the crate and file, says what is wrong, and gives the likely fix.

How to use it:
- When you find a new bug, add it here in the same change, even if you are not fixing it.
- When you fix one, delete its entry in the PR that fixes it and name the PR in the commit message.
- Group entries by crate. Mark limits that are still hard-coded constants with **(rule 8)**.

Last refreshed: 2026-10-21, on `db/async-stores` (foundation_db async-only stores). Before that: 2026-10-20, after PR #96 (spec 65 feature 06 T5 updates). In flight on branches:
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
- **Wasm SSE parser errors are swallowed.** `wasm/client/stream.rs`
  `WasmSseIterator::next` maps `parse_next()`'s `Err(_)` to the same arm as
  `Ok(None)` (`Wait`, or end of stream), so a malformed event or a reader error
  silently stalls or truncates the stream. Surface it as
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

## 10. Workspace / process

- **Shared target dir clobbering.** When parallel worktrees share one `CARGO_TARGET_DIR`, crates such as foundation_core and foundation_macros build with the same hashes and overwrite each other's test binaries and proc-macro artifacts mid-run. That gives stale results and `E0460` doctest failures.
  - Give each worktree its own target dir, or `RUSTFLAGS=-Cmetadata=<id>`. Note that the latter rebuilds all dependencies.
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

- **`AgentSession` still runs on the sync storage traits.** `MessageApi`,
  `MemoryStore`/`KvMemoryStore`, `MemoryCoordinator`, `ContextProvider` and the
  memory/search/agent tools take `D: DocumentStore` / `K: KeyValueStore` and
  call them from inside the async session. For I/O stores that means
  `foundation_db::Blocking` waits inside a valtron task. It is also the only
  reason `MemoryStorage`, `MemoryJsonStore` and `MemoryDocumentStore` keep
  their sync impls: `examples/cf-agent-loop` runs `AgentSession` on Workers,
  where `Blocking` can't exist. Fix: move these to the `Async*` traits, then
  delete the in-memory sync impls and the sync storage traits
  (`foundation_db/src/core/storage_provider.rs`).

## 11. foundation_db and its sync callers

Follow-ups from making the I/O backends async-only (branch `db/async-stores`).
- **Sync storage callers still go through `Blocking`.** Each should move to the
  `Async*` traits, one crate at a time, before the sync traits are deleted:
  - `foundation_auth` server services/handlers, generic over sync
    `QueryStore`/`KeyValueStore` and run inside HTTP handler tasks;
  - `foundation_cedar` `KvPolicyStore`/`SqlPolicyStore`;
  - `foundation_vectors` `SqlVectorStore`/`LibsqlVectorStore`;
  - `foundation_cronjobs` `CronScheduler`;
  - `MigrationRunner::run` and `StorageCleanup` (keep only `run_async` and an
    async cleanup).
- **`StateStore` is still a sync trait.** `LibsqlStore`'s `StateStore::init`
  / `sync_remote` and `native::create_state_store` wait on libsql with
  `.block()`, and `D1Store`'s `StateStore` impl uses a blocking HTTP call
  (`execute_sql`). Fix: an async `StateStore` (the deployment crates are its
  callers).
- **`Default` for `TursoStorage` / `LibsqlStore` blocks.** `Default` is sync,
  so it waits on the async constructor with `.block()`, and panics without a
  valtron pool. It exists for `S: Default` callers such as
  `AgentSession::builder_for`. Fix: drop it once those callers construct
  stores asynchronously.
- **Unused dependencies on wasm.** `cargo check --target wasm32` for
  `--features wasm-bindgen-storage` warns that `async-stream` and
  `foundation_netio` are unused (both are used only natively). Fix: move them
  to target-specific dependencies.
- **No CI build covers several feature sets, and they had rotted:**
  - foundation_db `libsql` (two libsql tests had no `#[test]`; fixed);
  - foundation_nativeapis `vfs-native,vfs-d1,vfs-r2` (didn't compile; fixed);
  - foundation_vectors `sql-store`;
  - foundation_auth `server-test`;
  - `examples/cf-login-app` (its manifest couldn't load; fixed).

  Fix: add these runs to `ci/packages.toml` and an examples job.

## 12. foundation_auth

- **17 IdP HTTP integration tests fail, on master too.** `tests/integration/e2e`
  (8) and `tests/integration/idp_server` (9) get non-success responses
  (e.g. `introspect_returns_inactive_for_unknown_token`: `response.is_success()`
  is false). CI never runs them: the test target needs `server-test`, which
  `ci/packages.toml` doesn't enable. Fix: find why the handlers fail, then add a
  `server-test` run.
- **`--features server-native` tests don't link here.** `webauthn-rs` needs
  OpenSSL 3 symbols (`EVP_Q_digest`, `OPENSSL_sk_*`) that the build host
  doesn't provide. Fix: vendor OpenSSL (`openssl/vendored`) or document the
  system dependency.
- **`AuthManager` calls the sync `CredentialStore` from async fns.**
  `authenticate`, `refresh` and `logout` (`src/shared/auth_manager.rs`) block
  on storage inside async code, and `AuthManager` is compiled out on JS-hosted
  wasm because of it. Fix: move it to `AsyncCredentialStore`.
- **`NativeOAuth` keeps sync twins** (`exchange_code`, `client_credentials`,
  `refresh_token`) next to the `*_async` methods (`src/native/oauth.rs`),
  against the async-only API rule. Fix: keep only the async methods (as the
  wasm client now does) and let callers `.block()`.

## 13. foundation_nativeapis

- **`FjallDocumentStore` implements only the sync `DocumentStore`** although it
  does disk I/O (`src/shared/vfs/fjall_document_store.rs`). Fix: implement
  `AsyncDocumentStore` (the I/O can run in the async body, like
  `JsonFileStorage`) and let sync callers use `Blocking`.
- **The async D1/R2 VFS files block and ignore the write offset.**
  `AsyncD1File` / `AsyncR2File` (`src/native/vfs/{d1,r2}_delta/mod.rs`) call
  the sync delta methods, which now wait through `Blocking`, from inside async
  fns. Their `write_at_async` ignores `offset` (it rewrites the whole file from
  `data`), and their `mode` field is never read. Fix: async delta methods over
  the `Async*` stores; honour `offset` and read-only `mode`.

## Paused

- Windows/macOS VM images (#69).
