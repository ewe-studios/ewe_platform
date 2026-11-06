---
feature: "A Connect client call fails on its own deadline"
status: "done"
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

- [x] Every native client transport enforces `Ctx::deadline()`.
- [x] The tests pass; the first fails on the old code.
- [x] `foundation_proxy` tests still pass, except the known macOS test in backlog §9.
- [x] The backlog §1 entry is deleted.
- [x] CI green. Run `42261737548` (head `cf5ab359a`) passed every job.
      Commits after that head change specification text only.

## Result

2026-11-06. Implemented on branch `spec70/priority-followups`.

### What changed

- **One shared helper.** `with_deadline` in
  `backends/foundation_connectrpc/src/shared/client.rs` races the call future
  against `foundation_core::valtron::sleep_async(remaining)`. If the sleep wins,
  it fires the call's `CancelSignal`, drops the call (and the transport and
  response streams it owns) and returns
  `ConnectError::deadline_exceeded("call deadline exceeded")`. If the deadline
  already passed, it fails before the call is polled, so no connection opens.
- The helper sits above `Transport`, in the shared `Client`, because
  `Transport::open` is synchronous and receives no `Ctx`. Every transport
  (`H1Transport`, `H2Transport`, `H3Transport`, the wasm fetch transport, and a
  future transport) reaches the wire through the `Client` methods, so no
  transport can omit the bound. The four client methods (`unary`'s POST and GET
  paths, `server_stream`, `client_stream`, `bidi_stream`) call it.
- Only `valtron::sleep_async` is used, so the same code compiles and runs on
  native and wasm; no thread and no OS timer is created.
- **New configuration.** `ClientOptions::with_default_deadline(Duration)` sets a
  client-wide default (a `ClientConfig` field, default `None`). A `Ctx` deadline
  wins over it. If both it and `default_timeout` are set and there is no `Ctx`
  deadline, the earlier bound wins. The wire timeout header is unchanged: it
  still comes from the derived deadline (`Connect-Timeout-Ms` or
  `grpc-timeout`).
- The cancel reuses the existing `CancelSignal` that the streaming reader and
  writer tasks already link against.
- **Scope.** The deadline bounds each call through its setup and initial
  response (for a stream, through `response_headers`). A later
  `ServerStream::receive`, `BidiReceiver::receive` and the other facade methods
  on a returned stream are not re-armed by a timer. This limitation is recorded
  in backlog §1.

### Tests

New binary `tests/client_deadline_tests.rs`, registered in `Cargo.toml` with
`required-features = ["rpc_multi"]`. It uses the real `HttpServer` on
`127.0.0.1:0` for the normal path and a real listener that accepts and never
answers for the failing paths (specification rule 3).

- `a_call_fails_on_its_deadline_when_the_server_never_answers` (H1): 200 ms
  deadline, `DeadlineExceeded`, under 1 s.
- `an_h2_call_fails_on_its_deadline_when_the_server_never_answers` (H2): the
  same through the second native transport.
- `a_client_default_deadline_ends_a_call_without_a_ctx_deadline`: proves the new
  `with_default_deadline`.
- `a_call_with_no_deadline_still_completes`: real H1 echo server.
- `a_deadline_in_the_past_fails_without_a_request`: `Duration::ZERO`; the
  listener's non-blocking `accept` returns `WouldBlock`, so no connection was
  made.
- `a_bidi_deadline_in_the_past_fails_without_a_request`: the same for
  `bidi_stream`; its setup is lazy, so the check runs before the transport
  opens.

Old-code proof: only the helper body was reverted to `call.await` (the
deadline was still recorded). The first test then failed after about 3 s with:

```text
assertion `left == right` failed: expected DeadlineExceeded, got: ErrorTrace:
  [0] Context: [unavailable] transport closed without response head (at backends/foundation_connectrpc/src/shared/error/connect_error.rs:232:9)

  left: Unavailable
 right: DeadlineExceeded
```

The helper was restored before the final runs.

### Commands run

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | pass |
| `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings` | pass |
| `cargo test --profile uat -p foundation_connectrpc` | 236 passed, 0 failed (1 doc test ignored) |
| `cargo test --profile uat -p foundation_proxy --no-fail-fast` | all pass except the known macOS `proxy_features_tests::websocket_upgrade_relays_and_tunnels_bytes` (backlog §9); `rpc_proxy_e2e_tests` 17 passed. One earlier run also hit `large_response_tests::proxy_large_response_is_byte_exact`; it passed alone and in the two later full runs. |
| `cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc` | pass, with 3 pre-existing warnings in `shared/router/dispatch.rs` (recorded in backlog §1) |

CI is not run yet, so this feature stays `pending (CI proof)`.

