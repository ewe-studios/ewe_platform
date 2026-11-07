---
feature: "Propagate connection errors and give keep-alive an idle timeout"
status: "completed"
priority: "high"
depends_on: []
crates: ["foundation_http", "foundation_netio"]
issues: [284, 288]
model: "sonnet"
branch: "spec73/f04-http-connection-correctness"
---

# Feature 04: HTTP connection correctness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** HTTP/1.1 (HTTP/1.1); WebSocket (WS); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close [#284](https://github.com/ewe-studios/ewe_platform/issues/284) (middleware and 404
paths ignore errors and desync keep-alive) and
[#288](https://github.com/ewe-studios/ewe_platform/issues/288) (the idle timeout is a
size-based read timeout).

## 2. Problem (measured on master, 2026-11-07)

1. **#288 — real, with moved lines.** `foundation_http::KeepAliveConfig` (`native/server/mod.rs:47`)
   has no idle timeout of its own. `idle_timeout()` (`native/server/connection.rs:193-195`)
   returns `TimeoutCalculator::calculate_read_timeout`, which for a request with no expected
   body size returns `TimeoutConfig::min_read_timeout`
   (`foundation_netio/src/shared/http/timeout.rs:507-512`). The server default is 120 s
   (`mod.rs:65`), so the h1 keep-alive wait is a read timeout by accident. The WebSocket
   task is worse: `foundation_netio/src/websocket/native/task.rs:762` builds its read
   timeout from `TimeoutCalculator::new()`, whose `TimeoutConfig::default()` has
   `min_read_timeout = 100 ms` (`timeout.rs:200`). A quiet WS connection can close after
   100 ms.
2. **#284 — already fixed. Close it with evidence.** Commit `51e61098b` ("close instead of
   reusing a connection with an unread body; report write errors") added
   `body_left_unread` and error propagation to the middleware path
   (`native/server/connection.rs:821-845`) and the 404 path (`connection.rs:898-906`). No
   `let _ =` on a render or write remains in `foundation_http/src`. The feature adds a
   regression test that pins both paths, or closes the issue with this evidence.

## 3. Requirements

1. A keep-alive connection waits `idle_timeout`, not a read timeout, for its next request.
   The default is 75 s, the nginx `keepalive_timeout` default. The value is a field with a
   builder method and a doc comment that names nginx (root rule 8).
2. The WebSocket read wait uses an idle timeout with the same default, not the 100 ms
   minimum read timeout.
3. #284's rule stays enforced and gets tests: a render or write error is propagated, and an
   unread request body closes the connection, so leftover bytes never become a request.

## 4. Design (completed 2026-11-07)

### 4.1 h1 keep-alive: an idle timeout on `KeepAliveConfig`

1. Add `pub idle_timeout: Duration` to `KeepAliveConfig` (`native/server/mod.rs:47`).
   `defaults()` sets 75 s. Doc comment: "nginx `keepalive_timeout` default".
2. `with_idle_timeout` sets the field. It no longer rewrites
   `min_read_timeout` through the calculator.
3. `ConnectionHandler::idle_timeout()` (`connection.rs:193`) returns the field.
   `handle_idle` (`connection.rs:314`) then waits the idle timeout, not a read timeout.
4. `disabled()` sets `idle_timeout` to 1 s, so a disabled keep-alive closes at once.

### 4.2 WebSocket: an idle timeout on the WS task config

`foundation_netio/src/websocket/native/task.rs:762` uses
`calculate_read_timeout(&ctx)` (100 ms with the default calculator). Add:
1. `pub idle_timeout: Duration` to the WS task config (the struct that holds
   `timeout_calculator`, around line 53), default 75 s, with a builder. Doc comment names
   nginx.
2. Line 762 waits `config.idle_timeout`, not `calculate_read_timeout`.
3. `foundation_http` passes its `KeepAliveConfig::idle_timeout` when it builds the WS task.
   State the exact constructor that changes.
