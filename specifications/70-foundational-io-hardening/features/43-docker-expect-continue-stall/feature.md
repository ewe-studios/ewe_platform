---
feature: "Docker POST requests no longer stall 2 minutes on Expect: 100-continue"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_netio", "foundation_deployment_docker"]
catalogue_rows: [4]
---

# Feature 43: `100 Continue` stall

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (backlog §6)

Against Docker (seen on OrbStack), every POST with a body (`containers/create`,
`containers/start`) waits exactly 2 minutes before the body is sent. GETs are fast. The
client sends `Expect: 100-continue` (`http/tasks/request_redirect.rs` ~line 535,
`ClientConfig::expect_continue_enabled`). It should send the body after
`expect_continue_read_timeout` (default 3 s) if no `100` arrives; 2 minutes matches the
response-head idle timeout instead.

## Research to do first (write the answer in the PR body)

The `AwaitingContinue` state parks on "readable OR the send-body timer"
(`park_for_continue`, ~line 1188: `AnyReadiness::either(io, DeadlineReadiness::at(send_body_at))`).
Two places can lose the timer; find which one runs for a Unix-socket Docker connection:

1. `park_for_continue` when `connection.park_readiness(Readable)` returns `None` (the
   `else` branch at ~line 1189): what does it return? If it falls back to a park without the
   deadline, the task sleeps until the idle timeout.
2. The other `self.park(&connection, IoInterest::Readable, &mut idle, …)` call in the
   `AwaitingContinue` path (~line 1132, after a partial interim read): it parks on read only,
   with no send-body deadline.

Reproduce without Docker: a test server (`foundation_testing` has HTTP test servers) that
accepts the request head and never sends `100 Continue`; over TCP and over a Unix socket.

## Fix

- Every park in `AwaitingContinue` includes the `send_body_at` deadline. When the deadline
  passes, send the body (RFC 9110 §10.1.1: a client that sent `Expect` need not wait forever).
- The Docker client (`foundation_deployment_docker`): Docker never needs the handshake. Build
  its HTTP client with `expect_continue(false)` (both Unix socket and TCP), with a comment that
  cites this bug. Keep the netio fix too: other servers can behave the same.

## Tests

- netio: `expect_continue_sends_body_after_timeout_tcp` and `…_unix` (fail on master if the
  cause is in netio): server never sends 100; set `expect_continue_read_timeout` = 300 ms;
  assert the server receives the body within 1 s.
- docker: `docker_client_sends_no_expect_header` (a mock daemon records headers).
- Run one real Docker integration test if Docker is available here and report the time
  (before: about 2 min per POST).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_netio --profile dev
python3 scripts/ci/ci.py run foundation_deployment_docker --profile dev
```
Flake check: 20 runs of the new netio tests.

## Done when

- [x] Root cause named in the PR body with the line. Netio tests fail on master, pass now.
- [x] Backlog §6 entry deleted.
