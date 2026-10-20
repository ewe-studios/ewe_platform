# Bug and limitation backlog

This is the single list of known bugs and limitations that are still open.

CLAUDE.md rule 9 says every bug found gets tracked and scheduled. This file is where they are tracked. Each entry names the crate and file, says what is wrong, and gives the likely fix.

How to use it:
- When you find a new bug, add it here in the same change, even if you are not fixing it.
- When you fix one, delete its entry in the PR that fixes it and name the PR in the commit message.
- Group entries by crate. Mark limits that are still hard-coded constants with **(rule 8)**.

Last refreshed: 2026-10-20, after PR #96 (spec 65 feature 06 T5 updates). In flight on branches:
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

## 5. foundation_nostd / foundation_wasm / foundation_iogate

- **The owned wasm path imports wasm-bindgen glue.** `foundation_compact`
  (`Cargo.toml`: `wasm-bindgen`/`js-sys` unconditionally on
  `wasm32-unknown-unknown`; `src/wasm/{instant,system_time}.rs`,
  `src/entropy/backends/wasm_js.rs`) makes every foundation_core wasm build import
  `__wbindgen_placeholder__`, so a `js-foundation-wasm` module (the zero
  wasm-bindgen path) cannot be instantiated by `foundation-wasm.js`: all of
  `foundation_core/integration/wasm_tests` fails at load (spec 68). Give
  compact a feature-selected backend that reads time/entropy through host ABI
  imports implemented by `foundation-wasm.js`, and use it under `js-foundation-wasm`.

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

## 9. Workspace / process

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

## 10. foundation_ai

- **Non-streaming wasm `generate` is unsupported.** The providers' sync
  `generate` path calls `HttpClient::send`, which on wasm now returns an error
  (spec 65 feature 06 T5: a sync send cannot wait for `fetch` without freezing the
  isolate), and its retry backoff `backend_utils::sleep` is a no-op on wasm (T4).
  So one-shot generation fails fast on wasm instead of deadlocking, but does not
  work. Likely fix: an async `generate` seam (e.g. `generate_async`) over
  `send_async`, or route wasm `generate` through the lazy `send_sse`/`stream`
  machinery and collect its result async.
## Paused

- Windows/macOS VM images (#69).
