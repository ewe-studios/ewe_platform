---
feature: "The H2 client pump parks on reactor readiness instead of polling every 10 ms"
status: "pending (CI proof)"
lane: "priority follow-up (owner request after feature 121)"
model: "sonnet"
branch: "spec70/f125-h2-pump-readiness"
crates: ["foundation_connectrpc"]
catalogue_rows: []  # from docs/bug-backlog.md §1
---

# Feature 125: the H2 client pump parks on reactor readiness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **pump**: the task that owns one HTTP/2 client connection and moves frames between the
  socket and the pipes (`H2Pump`, `foundation_connectrpc/src/native/transport/h2.rs`).
- **readiness**: a signal that a socket can be read or written without blocking.
- **reactor**: the process-wide thread that waits for readiness on many sockets.
- **park**: to stop polling a task until a readiness signal wakes it.

## Problem (measured 2026-11-06, feature 121)

`H2Pump::next_status` never parks on a readiness signal. Every point where it must wait
returns `TaskStatus::Delayed(POLL_DELAY)` with `POLL_DELAY` = 10 ms
(`h2.rs:38`, and the six `Delayed` returns). The task therefore runs every 10 ms for the
whole life of the connection, even when the socket has no bytes and the caller has queued
no body chunk.

This costs work and, more importantly, it hides the task's real readiness from the
executor. A worker that runs a long task delays every Delayed task it hosts. Feature 121
measured a 1.5 s stall of an H2 pump with `polls=1`, and the second cause it found was a
past-due timer dropped from the wakeup calculation. Both are symptoms of a scheduler that
must guess when a task should run.

The reactor already gives exact readiness:
`foundation_reactor::FdReadiness` implements `EventReadiness` and wakes a registered
valtron task on every socket event. The pipe gives
`PipeReceiver::readiness()` → `QueueReadiness`, which is ready when a body chunk is queued.
`AnyReadiness` combines them and reports the earliest deadline.

## Design

### 1. Give the pump exact parking signals

`H2Pump` gains:

```rust
/// The socket's reactor registration. READABLE is always watched; WRITABLE is
/// added only while `pending_out` holds bytes, because an idle socket is always
/// writable and would spin the drain thread.
socket: Arc<FdReadiness>,
/// Ready when a request body chunk is queued in `send_rx`.
body_queued: EventReadinessPtr,
/// One poll counter for the regression test.
polls: &'static AtomicU64,
```

- Build `socket` with `FdReadiness::register(fd)` in `open`, after the dial and after
  `set_nonblocking(true)`. The pump already sets the socket non-blocking on every poll;
  do it once at construction instead.
- `body_queued` is `Arc::new(send_rx.readiness())`.
- On a short write (`write_out` returns `Ok(false)`), call `socket.set_writable(true)`.
  Once `pending_out` drains, call `socket.set_writable(false)`. Arm the write interest
  only at that point: arming it before every write latches a writable edge on an idle
  socket and makes the task re-poll once per turn.
- The park also includes a close-only readiness for the head and the body pipe. A
  cancelled call drops its response receivers; without this wake the pump stays parked,
  never sees `is_closed`, and never sends `RST_STREAM`. `QueueVacancyReadiness` reports
  "not full", which is true for an empty pipe, so a small adapter reports the close
  case only.
- Call `socket.clear()` once at the start of every turn, before the read loop. Clear
  *before* an attempt to read or write, never after it: a clear after a `WouldBlock` can
  erase an edge that arrived during the attempt, which loses the wake. A clear before the
  attempt can only cause a spurious wake, which is harmless because the pump reads again
  and gets `WouldBlock`. This follows the `FdReadiness` module docs and the in-repo user of
  the same protocol, the h2 server's `arm_wakers`
  (`foundation_http/src/native/server/h2_connection.rs`).

### 2. Return `Depends` instead of `Delayed`

Replace every `Some(TaskStatus::Delayed(POLL_DELAY))` with
`Some(TaskStatus::Depends(self.park()))`, where:

```rust
fn park(&self) -> EventReadinessPtr {
    let mut children = vec![
        Arc::clone(&self.socket) as EventReadinessPtr,
        Arc::clone(&self.body_queued),
    ];
    if let Some(deadline) = self.next_deadline() {
        children.push(Arc::new(DeadlineReadiness::at(deadline)));
    }
    Arc::new(AnyReadiness::new(children))
}
```

`next_deadline` returns:

- `handshake_deadline` while `state == Handshaking`;
- `last_progress + read_timeout` when `read_timeout` is non-zero and the head is out;
- `None` otherwise.

`DeadlineReadiness` is a small `EventReadiness` in the same module: `is_ready` is
`Instant::now() >= at`, `deadline` returns `Some(at)`, `register` returns `true` (it needs
no waker; the executor sleeps until `deadline`). Build it per park, because the idle
deadline moves with `last_progress`.

Keep `POLL_DELAY` only as the fallback when `FdReadiness::register` fails (for example on a
platform without a reactor); log the fallback once at `tracing::debug!`.

### 3. Keep the pump's own read discipline

The pump must still read until `WouldBlock` before it parks, and it must still pump the
request body before every write. `Depends` only replaces the "nothing to do, wait" return;
it must not replace a `continue` or a fall-through.

### 4. Bound the memory of the park signal

`AnyReadiness` allocates one `Arc` per child per park. A park happens at most once per
executor turn, and the pump already allocates frames, so this is small. Do not cache the
deadline child: its instant changes.

