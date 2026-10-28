---
feature: "Readiness for transports without a pollable fd"
status: "pending"
priority: "medium"
depends_on: ["05-push-body-and-accept"]
branch: "fix/io-06-transports-without-fd"
crates: ["foundation_connectrpc", "foundation_netio", "foundation_iogate", "foundation_wireguard"]
backlog: "docs/bug-backlog.md §1 Transports with no pollable fd fail; §2 the remaining transports (task #25)"
---

# Feature 06: Readiness for transports without a pollable fd

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** file descriptor (fd); Transport Layer Security (TLS); remote
procedure call (RPC); pull request (PR); AeroSpace and Defence Industries Association of Europe
(ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Every transport that `RawStream` / `Connection` can hold reports readiness, so RPC and HTTP code
parks on it. Today `foundation_connectrpc/src/native/conn_io.rs` readiness waits (`fd()`,
`gone_now`, `ready_now`) return `Unsupported` for:
- the WireGuard overlay stream;
- io_uring completion sockets (`foundation_iogate` `CompletionSocket`, feature `uring`);
- openssl and native-tls streams.

With rustls, the code polls the fd in the application's direction only, so a TLS session that
needs the other direction in the middle of a stream (a key update, a renegotiation-like write
during a read) waits until the timeout.

## 2. Design

### 2.1 One readiness interface for every transport

Add (or extend, if features 02–05 started it) a method on the transport enum:

```rust
/// A readiness signal for `interest`, or `None` when the transport cannot report it.
fn readiness(&self, interest: IoInterest) -> Option<EventReadinessPtr>;
```

Implement it per transport:

| Transport | Source of readiness |
|---|---|
| Plain TCP, Unix socket | the shared reactor (`FdReadiness`) — exists |
| rustls over TCP | the reactor on the socket fd, with the interest from `wants_read()` / `wants_write()`, not from the application's direction |
| openssl, native-tls | the reactor on the underlying fd; on `WouldBlock`, map the TLS error to the interest it needs (`SSL_ERROR_WANT_READ` / `WANT_WRITE`) |
| WireGuard overlay | the overlay's own wake registration (`register_read_waker` exists); add a write-space wake if the overlay has back-pressure |
| io_uring `CompletionSocket` | completion events; `CompletionSocket::readiness` exists — wire it to `EventReadinessPtr` |

`conn_io.rs` then asks the transport for readiness instead of reading `fd()` itself.

### 2.2 Correctness rules

1. For TLS, the interest comes from the TLS state, not from what the application asked for.
2. A transport that truly cannot report readiness returns `None`, and the caller uses one
   documented fallback (a configured poll interval) with a `debug` log once per connection.
3. No change in behaviour for plain TCP.

## 3. Tasks

- [ ] T1. Branch from the base after feature 05.
- [ ] T2. Read `conn_io.rs` and list each `Unsupported` path. Read each transport's code.
- [ ] T3. Implement `readiness` per transport (§2.1). Route `conn_io.rs` through it.
- [ ] T4. Regression tests in `backends/foundation_connectrpc/tests/` (and the transport crate's
      `tests/` where the code lives):
  - `overlay_rpc_parks_and_completes`: a unary and a server-streaming RPC over the WireGuard
    overlay (use the in-memory overlay test harness if one exists; check
    `foundation_wireguard/tests`). Assert success and no `Unsupported` error.
  - `rustls_opposite_direction_does_not_stall`: a server that sends a TLS key update (rustls
    `refresh_traffic_keys` or the closest API) in the middle of a stream while the client only
    reads. Assert the stream finishes in under 1 s (the old code waits for the timeout).
  - `uring_rpc_parks_and_completes` behind `cfg(feature = "uring")` on Linux.
  - openssl / native-tls: one round-trip test each, if those features are built in CI.
- [ ] T5. Delete the backlog entries. Update `progress.md`, `learnings.md`.

## 4. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_connectrpc foundation_netio foundation_iogate foundation_wireguard; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo test -p foundation_iogate --profile dev --features uring   # Linux only
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new test.
rg -n 'Unsupported' backends/foundation_connectrpc/src/native/conn_io.rs   # each hit has a reason
```

## 5. Done when

- [ ] All §4 commands pass; new tests passed 20 runs in a row.
- [ ] No transport returns `Unsupported` for readiness unless the PR body explains why it cannot.
- [ ] CI green.

## 6. Review checklist (main session)

- [ ] TLS interest follows `wants_read`/`wants_write`.
- [ ] The overlay wake fires on both data and close.
- [ ] The fallback poll interval is configuration and logged.
