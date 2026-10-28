---
feature: "Non-blocking connect and TLS handshake"
status: "done"
priority: "high"
depends_on: ["03-client-body-readiness"]
branch: "fix/io-01-valtron-parking"
crates: ["foundation_netio", "foundation_iogate"]
backlog: "docs/bug-backlog.md §2 Non-blocking work not done yet: non-blocking connect and TLS handshake (task #23, deleted); DNS entry added"
---

# Feature 04: Non-blocking connect and TLS handshake

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Transmission Control Protocol (TCP); Transport Layer Security
(TLS); Domain Name System (DNS); Hypertext Transfer Protocol (HTTP); round-trip time (RTT);
pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

Opening an HTTP client connection does not block a valtron worker. The TCP connect and the TLS
handshake park the task on socket readiness and resume when the socket is ready.

Today:
- `foundation_netio/src/native/connection.rs` `Connection::with_timeout` (around line 520) calls
  `TcpStream::connect_timeout`, which blocks the thread for up to the connect timeout (one RTT
  at best, the whole timeout for an unreachable host).
- The TLS upgrade runs its handshake in place, blocking for several RTTs.
- These run inside the HTTP client task `foundation_netio/src/http/tasks/request_intro.rs`
  (calls at around lines 419 and 501), so a slow or dead host holds a worker.

## 2. Design

### 2.1 Non-blocking TCP connect

1. Create the socket non-blocking (`socket2` is already an optional dependency of
   `foundation_netio`; make it a normal dependency for native targets if needed).
2. `connect` returns `EINPROGRESS` (Unix) or `WSAEWOULDBLOCK` (Windows). The task parks on
   **write readiness** of the fd through the shared reactor (`FdReadiness`,
   `foundation_iogate/src/native/fd_readiness.rs`).
3. When writable, read `SO_ERROR` (`socket.take_error()`). `None` means connected; `Some(err)`
   means the connect failed with `err`.
4. The connect timeout becomes a **deadline**: the task's readiness wait includes the deadline,
   and the deadline expiring gives `ConnectionTimeout`.
5. Try each resolved address in order (as `connect_timeout` callers do today), each with the
   remaining time.

First read `foundation_iogate` `connect_completion` (exported from `src/lib.rs`): it may already
do part of this. Reuse it if it fits; say in the PR body why or why not.

### 2.2 Non-blocking TLS handshake

With rustls (`foundation_netio/src/native/ssl/rustls.rs`): drive the handshake with
`conn.complete_io(&mut socket)` on the non-blocking socket. On `WouldBlock`, park on
`conn.wants_read()` → read readiness, `conn.wants_write()` → write readiness. The handshake
deadline is the same connect deadline (or its own config field if one exists; see feature 09).

For openssl and native-tls (if compiled): use their non-blocking handshake APIs
(`MidHandshakeSslStream` / `HandshakeError::WouldBlock`) the same way. If a backend cannot do
it, keep the blocking handshake for that backend only, behind a clear comment and a backlog
entry; feature 06 handles transports without readiness.

### 2.3 Where it plugs in

`request_intro.rs` is a state machine. Add states: `Connecting { socket, addrs, deadline }` and
`TlsHandshake { conn, deadline }`. Each returns `TaskStatus::Depends(readiness)` on
`WouldBlock`, and moves on when ready. The public sync constructors
(`HttpClientConnection::connect…`) stay for sync callers at a real boundary; they can drive the
same state machine to completion with a blocking wait.

### 2.4 Correctness rules

1. The connect result comes from `SO_ERROR`, never from "writable" alone.
2. Deadlines use the valtron clock or the existing timeout config, not a new constant.
3. Every error keeps its kind: refused, unreachable, timed out, TLS alert. Tests check the kind.
4. DNS resolution still blocks (`SystemDnsResolver`). That is out of scope here: add a backlog
   entry "DNS resolution blocks the HTTP client task" with the file and line.

## 3. Tasks

- [x] T1. Branch from the base after feature 03.
- [x] T2. Read `connect_completion` in `foundation_iogate`. Decide reuse or not; write it down.
- [x] T3. Non-blocking connect (§2.1), with the deadline and the address loop.
- [x] T4. Non-blocking rustls handshake (§2.2). Then the other TLS backends that are compiled.
- [x] T5. New states in `request_intro.rs` (§2.3). Sync constructors drive the same code.
- [x] T6. Regression tests in `backends/foundation_netio/tests/` (pick the right existing test
      directory, for example `tcp_and_tls/`, and register in its `mod.rs`):
  - `connect_to_blackhole_does_not_hold_a_worker`: connect to a non-routable address (for
    example `10.255.255.1:9`) with a 2 s timeout from a valtron task on a 2-thread pool, while a
    second task on the same pool runs a 10 ms ticker. Assert the ticker ticked at least 150
    times during the connect (the old code blocks one of only two workers) and the connect
    ends with `ConnectionTimeout` after 2 s ± 0.5 s. If the CI network rejects that address
    at once, use a local listener with a full backlog instead; write which in the test.
  - `connect_refused_reports_refused`: connect to a closed local port; assert the error kind.
  - `tls_handshake_waits_on_slow_server`: a local TLS server that waits 500 ms before it answers
    the ClientHello; assert the ticker keeps ticking and the handshake succeeds.
  - `tls_handshake_bad_cert_reports_alert`: a self-signed server without trust; assert the error
    kind.
- [x] T7. Backlog: delete the task #23 line; add the DNS entry (§2.4 rule 4). Update
      `progress.md`, `learnings.md`.

## 4. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_netio foundation_iogate foundation_connectrpc foundation_deployment; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new test.
rg -n 'connect_timeout\(' backends/foundation_netio/src   # only the sync boundary path, with a comment
```

## 5. Done when

- [x] All §4 commands pass; new tests passed 20 runs in a row.
- [x] `connect_to_blackhole_does_not_hold_a_worker` fails on the base code and passes now.
- [x] No `connect_timeout(` or in-place handshake left on the task path.
- [ ] CI green (Linux and macOS if it runs).

## 6. Out of scope

DNS (backlog entry only). Pushed request bodies (feature 05). WireGuard overlay and other
transports without an fd (feature 06).

## 7. Review checklist (main session)

- [ ] `SO_ERROR` is checked after writability.
- [ ] `wants_read`/`wants_write` choose the right interest at each step.
- [ ] The deadline covers connect + handshake as configured and cannot be reset by a partial
      handshake.
- [ ] Error kinds survive to the caller.
