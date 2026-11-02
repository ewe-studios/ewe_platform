---
feature: "foundation_netio: remove collect_exchange/drain_exchange_head and QuicListener (dead code that spins or sleeps)"
status: "pending"
lane: "A"
model: "haiku"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_netio", "foundation_deployment"]
catalogue_rows: [2, 3]
---

# Feature 42: Dead wait loops in netio

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem

1. `shared/client/body_reader.rs`: `collect_exchange` and `drain_exchange_head` loop on
   `Some(_) => std::thread::yield_now()` while the head or body is pending. That spins a CPU
   for the whole network wait. Nothing calls them; `foundation_deployment/src/providers/common/api_types.rs:23`
   names `collect_exchange` only in a deprecation note.
2. `native/quic_listener.rs`: `QuicListener::accept_timeout` polls the driver and sleeps
   1 ms per loop; `accept()` passes a constant 30 s. Nothing outside the file uses
   `QuicListener` (check again: `rg -n 'QuicListener|quic_listener' backends examples bin crates`).

## Decision rule (same as spec 70 feature 38)

Code is kept only when a caller needs it. If T0 confirms there are still no callers (including
doc tests, examples and `pub use` re-exports that other crates use), **delete**:
`collect_exchange`, `drain_exchange_head` (and any helper used only by them), the
`quic_listener` module and its re-exports. Update the deprecation note in
`api_types.rs` to name `split_exchange` / `send_and_split` only. Update docs that name
the deleted items (`rg -n 'collect_exchange|QuicListener' backends documentation docs`).

If a caller exists, do not delete; instead:
- `collect_exchange`: replace `yield_now` with a park: register a wake on the observer
  (`WakeableStream::wake`, it is implemented for `SplitUntilObserverMap` and
  `SplitCollectorMapObserver`) that unparks the current thread, then
  `std::thread::park_timeout(config wait)`; this is a blocking boundary, so wrap the
  function in `blocking_boundary!` like other sync helpers.
- `accept_timeout`: park on the QUIC driver's UDP readiness (spec 70 feature 28 added it;
  find it in the h3 server accept path) with the deadline; the timeout becomes a config field.

## Tests

- Deletion: `cargo check --workspace --all-targets` and the netio and deployment test suites
  pass; `rg` shows no remaining reference.
- If kept: `collect_exchange_parks_while_waiting` (thread CPU time under 20 ms while the
  server delays the head 500 ms; use `libc::clock_gettime(CLOCK_THREAD_CPUTIME_ID)` as
  `foundation_core/tests/valtron/threaded_future.rs` does).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_netio --profile dev
python3 scripts/ci/ci.py run foundation_deployment --profile dev
rg -n 'yield_now' backends/foundation_netio/src            # no output
rg -n 'thread::sleep' backends/foundation_netio/src/native  # each hit has a comment saying why
```

## Done when

- [ ] T0 line (callers found or not). Deleted or fixed as the decision rule says.
- [ ] No `yield_now` in netio `src`. CI green.
