---
feature: "A Connect client call fails on its own deadline"
status: "pending"
lane: "priority follow-up (limitation found while diagnosing feature 121)"
model: "sonnet"
branch: "spec70/f123-client-deadline"
crates: ["foundation_connectrpc"]
catalogue_rows: []  # from docs/bug-backlog.md §1; not a §8 catalogue row
---

# Feature 123: a Connect client call fails on its own deadline

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **call**: one client request and its response stream.
- **deadline**: the latest time by which a call must finish.
- **parked**: a task that waits for readiness and does nothing else.

## Problem (found 2026-11-06)

`Ctx::with_deadline` (`foundation_connectrpc/src/shared/context/mod.rs:228`) only writes
`Instant::now() + d` into `RequestContext`. No timer starts, and nothing cancels a call
that passes the timestamp. A call parked on a read therefore ignores its deadline.

The proxy e2e tests show the result. `connect_streams_spread_across_grpc_backends` gives
every client a 15 s deadline and still ran to the test's 30 s cap (GitHub run
42087629722). `h2_grpc_request` sets no deadline and waited 60 s for the HTTP/2 idle
timeout (run 42099300473).

The same timestamp is useful to a server: it can travel as `Connect-Timeout-Ms` (Connect)
or `grpc-timeout` (gRPC). The defect is that the client trusts it and does not enforce it.

## Design

### 1. Enforce the deadline in the client transport

- On send, read `ctx.deadline()`. When it is `Some`, wrap the call future in a
  `select` with `foundation_core::valtron::sleep_async(remaining)`.
- When the sleep wins, cancel the call, drop the response stream and return the
  transport's deadline error with `DeadlineExceeded` semantics.
- When the deadline already passed at send time, fail at once without a network call.
- Keep the timeout header as today; a server can still finish early.

Apply it on each native client transport (`H1Transport`, `H2Transport`, the Connect
codecs), in one shared helper so the bound cannot be forgotten on a new transport.

### 2. Keep wasm and native behaviour the same

`foundation_connectrpc` builds for wasm. Use the valtron sleep, not a thread or an OS
timer, so the same code runs on both. The timer is a configuration value, not a constant
(rule 8): add `ClientOptions::with_default_deadline(Duration)` and document that the
`Ctx` deadline wins.

### 3. Follow the existing cancellation path

`CancelSignal` already exists. Use it for the cancel, so a caller that links signals sees
the same behaviour as an explicit cancel.

## Tests

Test target: `tests/client_deadline_tests.rs`, registered in `tests/mod.rs`.

- `a_call_fails_on_its_deadline_when_the_server_never_answers`: a listener that accepts
  and never replies; a client with a 200 ms deadline returns a deadline error in under
  1 s. It fails on the old code (the call waits until the test kills it).
- `a_call_with_no_deadline_still_completes`: the normal path keeps working.
- `a_deadline_in_the_past_fails_without_a_request`: the server sees no connection.
- Use the real server on `127.0.0.1:0`, not a stub (specification rule 3).

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc
cargo test --profile uat -p foundation_proxy
python3 scripts/ci/ci.py run foundation_connectrpc --profile uat
```

## Done when

- [ ] Every native client transport enforces `Ctx::deadline()`.
- [ ] The four tests pass; the first fails on the old code.
- [ ] `foundation_proxy` tests still pass.
- [ ] The backlog §1 entry is deleted after merge.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. Found while diagnosing feature 121; the proxy tests
keep their own bound (feature 121, Design 4) until this lands.
