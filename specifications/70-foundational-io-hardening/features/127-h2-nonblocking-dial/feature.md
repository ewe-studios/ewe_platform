---
feature: "The H2 client dials without blocking a valtron worker"
status: "done"
lane: "priority follow-up (found by feature 121)"
model: "sonnet"
branch: "spec70/f127-h2-nonblocking-dial"
crates: ["foundation_connectrpc"]
catalogue_rows: ["1"]
---

# Feature 127: the H2 client dials without blocking a valtron worker

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **dial**: to open a TCP connection to a peer.
- **worker**: a thread of the valtron pool that runs many tasks.
- **park**: to stop polling a task until a readiness signal wakes it.
- **readiness**: a signal that a socket can be read or written without blocking.

## Problem (found by feature 121 on 2026-11-06)

`H2Transport::open` (`foundation_connectrpc/src/native/transport/h2.rs`) runs on a valtron
worker and calls `TcpStream::connect_timeout(addr, self.connect_timeout)`. That call
**blocks the worker** for as long as the peer takes to answer, up to the connect budget
(default 10 s). Every task the worker hosts waits too, including the pump of another call
and the response-body relay of a proxy request.

This is the same defect class that feature 121 removed from the response-body read, and
the file already carries the neighbouring lesson: the comment above `H2Channel::new`
records that waiting for the peer's SETTINGS on this thread deadlocked the pool whenever
every worker was blocked on a peer that needed a worker to answer. The dial is the same
wait, one step earlier.

The Unix path (`UnixStream::connect`) blocks in the same way.

`docs/bug-backlog.md` §1 holds this entry.

## Design

The dial moves into the pump. `open` starts a non-blocking connect and returns at once;
the pump's handshake phase decides when the connection is complete, failed or out of time.
This matches the handshake decision already recorded in the file.

### 1. `open` starts the connect without waiting

- Parse the address. Keep the current `TransportError::Connect` for a bad address, since
  no I/O is involved and the caller expects an immediate error.
- Create the socket with `socket2` (or `libc::socket` behind the crate's existing
  platform module): `SOCK_STREAM`, then `set_nonblocking(true)`, then `connect`.
- `connect` returns `Ok(())` (a loopback peer that accepts at once), `EINPROGRESS`
  (normal) or an immediate error. Only a non-`EINPROGRESS` error returns
  `Err(TransportError::Connect)` from `open`.
- The unix path does the same with `UnixStream`: `UnixStream::connect` on a non-blocking
  socket is not available on every platform, so keep the current blocking call for the
  unix path and record the limitation in the Result and in the backlog.
- Register the fd with the reactor **before** the pump's first turn, arm WRITABLE while
  the connect is in flight, and clear it once the connection resolves.

### 2. The pump's first turn resolves the connect

Add a `PumpPhase::Connecting` before `Handshaking`. In it:

- `socket.take_error()` (or `SO_ERROR` through the platform module) returns the pending
  error. `None` plus a successful `peer_addr()` means connected: move to `Handshaking`,
  clear WRITABLE.
- An error means the dial failed: publish `TransportError::Connect` on the head pipe,
  close the body and trailer pipes, return `None`.
- Neither means still in flight: park with `Depends` over the socket's WRITABLE readiness
  and a `DeadlineReadiness` at `connect_started + connect_timeout`.

The deadline needs its own value. `handshake_deadline` currently starts at `open`; give
`connecting_deadline` the same start instant so the whole setup keeps one budget, or keep
two budgets and document the choice. Prefer one budget: the call's setup deadline is
`open` plus `connect_timeout` plus the handshake budget, computed once.

### 3. The failure now arrives on the head pipe

Today a failed dial is `Err` from `open`. After this change `open` succeeds and the head
pipe carries `Err(TransportError::Connect)`. Audit every caller before the change:

- `grep -rn 'H2Transport' --include='*.rs' backends` for the construction sites;
- the shared `Client` maps an `open` error and a head error to different paths — check that
  both end in the same `ConnectError` for the caller;
