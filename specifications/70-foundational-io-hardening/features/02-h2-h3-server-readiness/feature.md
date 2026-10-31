---
feature: "HTTP/2 and HTTP/3 servers park on readiness"
status: "pending"
priority: "critical"
depends_on: ["01-valtron-parking"]
branch: "fix/io-02-h2-h3-readiness"
crates: ["foundation_netio", "foundation_http", "foundation_connectrpc", "foundation_iogate", "foundation_nativeapis"]
backlog: "docs/bug-backlog.md §2 QUIC streams register no waker; §3 h2 connection polls on a 10 ms timer; §3 H3AcceptTask polls on a fixed 5 ms delay"
starting_point: "fix/foundational-readiness commits 31467ebd4 (QUIC/HTTP/3) and ed13119db (HTTP/2)"
---

# Feature 02: HTTP/2 and HTTP/3 servers park on readiness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol version 2 and 3 (HTTP/2, HTTP/3);
QUIC, the transport under HTTP/3 (a name); file descriptor (fd); central processing unit (CPU);
pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

An idle HTTP/2 or HTTP/3 connection uses no CPU. Its task parks until the socket, the QUIC
driver, a handler's response pipe, a timer it needs (idle deadline), or shutdown wakes it.

Today:
- `foundation_http` `native/server/h2_connection.rs` waits on a 10 ms `POLL_DELAY` (1 ms in the
  handshake) when idle.
- `foundation_http` `native/server/h3_server.rs` `H3AcceptTask` waits on a fixed 5 ms delay, a
  hard-coded limit.
- QUIC streams register no waker, so every HTTP/3 future must wake itself and busy-polls a
  worker. `foundation_connectrpc` `native/h3_serve.rs` `map_h3` works around it.

## 2. Starting point (already written; must be verified, not trusted)

Two commits on `fix/foundational-readiness` do most of this. Their messages describe them.

- `31467ebd4` "park HTTP/3 futures on the connection instead of busy-polling":
  `QuicConnection/QuicSendStream/QuicRecvStream::register_waker`; the quinn driver keeps parked
  wakers in `ConnState` and wakes them on every quinn event, and wakes `QuicDriver::accept_wakers`
  when a connection finishes its handshake; `H3Connection/H3Request::register_waker`;
  `h3_serve.rs` registers its waker and parks (self-wake only as a fallback);
  `H3AcceptTask` parks on its waker or shutdown. Test: `foundation_netio/tests/http3/waker_tests.rs`.
