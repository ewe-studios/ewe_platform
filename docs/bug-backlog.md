# Bug and limitation backlog

This is the single list of known bugs and limitations that are still open.

CLAUDE.md rule 9 says every bug found gets tracked and scheduled. This file is where they are tracked. Each entry names the crate and file, says what is wrong, and gives the likely fix.

How to use it:
- When you find a new bug, add it here in the same change, even if you are not fixing it.
- When you fix one, delete its entry in the PR that fixes it and name the PR in the commit message.
- Group entries by crate. Mark limits that are still hard-coded constants with **(rule 8)**.

Last refreshed: 2026-10-20, after PR #96. In flight on branches:
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
- **Wasm `fetch` blocks the isolate via `block_on_collect`.** `wasm/client/client.rs`
  `send` and `send_sse` wrap `do_fetch` in `block_on_collect` (was `run_future`),
  which does a synchronous
  `ReadyValues::new(iter).collect()` that never yields to the JS event loop, so the
  `fetch` promise never resolves and the whole isolate blocks (spec 65 T6 hang).
  Fix in spec 65 feature `06-wasm-async-streaming`: make wasm `send_sse` lazy —
  spawn `do_fetch` via `spawn_local` on first poll and flatten a `WasmSseIterator`,
  instead of wrapping the fetch in `block_on_collect`; and make `send` yield or drop it
  from the wasm path.
- **Wasm `send_sse_async` returns a non-parking stream.** It boxes via
  `into_future_stream()` (`StreamAsFutureStream`, a raw lens whose `poll_next` is
  `Poll::Ready(iter.next())`), so `Wait`/`Pending` are forwarded as items and the
  stream busy-polls instead of parking. Fix in spec 65 feature
  `06-wasm-async-streaming`: rename the raw lens to `into_pure_future_stream()` and
  make `into_future_stream()` an await-friendly lens that parks on `Wait`/`Pending`
  (with `impl WakeableStream for WasmSseIterator` — a `WakeRegistry` fired by its
  `spawn_local` reader on each queue push). `send_sse_async` keeps the full-enum
  item type, so it stays on `into_future_stream()` (now parking) rather than
  switching to `into_next_stream()`.

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
- **`block_on_collect` (was `run_future`) is wasm-unsafe.**
  - `executors/non_sendables.rs` `block_on_collect` collects a `DrivenRecvIterator`
    synchronously (`filter_map(ReadyValue::inner).collect()`) with no JS yield, so
    it deadlocks on the single-threaded wasm executor. It is still used by the
    three wasm call sites (netio `send`/`send_sse`, ai `backend_utils::sleep`)
    until feature 06 T3/T4 replace them with lazy/async equivalents.
  - The wasm-native bridge is `spawn_local` + `ConcurrentQueue` + `Stream::Wait`,
    consumed by the parking `into_next_stream`/`into_future_stream`. Fix in spec 65
    feature `06-wasm-async-streaming`: make wasm `send_sse` lazy (no
    `block_on_collect`), fix the wasm `sleep` backoff, and add a parking test to
    guarantee `into_future_stream`/`into_next_stream` park on `Wait`/`Pending`.
  - (Fixed in T2) the single-threaded `ThreadedIterFuture`/`FutureIterator` path
    dropped the future after one `Pending`; it now retains the pinned future and
    re-polls it on each `next()` (`test_threaded_future_survives_pending`).
- **Release coupling.** `foundation_macros` calls `foundation_core::valtron::initialize_current_thread`, so the two crates need a coordinated version bump when published.

## 5. foundation_nostd / foundation_wasm / foundation_iogate

- **`SpinWaiter`** (foundation_nostd, still used on wasm) rounds sub-millisecond waits down to zero, and its spin calibration is arbitrary.
- **Unused import** at `foundation_wasm/src/capability.rs:24` on wasm32.
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

## Paused

- Windows/macOS VM images (#69).
