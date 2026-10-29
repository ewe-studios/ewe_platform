# Plan: Feature 08 — Async `Serve` for long-lived HTTP/1.1 handlers

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are._

**Abbreviations in this file.** Hypertext Transfer Protocol version 1.1 (HTTP/1.1);
remote procedure call (RPC); application programming interface (API); pull request (PR).

## 1. What this plan covers

This plan gives the exact types, the state changes, the reused wake bridge, and
every place that matches on `ConnectionResult`. The main session approves this plan
before any code. This is task T1.

## 2. The new types

### 2.1 `ServeFuture`

Add to `foundation_http/src/shared/serve/mod.rs`, next to `ConnectionResult`.

```rust
use std::future::Future;
use std::pin::Pin;

/// A handler's async continuation. It owns everything it needs ('static) and
/// resolves to what the connection does next.
pub type ServeFuture = Pin<Box<dyn Future<Output = ConnectionResult> + Send + 'static>>;
```

`ServeFuture` names `ConnectionResult`, and `ConnectionResult::Async` holds a
`ServeFuture`. The `Box` inside `ServeFuture` breaks the recursion. The type is sized.

### 2.2 `ConnectionResult::Async`

```rust
pub enum ConnectionResult {
    Keep,
    Take,
    Close(Option<ErrorTrace<ServeError>>),
    /// The handler continues asynchronously. The connection task parks on the
    /// future and applies its result (Keep / Take / Close) when it completes.
    Async(ServeFuture),
}
```

`Serve` stays synchronous. Every existing handler compiles unchanged. A handler
that waits returns `Async(fut)` at once.

### 2.3 Where the new types live

`ServeFuture` and the `Async` variant go in `shared/serve/mod.rs`. This file is
compiled on both native and wasm. `Pin<Box<dyn Future + Send + 'static>>` compiles
on both targets. No `cfg` gate is needed.

## 3. The reused wake bridge

The bridge is the one `FutureTask` uses (spec 50 feature 01):

- `foundation_core::valtron::queue_waker(queue, id) -> Waker`. The waker pushes a
  `WakeToken` onto a `ConcurrentQueue` and calls `request_executor_resume`.
- `foundation_core::valtron::QueueReadiness<T>`. It is ready when the queue is not
  empty.
- `foundation_core::valtron::WakeToken`. The token the waker pushes.

The `ConcurrentQueue` type comes from the `concurrent-queue` crate. `foundation_http`
does not name it today. Add one dependency:

```toml
concurrent-queue = { workspace = true }
```

The workspace already pins `concurrent-queue = "2.5.0"` (the version `foundation_core`
uses).

### 3.1 The shutdown signal is composed in

A parked `Awaiting` task must wake on shutdown. `QueueReadiness` alone does not watch
the shutdown signal. Add one small type that ORs the two. It mirrors
`CancellableFutureTask`'s `CancelOrReadiness` (foundation_core, same file as
`FutureTask`), which ORs an inner readiness with a cancel flag. This is composition of
the existing bridge, not a new bridge.

```rust
struct AsyncReadiness {
    future: foundation_core::valtron::QueueReadiness<foundation_core::valtron::WakeToken>,
    shutdown: std::sync::Arc<foundation_core::synca::OnSignal>,
}

impl foundation_core::valtron::EventReadiness for AsyncReadiness {
    fn is_ready(&self, dur: Option<std::time::Duration>) -> bool {
        self.shutdown.probe() || self.future.is_ready(dur)
    }
}
```

The executor polls a readiness-parked task with `is_ready(Some(10 ms))` on its
~1 ms readiness-poll quantum. So a passive `OnSignal` is seen within about 1 ms, well
inside the 100 ms test bound. The future's own waker still wakes the executor at once
via `request_executor_resume`.

## 4. The connection handler state changes

File: `foundation_http/src/native/server/connection.rs`.

### 4.1 New state variant