- `ed13119db` "park the h2 connection task on its socket; fix partial h2 flushes":
  `H2Connection::flush_write` no longer re-sends a written prefix after `WouldBlock`;
  `H2ConnectionHandler` parks (`Depends`) on the socket through the shared reactor (or the
  overlay's read waker), the handlers' response pipes, body-pipe vacancy, the idle deadline and
  shutdown; write errors other than `WouldBlock` close the connection; `FdReadiness` in
  `foundation_iogate`; the reactor drain thread requests an executor resume; pipe
  `register_waker`. Tests: `foundation_http/tests/h2_parking_tests.rs`,
  `foundation_netio/tests/http2/conn_eof_tests.rs`.

## 3. Tasks

- [x] T1. Create `fix/io-02-h2-h3-readiness` from the base, after feature 01 is merged (or stacked
      on its branch). `git cherry-pick 31467ebd4 ed13119db`. Resolve conflicts with master.
- [x] T2. Review both commits line by line against §4 (correctness rules). Write each rule and
      where the code keeps it in the PR body. Fix what breaks a rule.
- [x] T3. List every remaining timer or self-wake in these files and either remove it or write
      why it must stay (a comment in the code and a line in the PR body):
      `foundation_http/src/native/server/{h2_connection.rs,h3_server.rs}`,
      `foundation_connectrpc/src/native/h3_serve.rs`, `foundation_netio/src/quic/*.rs`,
      `foundation_netio/src/http3/connection.rs`. The idle deadline timer is allowed. A fallback
      self-wake is allowed only for a backend that cannot register a waker, and it must log once
      at `debug`.
  - Remaining timers, each justified: `h2_connection.rs` `POLL_DELAY` (10 ms) and
    `HANDSHAKE_POLL_DELAY` (1 ms) are the fallback only for a socket that cannot be watched
    (non-unix, or the overlay transport). `quinn_driver.rs` `IDLE_POLL` (1 ms) stays until
    `with_fd_readiness` is wired up — tracked as feature 28.
- [x] T4. Regression tests. Add to the existing test files, registered in their `mod.rs`:
  - `foundation_http/tests/h2_parking_tests.rs`: `idle_h2_connection_uses_no_cpu`. Open an HTTP/2
    connection, send one request, then stay idle 1 s. Assert the server process used under
    50 ms of CPU in that second (read `/proc/self/stat` utime+stime before and after, or
    `getrusage`). On the old code the 10 ms timer wakes 100 times.
  - `foundation_http/tests/h2_parking_tests.rs`: `h2_request_after_idle_is_served_at_once`. After
    1 s idle, a request completes in under 50 ms (proves the socket wake works).
  - `foundation_netio/tests/http3/waker_tests.rs`: keep the existing tests; add
    `idle_h3_connection_uses_no_cpu` (same method as HTTP/2).
  - `foundation_http`: `h3_accept_task_parks_until_a_connection_arrives`. No 5 ms polling: count
    the accept task's polls while idle for 500 ms; assert under 5.
  - `foundation_netio/tests/http2/conn_eof_tests.rs`: keep; add
    `partial_flush_resumes_without_resending` if the commit has no direct test of the
    `flush_write` fix (a writer that accepts 3 bytes, then `WouldBlock`, then the rest; assert the
    bytes on the wire equal the frames once).
- [x] T5. Delete the three backlog entries. Update `progress.md` and `learnings.md`.

## 4. Correctness rules the code must keep

1. **Register before check.** A task registers its waker, then checks for work, then returns
   `Pending`. If it checks first, an event between the check and the registration is lost.
2. **Wake on every event** that can make progress: readable data, write space after a
   `WouldBlock`, a new stream, a new connection, flow-control credit, a handler's response
   chunk, a closed peer, shutdown.
3. **No busy loop on error.** A read or write error other than `WouldBlock` ends the connection.
4. **Shutdown wakes everything.** A parked task must see shutdown within one wake.
5. **One waker per parked task is enough;** replacing a stale waker with the newest is correct.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for c in foundation_netio foundation_http foundation_connectrpc foundation_iogate foundation_nativeapis; do
  python3 scripts/ci/ci.py run $c --profile dev || exit 1
done
cargo check --workspace --all-targets --profile dev
# 20-run flake check for each new test and for these existing ones:
#   foundation_http h2_parking_tests (all), foundation_netio http3 waker_tests (all),
#   foundation_netio http2 conn_eof_tests (all), foundation_connectrpc conformance_tests (all)
rg -n 'POLL_DELAY|from_millis\(5\)|from_millis\(10\)' backends/foundation_http/src/native/server   # each hit must be justified in the PR body
```

## 6. Done when

- [ ] All §5 commands pass; new tests and the listed suites passed 20 runs in a row.
- [ ] The CPU tests fail on the base code and pass on the new code (PR body shows both).
- [ ] Every remaining timer or self-wake has a written reason.
- [ ] CI is green, including the macOS job if it runs.

## 7. Out of scope

HTTP/1.1 accept loop (feature 05). Transports without a pollable fd (feature 06): keep their
current timer fallback, but make sure it is the only path that uses a timer.

## 8. Review checklist (main session)

- [ ] Rule 1 (register before check) in every `poll` that returns `Pending`.
- [ ] The quinn driver wakes on credit and on new streams, not only on data.
- [ ] `flush_write` keeps unwritten bytes in order across many `WouldBlock`s.
- [ ] CPU thresholds have margin; tests read CPU for the right process or thread.