## Tests

Add `backends/foundation_connectrpc/tests/h2_pump_parking_tests.rs` (own binary, registered
in `Cargo.toml` with `required-features = ["rpc_multi"]`), and expose one
`#[doc(hidden)] pub fn h2_pump_poll_count() -> u64` from the transport module so a test can
count pump turns. Both tests use a real listener on `127.0.0.1:0`.

- `an_idle_h2_call_does_not_busy_poll`: the listener accepts and sends nothing. Open an
  `H2Transport` call with a 200 ms deadline, let the call fail, and assert the pump poll
  count grew by at most 5 across a 200 ms idle window. On the old code it grows by about
  20 (one per `POLL_DELAY`), so the test fails on the old code.
- `a_slow_response_body_arrives_without_a_poll`: the server sends the response head at
  once and the body 300 ms later. The call must return both, and the poll count must stay
  small, which proves the park wakes on the socket edge rather than on a timer.

Both tests must be deterministic: use a real local socket, a bounded wait, and no CPU
assumption.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
cargo test --profile uat -p foundation_proxy
cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc \
  --no-default-features --features rpc
```

Then the feature 121 loop in the Linux CI image with `--cpus=2`: 30 runs of
`rpc_proxy_e2e_tests` and 30 runs of `h2_integration_tests`.

## Done when

- [x] No `TaskStatus::Delayed` remains on the normal wait paths of `H2Pump`.
- [x] The pump registers write readiness only while output is pending.
- [x] `an_idle_h2_call_does_not_busy_poll` fails on the old code and passes with the park.
- [x] `foundation_connectrpc` and `foundation_proxy` tests pass, and the 30-run loop is
      green.
- [ ] CI green.

## Result

2026-11-06. Implemented on branch `spec70/priority-followups`.

What changed (`backends/foundation_connectrpc/src/native/transport/h2.rs`):

- `H2Pump` holds `socket: Option<Arc<FdReadiness>>`, `watching_writes`,
  `body_queued: EventReadinessPtr` and `polls: &'static AtomicU64`. `open()` registers
  the fd after it sets the socket non-blocking once (the per-turn `set_nonblocking` call
  is gone). When `FdReadiness::register` fails, the pump logs one `debug` line and keeps
  the `POLL_DELAY` fallback.
- Every normal wait now returns `Some(TaskStatus::Depends(self.park()))`. The park is an
  `AnyReadiness` over: the socket (READABLE always; WRITABLE only after a short write);
  the request-body queue while the request direction is open; a per-park
  `DeadlineReadiness` for the handshake deadline or `last_progress + read_timeout`; the
  response pipe's vacancy while `pending_body` is full; and a close-only `ReceiverGone`
  readiness for the head and body pipes.
- The pump reads until `WouldBlock` before it parks. Each "park" return is guarded by
  `if read_some { continue; }`, so bytes still in the kernel are read before the task
  sleeps and no latched edge is lost.
- `socket.clear()` runs once at the start of a turn, before the read loop. §1 above is
  corrected: a clear after `WouldBlock` can erase a real edge.
- `write_out` arms WRITABLE only when a write returns `WouldBlock`, and clears it when
  `pending_out` drains. Arming it before every write latched a writable edge on an idle
  socket and made every park "already ready".
- `TaskStatus::Delayed(POLL_DELAY)` remains only for a platform without the reactor and
  for a failed registration.

One gap in the plan: the park list had no wake for a dropped response receiver. A
cancelled call drops its receivers, so a parked pump never ran the `is_closed` checks and
never sent `RST_STREAM`. `client_cancel_resets_the_stream_on_the_server` failed 3/3 until
`ReceiverGone` was added. It wraps the pipe's vacancy notifier but reports ready only on
a close, because vacancy is ready for any empty pipe.

Tests (`tests/h2_pump_parking_tests.rs`, `required-features = ["rpc_multi"]`, unix only):

- `an_idle_h2_call_does_not_busy_poll`: a silent peer; the 200 ms handshake timeout ends
  the call. The pump ran 2 turns with the park. With `park()` reverted to
  `Delayed(POLL_DELAY)` (counter kept) it ran 18-19 turns and the test failed.
- `a_slow_response_body_arrives_without_a_poll`: a raw `H2Channel` server sends the head
  at once and the body after 300 ms. The pump ran 5 turns with the park and 29 with the
  old code. Both values arrived whole.

Commands and results:

| command | result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings` | clean |
| `cargo test --profile uat -p foundation_connectrpc` | 238 passed, 0 failed |
| `cargo test --profile uat -p foundation_proxy` | 73 passed, 1 failed: the known macOS `websocket_upgrade_relays_and_tunnels_bytes` (backlog §9) |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc` | passes; 3 pre-existing `dispatch.rs` warnings (backlog §1) |
| Linux CI image, 30 x `rpc_proxy_e2e_tests` + 30 x `h2_integration_tests` | `30_RUNS_GREEN`: all 60 runs pass (17 and 7 tests per run) |
| `scripts/ci/linux.sh foundation_connectrpc` (fmt, build, clippy, test, wasm in the Linux image) | exit 0; the new parking tests pass on epoll; only the 3 known wasm warnings |

Version: `foundation_connectrpc` 0.0.4 to 0.0.5 for the new `h2_pump_poll_count` test
hook; the `foundation_proxy` and `foundation_deployment_docker` requirements follow.

Status: `pending (CI proof)`.