```rust
enum HandlerState {
    Idle,
    CheckExpect { req: SimpleIncomingRequest, should_close: bool, continue_retries: usize },
    WaitingForBody { req: SimpleIncomingRequest, should_close: bool, attempt: usize },
    Processing { req: SimpleIncomingRequest, should_close: bool },
    /// A handler returned `ConnectionResult::Async`. The future is polled once per
    /// `next_status` and parked on the bridge until it resolves or the server drains.
    Awaiting { future: ServeFuture, should_close: bool },
}
```

### 4.2 New field on `ConnectionHandler`

```rust
wake_queue: std::sync::Arc<concurrent_queue::ConcurrentQueue<foundation_core::valtron::WakeToken>>,
```

Built once in `new()` with `ConcurrentQueue::unbounded()`. It is drained before each
poll. One connection has at most one `Awaiting` at a time, so one queue is enough.

### 4.3 The `Awaiting` handler

```rust
fn handle_awaiting(
    &mut self,
    mut future: ServeFuture,
    should_close: bool,
) -> Option<TaskStatus<(), ()>> {
    // Shutdown first: drop the future and close. Dropping `future` here runs the
    // handler's drop flags (the shutdown test checks this).
    if self.shutdown.probe() {
        tracing::trace!(client_ip = %self.client_ip, "Awaiting: server draining, dropping the handler future");
        return None;
    }

    // Drain stale tokens before polling (the FutureTask pattern).
    while self.wake_queue.pop().is_ok() {}

    let waker = foundation_core::valtron::queue_waker(self.wake_queue.clone(), 0);
    let mut cx = std::task::Context::from_waker(&waker);
    match future.as_mut().poll(&mut cx) {
        std::task::Poll::Ready(result) => self.apply_async_result(result, should_close),
        std::task::Poll::Pending => {
            self.state = Some(HandlerState::Awaiting { future, should_close });
            if self.wake_queue.is_empty() {
                Some(TaskStatus::Depends(std::sync::Arc::new(AsyncReadiness {
                    future: foundation_core::valtron::QueueReadiness::new(self.wake_queue.clone()),
                    shutdown: self.shutdown.clone(),
                })))
            } else {
                // A self-wake during poll: requeue without parking.
                Some(TaskStatus::Ignore)
            }
        }
    }
}
```

### 4.4 Applying the result

```rust
fn apply_async_result(
    &mut self,
    result: ConnectionResult,
    should_close: bool,
) -> Option<TaskStatus<(), ()>> {
    match result {
        ConnectionResult::Keep => {
            if should_close {
                tracing::trace!(client_ip = %self.client_ip, "Awaiting: handler kept connection but Connection: close; closing");
                return None;
            }
            tracing::trace!(client_ip = %self.client_ip, "Awaiting: handler kept connection, returning to Idle");
            self.state = Some(HandlerState::Idle);
            Some(TaskStatus::Pending(()))
        }
        ConnectionResult::Take => {
            tracing::trace!(client_ip = %self.client_ip, "Awaiting: handler took connection");
            None
        }
        ConnectionResult::Close(err) => {
            if let Some(e) = &err {
                tracing::error!(client_ip = %self.client_ip, err = ?e, "Connection closed with error");
            }
            None
        }
        ConnectionResult::Async(_) => {
            // An Async whose future resolves to Async again is a programming error.
            tracing::error!(client_ip = %self.client_ip, "Awaiting: a handler returned Async from an Async future; closing");
            None
        }
    }
}
```

### 4.5 The `Processing` match gains an arm

In `handle_processing`, after `let result = handler.serve(bag, req, self.conn.clone());`:

```rust
ConnectionResult::Async(future) => {
    tracing::trace!(client_ip = %self.client_ip, "Processing: handler returned Async; parking on its future");
    self.state = Some(HandlerState::Awaiting { future, should_close });
    Some(TaskStatus::Pending(()))
}
```

### 4.6 The `next_status` dispatch

