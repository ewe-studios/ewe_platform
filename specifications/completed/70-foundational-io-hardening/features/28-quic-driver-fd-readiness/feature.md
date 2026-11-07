---
feature: "QUIC driver parks on its UDP socket"
status: "done"
priority: "medium"
depends_on: "02-h2-h3-server-readiness"
branch: "fix/io-28-quic-driver-fd-readiness"
crates: ["foundation_netio", "foundation_http", "foundation_connectrpc", "foundation_iogate"]
backlog: "docs/bug-backlog.md §2 QUIC driver falls back to a 1 ms poll"
---

# Feature 28: QUIC driver parks on its UDP socket

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** User Datagram Protocol (UDP); file descriptor (fd); central
processing unit (CPU); pull request (PR); AeroSpace and Defence Industries Association of
Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## 1. Goal

An idle HTTP/3 endpoint uses no CPU. Its `QuicDriver` task parks on the UDP socket's fd through
the shared reactor, instead of waking every 1 ms.

Today `QuicDriver::next_status` (in `foundation_netio/src/quic/quinn_driver.rs`) already supports
this: with a readiness signal it returns `TaskStatus::Depends`, otherwise it falls back to
`TaskStatus::Delayed(IDLE_POLL)` (1 ms). But nothing injects the signal. The server
(`foundation_http/src/native/server/h3_server.rs` `serve_h3`, and `foundation_connectrpc`'s H3
transport) creates the driver with `QuicDriver::server(..)` and never calls
`with_fd_readiness`. The driver therefore polls at 1 ms.

Two things block the wire-up today:

1. The driver has no way to clear edge-triggered readiness before each receive. The reactor
   latches readiness until it is cleared. The H2 handler clears its `FdReadiness` before every
   read (`arm_wakers`). The `QuicDriver` holds an `EventReadinessPtr` (`Arc<dyn EventReadiness>`),
   whose trait has only `is_ready`, no `clear`. Without a clear, the first datagram would leave
   the socket "ready" forever and the driver would be re-polled in a tight loop.
2. `serve_h3` does not wrap the driver's raw UDP socket fd in `FdReadiness` and pass it in.

## 2. Where

- `foundation_netio/src/quic/quinn_driver.rs`:
  - `QuicDriver { fd_readiness: Option<EventReadinessPtr>, .. }` and
    `with_fd_readiness(fd)` (already written).
  - `next_status`: `Some(fd) => TaskStatus::Depends(..)`, `None => TaskStatus::Delayed(self.idle)`.
  - `IDLE_POLL = 1 ms` (line 56) — the fallback this feature removes for the fd-backed path.
- `foundation_http/src/native/server/h3_server.rs` `serve_h3` — builds `QuicDriver::server(..)`
  and `send`s it without `with_fd_readiness`.
- `foundation_connectrpc/src/native/transport/h3.rs` — the other caller, if it drives its own
  `QuicDriver`.
- `foundation_iogate/src/native/fd_readiness.rs` `FdReadiness` — has `clear()` and `is_ready`.

## 3. Fix

1. Give the driver a clear hook. Two options; pick the smaller one that keeps the H2 handler
   unchanged:
   - Add `fn clear(&self) {}` (a default no-op) to `EventReadiness` in
     `foundation_core/src/valtron/task.rs`, and implement it on `FdReadiness` to call its
     existing `clear`. The driver calls `fd.clear()` before `pump_recv`.
   - Or, store the concrete `FdReadiness` (not the erased pointer) in `QuicDriver` and call
     `clear()` on it directly.
2. In `QuicDriver::next_status`, clear the readiness before the receive (`pump_recv`), exactly as
   the H2 handler clears before its read. Clearing after the receive would erase an edge that
   arrived after a `WouldBlock`.
3. In `serve_h3` (and the connectrpc H3 transport), wrap the driver's socket in `FdReadiness`
   and call `with_fd_readiness`. `QuicDriver::socket()` returns `Arc<UdpSocket>`; take its raw
   fd with `AsRawFd`. Register a `FdReadiness` (readable only, no writable) and pass
   `Arc::new(readiness)`.
4. Compose the socket readiness with the next connection timer. A `Depends` sleeper has no
   deadline of its own, so parking purely on `Depends(fd)` would starve QUIC timers (loss
   detection, idle timeout, keep-alive). Mirror the H2 handler: return
   `Depends(AnyReadiness::either(fd, DeadlinePassed(next_timeout)))`, where `next_timeout` is
   the earliest `quinn_proto::Connection::poll_timeout()` across all connections (or `Depends(fd)`
   alone when no connection has a pending timer). `DeadlinePassed` is a small `EventReadiness`
   that is ready once an `Instant` has passed.

## 4. Tests

- Extend `foundation_netio/tests/http3/waker_tests.rs` `idle_h3_connection_uses_no_cpu` so the
  driver is built with `with_fd_readiness`, and assert the idle driver polls a tiny number of
  times over one second (`CountingDriver` counts `next_status` calls; a parked driver stays at
  ~0, a 1 ms poller fires ~1000). This replaces the old process-wide `getrusage(RUSAGE_SELF)`
  measurement, which flaked under a full parallel `quic` run.
- Add a test that the driver parks on the fd: with `with_fd_readiness` attached and no traffic,
  the driver returns `Depends`, not `Delayed` (`driver_parks_on_fd_readiness_not_delayed`), and
  that without the signal it still returns `Delayed`.
- Keep `h3_accept_task_parks_until_a_connection_arrives` and all feature 02 tests green.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_netio foundation_http foundation_connectrpc foundation_iogate foundation_core; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
for i in $(seq 1 20); do
  timeout 300 cargo test -p foundation_netio --profile dev --features multi \
    --test foundation_netio_tests http3::waker_tests::idle_h3_connection_uses_no_cpu \
    || { echo "FAILED run $i"; exit 1; }
done
```

## 6. Done when

- [x] The driver clears the socket readiness before each receive.
- [x] `serve_h3` injects `with_fd_readiness`; the driver returns `Depends`, not `Delayed`, when
      it has a signal.
- [x] The no-fd fallback (1 ms) still works and is still reachable for transports without a
      pollable fd (feature 06).
- [x] The CPU test passes 20 runs in a row.
- [x] The backlog entry is deleted.
- [x] CI green.

## 7. Out of scope

Transports with no pollable fd (feature 06). WebTransport and SSE wake paths (feature 07).
