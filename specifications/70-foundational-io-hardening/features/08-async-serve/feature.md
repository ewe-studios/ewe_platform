---
feature: "Async Serve: long-lived HTTP/1.1 handlers stop holding a worker"
status: "pending"
priority: "high"
depends_on: ["05-push-body-and-accept"]
branch: "fix/io-08-async-serve"
crates: ["foundation_http", "foundation_connectrpc", "foundation_core"]
backlog: "docs/bug-backlog.md §1 HTTP/1.1 RPC holds a worker for its whole life"
review_gate: "two-step: the agent writes the plan (§5 T1) and stops; the main session approves it before any code"
---

# Feature 08: Async `Serve` for long-lived HTTP/1.1 handlers

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol version 1.1 (HTTP/1.1); remote
procedure call (RPC); Server-Sent Events (SSE); application programming interface (API); pull
request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## 1. Goal

An HTTP/1.1 handler that waits a long time (a server-streaming RPC, SSE, a long poll) does not
hold a valtron worker while it waits.

**Now.** `foundation_http/src/shared/serve/mod.rs:72`:

```rust
pub trait Serve: Send + Sync + 'static {
    fn serve(&self, bag: Arc<ContextBag>, req: SimpleIncomingRequest,
             conn: SharedByteBufferStream<RawStream>) -> ConnectionResult;
}
pub enum ConnectionResult { Keep, Take, Close(Option<ErrorTrace<ServeError>>) }
```

`serve` is synchronous. The connection task
(`foundation_http/src/native/server/connection.rs`, around line 762) calls it and waits for the
result. `foundation_connectrpc/src/native/server.rs` `ConnectRpcServe::serve` (around line 129)
then runs the whole call with `io.block_on(select(dispatch, hangup))`, so the worker is busy for
the life of the RPC. Enough open streams use up the pool.

## 2. Design (approved by the main session; follow it)

### 2.1 A fourth result: an async continuation

```rust
/// A handler's async continuation. It owns everything it needs ('static) and
/// resolves to what the connection does next.
pub type ServeFuture = Pin<Box<dyn Future<Output = ConnectionResult> + Send + 'static>>;

pub enum ConnectionResult {
    Keep,
    Take,
    Close(Option<ErrorTrace<ServeError>>),
    /// The handler continues asynchronously. The connection task parks on the
    /// future and applies its result (Keep / Take / Close) when it completes.
    Async(ServeFuture),
}
```

- `Serve` stays synchronous, so every existing handler compiles unchanged. A handler that waits
  returns `Async(fut)` at once.
- An `Async` whose future resolves to `Async` again is a programming error: log at `error`, close.

### 2.2 The connection task drives the future

- The connection handler (`connection.rs`) is a valtron `TaskIterator` with `HandlerState`.
  Add `HandlerState::Awaiting(ServeFuture)`.
- In `Awaiting`, each `next_status` polls the future once with a waker that sets a readiness
  signal, and returns `TaskStatus::Depends(readiness)` on `Pending`. Use valtron's existing
  waker → readiness bridge (spec 50 feature 01, the same machinery `FutureTask` uses). Do not
  write a new bridge if one exists; find it first and name it in the plan.
- On `Ready(Keep)` go to `Idle` (unless the connection must close); `Ready(Take)` ends the task;
  `Ready(Close(e))` logs and ends.
- Shutdown: a parked `Awaiting` task must see shutdown (drop the future, close) within one wake.

### 2.3 Connect RPC uses it

- `ConnectRpcServe::serve` builds a `'static` future: it moves `conn` (the shared stream clone),
  an `Arc<ConnIo>`, the body reader, the request, `bag` and the handler `Arc` into the future.
  `H1Body` and `H1Sink` today borrow `&io` and `&mut conn`; change them to own `Arc`s or clones.
- The future runs `select(dispatch, hangup)` with `.await`, not `io.block_on`.
- It returns `ConnectionResult::Async(Box::pin(fut))`. The future resolves to
  `Keep` / `Close` with the same rules as today (keep only if the body was read to its end).

### 2.4 The socket wait is waker-capable

The original plan under-specified how `H1Body` and `H1Sink` wait for the socket
inside the `'static` future. The implementation rebuilds `ConnIo` (`conn_io.rs`)
on the readiness seams already present:

- `ConnIo` owns a `ConnReadiness` (Feature 06) and, for a kernel-fd transport, an
  `FdReadiness` (foundation_reactor). It has no `block_on` and no `DeadlineTimer`.