- `foundation_proxy`'s upstream leg and the e2e tests that dial a closed port.

If a caller or a test depends on `open` returning `Err`, keep that shape by exposing
`H2Transport::open` unchanged and adding `open_nonblocking` for the streaming path, or
update the caller. Record the choice in the Result.

## Tests

Add `backends/foundation_connectrpc/tests/h2_dial_tests.rs` (own binary,
`required-features = ["rpc_multi"]`, unix only):

- `a_closed_port_fails_on_the_head_pipe`: bind a listener, note the port, drop the
  listener, then `open`. The call's head yields `Err(TransportError::Connect)` within
  2 s. On the old code the head yields the same error but `open` itself blocks; the test
  therefore also asserts that `open` returned in under 50 ms, which fails on the old code
  against a blackhole address.
- `a_blackholed_dial_does_not_block_the_worker`: use `10.255.255.1:9` (a non-routable
  address that does not answer) with a 300 ms `connect_timeout`. Measure two things: the
  time `open` takes (under 50 ms) and that a second, unrelated valtron task runs to
  completion while the dial is in flight. On the old code the second task does not run
  until the connect times out. This is the regression the feature exists for.
- `a_unix_socket_dial_still_connects`: guard the unix path against regression.

Both tests are bounded and use no CPU assumption. A blackhole address is environment
dependent: skip the test body and report a skip when the address is rejected at once
(`EACCES` or `ENETUNREACH` on the first `connect`).

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
cargo test --profile uat -p foundation_proxy
cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc \
  --no-default-features --features rpc
