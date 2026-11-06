---
feature: "A bridged readiness waker reports when its future is gone"
status: "pending"
lane: "priority follow-up (found under feature 138)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_core", "foundation_connectrpc"]
catalogue_rows: ["1", "4"]
---

# Feature 140: a bridged waker reports its liveness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **bridged waker**: a `ReadinessWaker` built from a standard `Waker` with
  `ReadinessWaker::from_waker`, for a source that sits behind a `Future`.
- **liveness**: whether the task that registered a waker still waits on it.
- **notifier**: `ReadinessNotifier`, the valtron object that keeps the wakers of one
  readiness and wakes them.

## Problem (found on 2026-11-06, backlog §1)

`ReadinessWaker::from_waker` (`backends/foundation_core/src/valtron/readiness_wake.rs:170`)
sets `park: None`, so `is_live()` always returns `true`. `ReadinessNotifier::register` and
`notify` drop wakers whose `is_live()` is false
(`readiness_wake.rs:229,245`). A bridged waker is therefore never dropped.

`CancelSignal::cancelled()` is one such bridge
(`backends/foundation_connectrpc/src/shared/context/cancel.rs`). Every facade wait on a
stream creates a fresh `cancelled()` future, and each poll registers a new bridged waker.
The signal fires once, at the end of the call, so a call that returns thousands of
messages keeps thousands of wakers until then. The old private vector had the same growth
plus one clone per poll; feature 138 did not change it.

## Design

1. `foundation_core`: let a bridged waker carry a liveness token.

   ```rust
   // readiness_wake.rs
   pub fn from_waker_with_liveness(waker: Waker, live: Arc<AtomicBool>) -> Self
   ```

   `ReadinessWaker` gains `live: Option<Arc<AtomicBool>>`; `is_live()` returns
   `live.load(Ordering::SeqCst)` for a bridged waker and keeps the `park` check for an
   executor waker. `from_waker` keeps its signature and passes `None`.

2. `foundation_connectrpc`: `Cancelled` holds `Arc<AtomicBool>`, set to `false` in `Drop`,
   and registers with `from_waker_with_liveness(cx.waker().clone(), Arc::clone(&self.live))`.
   A finished wait is then pruned on the next register or notify.

3. Limits stay configuration-free: the vector is pruned, not capped. The notifier's
   existing retain runs on every register and notify.

## Tests

1. `foundation_core/tests/valtron/`: `a_dead_bridged_waker_is_dropped` — register a
   bridged waker with a false token and assert the notifier holds no waiter
   (`has_waiters()` is false). Fails on the old code, where `is_live()` is always true.
2. `foundation_connectrpc/tests/client_deadline_tests.rs` or a unit-style test in
   `tests/`: a stream that receives N messages leaves at most a small constant number of
   registered wakers. This needs a counter; add one to `ReadinessNotifier` behind
   `#[doc(hidden)]` or expose `has_waiters` plus a test hook.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_core --test mod dead_bridged
cargo test --profile uat -p foundation_connectrpc
RUSTFLAGS='-D warnings' cargo check --profile uat --target wasm32-unknown-unknown \
  -p foundation_connectrpc --no-default-features --features rpc
```

Version: a new public method on `ReadinessWaker` moves `foundation_core` one patch step,
with every consumer requirement updated in the same change.

## Done when

- [ ] `from_waker_with_liveness` exists and `is_live` reads the token.
- [ ] `Cancelled` sets the token false on drop.
- [ ] Both tests exist; the first fails on the old code.
- [ ] The backlog §1 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Written while feature 138 landed the port change.