4. The 100 ms sleep at line 887 is a poll cadence, not a timeout. Leave it.

### 4.3 #284

No code. Add `a_404_with_a_body_does_not_desync_the_next_request` and
`a_middleware_write_error_closes_the_connection` as regression tests. They must pass on
master; record that, and close the issue with the commit evidence.

## 5. Tasks

- [ ] Verify both defects on master (done above).
- [ ] Add `idle_timeout` to `KeepAliveConfig` and use it in the h1 keep-alive wait.
- [ ] Add `idle_timeout` to the WS task config and use it in the WS read wait.
- [ ] Wire the server config into the WS task.
- [ ] Add the two #284 regression tests.
- [ ] Add the #288 tests.
- [ ] Run the verify commands. Record the results.
- [ ] Close #284 (evidence) and #288 (fix) in the PR body.

## 6. Tests

Tests live in `backends/foundation_http/tests/` (and `foundation_netio/tests/` for the WS
wait), registered in the crate's test module.

1. `keepalive_waits_the_default_idle_timeout`: a `SimulatedClock`; assert a connection quiet
   for 1 s with the 75 s default stays open, and closes after the clock passes the idle
   timeout. Old code: the default is a 120 s read timeout, and the test fails on the
   assertion for the configured value (set `with_idle_timeout(2 s)` and prove the close at
   2 s, not at the calculator's value).
2. `websocket_wait_uses_the_idle_timeout`: a `SimulatedClock`; assert the WS read wait lasts
   the configured idle timeout, not 100 ms. Old code: closes at 100 ms.
3. `a_404_with_a_body_does_not_desync_the_next_request` (pins #284).
4. `a_middleware_write_error_closes_the_connection` (pins #284).
5. `keepalive_disabled_closes_after_one_second`.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_http -p foundation_netio
cargo clippy --profile uat -p foundation_http --no-deps --all-targets -- -D warnings
cargo clippy --profile uat -p foundation_netio --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_http
cargo test --profile uat -p foundation_netio
```

## 8. Done when

- A quiet keep-alive connection lives until the idle timeout, and the default names nginx.
- The WS read wait is no longer the 100 ms minimum read timeout.
- The #284 regressions are pinned by tests that pass on master.
- #288 is closed by the fix; #284 is closed with the commit evidence.

## 9. Result (2026-11-07)

Commits `a536471a8` and `0236e7e2a` (the bump). Merged into
`spec73/high-severity-fixes` as `dec71174d`. New `KeepAliveConfig::idle_timeout` (default 75 s, nginx
`keepalive_timeout`), `ConnectionHandler::idle_timeout`, and `WebSocketConnectInfo::idle_timeout`
with a `WebSocketTask::with_idle_timeout` builder. `read_timeout` in
`connect_with_delivery`/`connect_with_pool_and_delivery` now acts as the idle timeout. Tests:
4 in `foundation_http/tests/connection_correctness_tests.rs`, 2 in
`foundation_netio/tests/websocket/idle_timeout_tests.rs`. Both old-code failures were
observed: the WS wait measured 101 ms against a configured 300 ms, and the h1 default was
120 s. `foundation_http` 77 passed, `foundation_netio` 1302 passed. #284 is confirmed fixed
(`51e61098b`) and pinned by two tests.

Two design points did not hold:
1. Design 4.2 item 3 ("wire from server config") has no call site. `foundation_http` never
   builds a `WebSocketTask`; its upgrade path only writes the 101 and hands over. The
   per-call `read_timeout` is the effective path.
2. Design test 2 asked for a `SimulatedClock`. The WS wait is a kernel socket timeout
   (`SO_RCVTIMEO`), not a timer, so the test measures real elapsed time with a bounded range.

Both crates are 0.0.9 after the follow-up bump. Filed #375 (medium: `sleep_between` is still
ignored), #376 (medium: the protocol-detect refusal drops its write result) and #377 (low:
`KeepAliveConfig` doc comments disagree with `defaults()`).