```

Then 30 runs of `rpc_proxy_e2e_tests` in the Linux CI image with `--cpus=2`.

## Done when

- [x] `open` does not block a worker on the TCP path.
- [x] A dial failure still reaches the caller as a connect error, on the same path.
- [x] `a_blackholed_dial_does_not_block_the_worker` fails on the old code and passes.
- [x] The unix path is unchanged or its limitation is recorded in the backlog.
- [x] CI green. Run `42261737548` (head `cf5ab359a`) passed every job.
      Commits after that head change specification text only.

## Result

2026-11-06. Implemented on branch `spec70/priority-followups`.

What changed (`backends/foundation_connectrpc/src/native/transport/h2.rs`):

- `H2Socket` gains a `Connecting(Option<socket2::Socket>)` variant. `connect_tcp` creates
  a non-blocking `socket2` socket and starts `connect(2)`. `Ok(())` means a loopback peer
  accepted at once; `EINPROGRESS` or `WouldBlock` keeps the `Connecting` state; any other
  error returns `TransportError::Connect` from `open` at once. `socket2` 0.6 is added to
  the crate's native (non-wasm) dependencies; the crate does not call `libc::socket`.
- `H2Transport::open` calls the private `start_dial`. It parses the address, starts the
  dial, then registers the fd with the reactor **before** the pump's first turn. While
  the connect is in flight it arms WRITABLE and records `watching_writes = true`.
- `PumpPhase::Connecting` comes before `Handshaking`. The pump checks `SO_ERROR` through
  `connect_state` before any read. `None` plus a successful `peer_addr` means connected:
  the pump replaces the variant with `Tcp`, disarms WRITABLE, and moves to `Handshaking`.
  A `Some` error publishes `TransportError::Connect` on the head pipe and closes the body
  and trailer pipes. Neither means still in flight: the pump parks on the socket's
  WRITABLE readiness and `connecting_deadline`. If the caller drops the head pipe while
  the dial is in flight, the pump stops at once.
- A connect timeout reports `io::Error::from(ErrorKind::TimedOut)`, so the caller sees the
  same `ConnectError` text that the old blocking `connect_timeout` produced.

Entry point: `H2Transport::open` keeps its `Transport` signature and behaviour for the
caller. No `open_nonblocking` was added, so the crate's public API does not change and the
version stays 0.0.5. The dial failure now rides the head pipe.

Callers audited:

- `src/shared/client.rs`: `do_unary_round_trip`, `server_stream_inner`,
  `client_stream_inner` and `bidi_stream_inner` each map `open` and the head through
  `ConnectError::from(TransportError)`. Both paths produce `Code::Unavailable` with
  `transport connect failure: ...`, so the caller sees the same error class.
- `foundation_proxy/src/h2_proxy.rs::response_relay`: the old open error sent
  `502`; a head error now takes the same `Some(Err(e))` branch and sends `502`.
- `foundation_proxy/src/rpc_relay.rs::connect_unary_to_grpc` and
  `foundation_proxy/src/health.rs`: the `GrpcClient` exchange surfaces a head error as the
  receive error, which both callers already map to an upstream failure string.
- `foundation_deployment_docker` (buildkit) uses the shared `Client`, so it inherits the
  same mapping.
- No test in `foundation_connectrpc` or `foundation_proxy` expected an `open` error for a
  closed or blackholed port. The new tests are the only ones that dial such an address.

Setup budget: two budgets, each starting with its own phase. The connect budget starts at
`open` and ends at `opened + connect_timeout`. The handshake budget starts when the
connect resolves and ends at `connected_at + handshake_timeout`. This is the correct
choice: each documented limit bounds its own step, a slow dial cannot spend the SETTINGS
budget, and the total setup is bounded by `connect_timeout + handshake_timeout`. One
shared deadline taken at `open + connect_timeout + handshake_timeout` would let a silent
peer hold the handshake twice as long, and it would break
`h2_pump_parking_tests::an_idle_h2_call_does_not_busy_poll`, whose 200 ms handshake
timeout must fire inside the test's 5 s bound.

Tests (`tests/h2_dial_tests.rs`, `required-features = ["rpc_multi"]`, unix only):

- `a_closed_port_fails_on_the_head_pipe`: a bound listener is dropped, then `open`. `open`
  returns in 0.32 ms and the head yields `Err(TransportError::Connect(ConnectionRefused))`.
  On the old code `open` returns `Err(Connect(ConnectionRefused))` after 0.01 ms and the
  test fails at the `open` expect. Observed with the blocking dial restored: panic
  `open starts the dial; a refused dial rides the head: Connect(Os { code: 61, kind:
  ConnectionRefused, ... })`.
- `a_blackholed_dial_does_not_block_the_worker`: `10.255.255.1:9` with a 300 ms connect
  timeout. `open` returns in 0.54 ms, an unrelated valtron task finishes at about 50 ms,
  and the head fails with `Connect` at the 300 ms budget. On the old code `open` blocks
  for 302.29 ms and returns `Err(connect timed out)`; the test fails. Observed with the
  blocking dial restored: `a blackhole dial must not fail inside open after 302.29ms:
  transport connect failure: connection timed out`. The environment skip is in place for
  `EACCES`, `ENETUNREACH` and `EHOSTUNREACH`; the local run did not skip.
- `a_unix_socket_dial_still_connects`: a `UnixListener` accepts the dial. The test passes
  on both the old and the new code, which is its purpose.

Commands and results:

| command | result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings` | clean |
| `cargo test --profile uat -p foundation_connectrpc` | 245 passed, 0 failed, 1 ignored |
| `cargo test --profile uat -p foundation_proxy` | 129 passed, 0 failed (the known macOS websocket test passed this run) |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc` | passes |

Unix limitation: `H2Transport::unix` still calls the blocking `UnixStream::connect`, and
the blocking call has no connect timeout. The crate records it in `docs/bug-backlog.md`
§1 for the next free feature slot.

No public API changed, so `foundation_connectrpc` stays 0.0.5 and no consumer requirement
moves. `Cargo.lock` gains `socket2` under the `foundation_connectrpc` package entry.

Status: `pending (CI proof)`.