```rust
HandlerState::Awaiting { future, should_close } => self.handle_awaiting(future, should_close),
```

## 5. Every place that matches on `ConnectionResult`

`ConnectionResult` is an enum. Adding a variant breaks only **exhaustive** matches.
There are exactly two. Both get a new `Async` arm.

1. `foundation_http/src/native/server/connection.rs` (the `match result` in
   `handle_processing`). Add the `Async` arm from §4.5.
2. `foundation_http/src/wasm/serve_writer.rs` (`handle_request_with_bag`, the
   `match result`). The wasm `ServeWriter` path is synchronous and cannot drive a
   future. Treat `Async` as an error:

```rust
ConnectionResult::Async(_) => Err(ErrorTrace::new(ServeError::InternalError {
    status: 500,
    reason: "a handler returned ConnectionResult::Async, which the synchronous wasm path cannot drive".into(),
})),
```

No other file exhaustively matches. `foundation_proxy`, `foundation_browser`,
`foundation_auth`, and `foundation_toolings` only construct `Keep`/`Take`/`Close`
(they never return `Async`). Their `.map_or(...)` and early returns keep compiling.

## 6. Connect RPC uses it (foundation_connectrpc)

File: `foundation_connectrpc/src/native/server.rs` and
`foundation_connectrpc/src/native/conn_io.rs`.

### 6.1 Own the driver

`ConnIo` is currently created per `serve` call and borrowed by `H1Body`/`H1Sink`.
The async future must own everything, so:

- Wrap it: `let io = Arc::new(ConnIo::new(&conn)?)`.
- `H1Body` owns `Arc<ConnIo>` and `Arc<AtomicBool>` (the `drained` flag) instead of
  `&ConnIo` and `&AtomicBool`.
- `H1Sink` owns a cloned `SharedByteBufferStream<RawStream>` and `Arc<ConnIo>`
  instead of `&mut conn` and `&ConnIo`.

### 6.2 Make the wait futures own the driver

`IoReady`, `Sleep`, and `Hangup` today hold `&'a ConnIo`. Change all three to hold
`Arc<ConnIo>`. `H1Body` stores an `IoReady` across polls, so this is required.

Change the constructors to take `self: &Arc<Self>`:

```rust
pub(crate) fn ready(self: &Arc<Self>, interest: IoInterest, timeout: Duration) -> IoReady;
pub(crate) fn sleep_until(self: &Arc<Self>, deadline: Instant) -> Sleep;
pub(crate) fn hangup(self: &Arc<Self>) -> Hangup;
```

Each builds its future with `io: self.clone()`.

### 6.3 The deadline timer

Replace `impl DeadlineTimer for ConnIo` with `impl DeadlineTimer for Arc<ConnIo>`. Its
`sleep_until` returns `Sleep { io: self.clone(), deadline }`. `serve` passes
`hooks.with_timer(&io)`, where `io: Arc<ConnIo>`, so the timer is `'static`.

### 6.4 Remove the blocking driver

`ConnIo::block_on` is used only by `ConnectRpcServe::serve`. After the change it is
dead. Remove both the unix and the non-unix `block_on`, and the now-unused
`impl Wake for WakeTx`. `WakeTx::notify` stays (the overlay waker still calls it).

### 6.5 The new `serve`