- `IoReady` waits by registering a `std::task::Waker` with the shared reactor
  (`FdReadiness::register_waker`), plus a valtron `timer::Sleep` for the timeout.
  Kernel fds park on the reactor; the WireGuard overlay parks on a one-shot
  `ConnWaker`; rustls corrects the watched direction (`wants_write`/handshaking).
- `Hangup` reads the reactor's latched `Ready` flags (`READ_CLOSED`/`WRITE_CLOSED`/
  `ERROR`), so it does not mistake a pipelined request for a close.
- `dispatch_to` takes `Box<dyn ResponseSink>` (owned), so the returned future is
  `'static`. The deadline timer is the existing `ThreadTimer`
  (`native/deadline_timer.rs`) held as a `static`, passed with `with_timer`.

### 2.5 Out of this design

`foundation_proxy` relays on a dedicated thread and returns `Take`. It stays as it is here; add
a backlog line that it can move to `Async` later. WebSocket upgrades keep `Take`.

## 3. Correctness rules

1. A handler that returns `Keep`/`Take`/`Close` behaves exactly as before (no change).
2. The future is polled only by its connection task; the waker only marks readiness.
3. Keep-alive: after `Async → Keep`, the next request on the connection is served.
4. Cancellation: a client that hangs up during an `Async` call cancels the call, as today.

## 4. Verification (run exactly)

```bash
for c in foundation_http foundation_connectrpc foundation_core foundation_proxy foundation_browser; do
  python3 scripts/ci/ci.py run $c --profile uat || exit 1
done
cargo check --workspace --all-targets --profile uat
# 20-run flake check for each new test and for foundation_connectrpc conformance_tests.
rg -n 'block_on' backends/foundation_connectrpc/src/native/server.rs   # expect no output
```

The `uat` profile is the LLVM build, not the default `dev` build (Cranelift has no panic
unwinding and cannot assemble the `candle` gemm-f16 FP16 SIMD code).

## 5. Tasks

- [x] T1. **Plan first.** Read `serve/mod.rs`, `connection.rs` (the whole `HandlerState` machine),
      `connectrpc/src/native/server.rs`, `conn_io.rs`, and the valtron waker → readiness bridge.
      Write a plan (a comment in a draft PR, or a file `features/08-async-serve/plan.md`): the
      exact types, the state changes, which bridge type you reuse, and every place that matches
      on `ConnectionResult` (it is an enum; adding a variant breaks exhaustive matches — list
      them: `connection.rs`, `wasm/serve_writer.rs`, tests, `foundation_browser`). **Stop and
      report.** The main session approves or changes the plan.
- [x] T2. Add `ConnectionResult::Async` and `ServeFuture`; update every match (wasm: treat
      `Async` as an error with a clear message, unless the wasm path can drive it — say which).
- [x] T3. `HandlerState::Awaiting` in `connection.rs` (§2.2).
- [x] T4. `ConnectRpcServe` returns `Async` (§2.3). Remove `io.block_on`.
- [x] T5. Regression tests:
  - `foundation_connectrpc/tests/…` `h1_streaming_calls_do_not_exhaust_the_pool`: a server with a
    2-worker pool; open 8 HTTP/1.1 server-streaming RPCs that each wait 1 s between messages;
    while they are open, a unary call completes in under 100 ms. On the old code the unary call
    waits until a stream ends (or times out).
  - `foundation_http/tests/…` `async_result_keep_serves_next_request`: a handler returns
    `Async(fut)` resolving to `Keep`; a second request on the same connection is served.
  - `foundation_http/tests/…` `async_result_shutdown_drops_future`: shutdown during a pending
    `Async`; assert the future is dropped (a drop flag) and the task ends within 100 ms.
  - `foundation_http/tests/…` `nested_async_closes_with_error`.
  - Existing hangup/cancellation tests still pass.
- [x] T6. Delete the backlog entry; add the proxy follow-up line. Update `progress.md`,
      `learnings.md`.

## 6. Done when

- [x] Plan approved by the main session before code.
- [x] All §4 commands pass; new tests passed 20 runs in a row.
- [x] `h1_streaming_calls_do_not_exhaust_the_pool` fails on the base code and passes now.
- [x] No `block_on` in `connectrpc/src/native/server.rs`.
- [x] CI green.

## 7. Review checklist (main session)

- [x] Every `ConnectionResult` match handles `Async`.
- [x] The future is `'static` without leaking the connection (drop closes it).
- [x] The waker bridge is the existing valtron one.
- [x] Keep-alive and cancellation behaviour unchanged.