```rust
impl Serve for ConnectRpcServe {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        mut req: SimpleIncomingRequest,
        conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let io = match ConnIo::new(&conn) {
            Ok(io) => Arc::new(io),
            Err(e) => {
                tracing::error!(error = %e, "could not set up the connection driver; closing");
                return ConnectionResult::Close(None);
            }
        };
        let body = req.body.take();
        let drained = Arc::new(AtomicBool::new(matches!(body, None | Some(SendSafeBody::None))));
        let cancel = CancelSignal::new();
        let handler = self.handler.clone();
        let timeouts = self.timeouts;

        let fut = async move {
            let body: RequestBody<'static> = Box::pin(H1Body {
                reader: BodyReader::new(body),
                io: io.clone(),
                timeout: timeouts.body_read,
                wait: None,
                drained: drained.clone(),
            });
            let mut sink = H1Sink {
                conn,
                io: io.clone(),
                timeout: timeouts.write,
            };
            let hooks = CallHooks::new(cancel.clone()).with_timer(&io);
            let dispatch = handler.dispatch_to(bag, req, body, &mut sink, hooks);
            let hangup = async {
                match io.hangup().await {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::debug!(error = %e, "cannot watch the connection for a hangup");
                        std::future::pending::<()>().await;
                    }
                }
            };
            match futures::future::select(dispatch, hangup).await {
                futures::future::Either::Left((outcome, _)) => match outcome {
                    Ok(()) if drained.load(Ordering::Acquire) => ConnectionResult::Keep,
                    Ok(()) => {
                        tracing::debug!("the request body was not read to its end; closing the connection");
                        ConnectionResult::Close(None)
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "writing the response failed; closing the connection");
                        ConnectionResult::Close(None)
                    }
                },
                futures::future::Either::Right(((), _)) => {
                    tracing::debug!("the client closed the connection; cancelling the call");
                    cancel.cancel();
                    ConnectionResult::Close(None)
                }
            }
        };

        ConnectionResult::Async(Box::pin(fut))
    }
}
```

Notes:

- The `async move` block owns `io`, `conn`, `bag`, `req`, `body`, `drained`,
  `cancel`, `handler`, and `timeouts`. All are `Send + 'static`. The `dispatch`
  future borrows `&mut sink` and `&io`; the `hangup` future borrows `&io`. Those
  borrows stay inside the block. The block is `Send + 'static`.
- `dispatch` is `Pin<Box<dyn Future + Send>>` and `hangup` is `Unpin`; `select`
  polls both by `&mut`. `pin!` where needed.
- Keep-alive, body-drain, and hangup rules are unchanged from today. They move from
  `io.block_on(select(...))` into `.await`.

## 7. Out of scope

`foundation_proxy` relays on a dedicated thread and returns `Take`. It stays as it is.
Add a backlog line: it can move to `Async` later. WebSocket upgrades keep `Take`.

## 8. Regression tests

### 8.1 `foundation_http/tests/async_serve_tests.rs` (new)

- `async_result_keep_serves_next_request`: a handler returns `Async(fut)` resolving to
  `Keep`. A second request on the same connection is served.
- `async_result_shutdown_drops_future`: shutdown during a pending `Async`. Assert the
  future is dropped (a drop flag) and the task ends within 100 ms.
- `nested_async_closes_with_error`: the future resolves to `Async` again. Assert the
  connection closes.

### 8.2 `foundation_connectrpc/tests/h1_pool_parking_tests.rs` (new)

- `h1_streaming_calls_do_not_exhaust_the_pool`: a server with a 2-worker valtron pool.
  Open 8 HTTP/1.1 server-streaming RPCs. Each waits 1 s between messages. While they
  are open, a unary call completes in under 100 ms. On the old code the unary call
  waits until a stream ends or times out. Build the 2-worker pool with
  `#[valtron_test(threads = 2)]` (or the matching `initialize_pool` form used by
  `server_socket_tests.rs`).

## 9. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_http foundation_connectrpc foundation_core foundation_proxy foundation_browser; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new test and for foundation_connectrpc conformance_tests.
rg -n 'block_on' backends/foundation_connectrpc/src/native/server.rs   # expect no output
```

Run the valtron tests with `--profile uat` (LLVM), not the default dev profile, per the
project rule.

## 10. Done when

- This plan is approved by the main session before any code.
- All §9 commands pass; the new tests pass 20 runs in a row.
- `h1_streaming_calls_do_not_exhaust_the_pool` fails on the base code and passes now.
- No `block_on` in `connectrpc/src/native/server.rs`.
- CI green.
