---
feature: "A bridged readiness waker reports when its future is gone"
status: "done"
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

- [x] `from_waker_with_liveness` exists and `is_live` reads the token.
- [x] `Cancelled` sets the token false on drop.
- [x] Both tests exist; the first fails on the old code.
- [x] The backlog §1 entry is deleted.
- [x] CI green.

## Result

2026-11-06. Implemented on `spec70/priority-followups`.

Files changed:

- `backends/foundation_core/src/valtron/readiness_wake.rs`: added the
  `live: Option<Arc<AtomicBool>>` field to `ReadinessWaker` and the
  `from_waker_with_liveness` constructor. `is_live()` reads the token for a
  bridged waker and keeps the `park` check for an executor waker. `from_waker`
  keeps its signature and passes `None`. Added the `#[doc(hidden)]`
  `ReadinessNotifier::waiter_count()` test hook.
- `backends/foundation_core/src/valtron/task.rs`: added the `#[doc(hidden)]`
  `ResumeSignal::waiter_count()` test hook, which delegates to the notifier.
- `backends/foundation_core/tests/valtron/bridged_waker_liveness.rs` (new) and
  `backends/foundation_core/tests/valtron/mod.rs` (registration).
- `backends/foundation_connectrpc/src/shared/context/cancel.rs`: `Cancelled`
  holds an `Arc<AtomicBool>` liveness token, sets it `false` in `Drop`, and
  registers through `from_waker_with_liveness`. Added the `#[doc(hidden)]`
  `CancelSignal::waiter_count()` test hook.
- `backends/foundation_connectrpc/tests/context_tests.rs`: added the bounded
  growth test.
- `backends/foundation_core/Cargo.toml`: version `0.0.11` to `0.0.12`. Every
  consumer requirement moved to `0.0.12` in the same change, and `Cargo.lock`
  was regenerated with `cargo metadata --format-version 1 --offline` (exit 0).
- `docs/bug-backlog.md`: deleted the §1 entry for `CancelSignal`.

Token ownership choice: the token belongs to one `Cancelled` future, not to the
`CancelSignal`. A token shared by all waits stays true while any wait lives, so
a dead waker would never be pruned. A per-wait token becomes false as soon as
that one wait is dropped, and the next `register` or `notify` on the signal
drops its waker. A repeated poll of the same future keeps the same `cx.waker()`,
so `will_wake` stops a second registration for it.

Test names and results:

- `valtron::bridged_waker_liveness::a_dead_bridged_waker_is_dropped`: ok.
  The test proves a live token holds the waiter (count 1), then proves that a
  dead token is pruned on the next register (count stays 1, not 2) and on
  `notify` (`has_waiters()` is false).
- `finished_cancel_waits_do_not_accumulate_wakers`: ok. It polls 1000 distinct
  bridged wakers through `CancelSignal::cancelled()` without firing, then
  asserts `waiter_count() <= 2`. On the old behaviour the count is 1000.

Command outcomes:

- `cargo fmt --all --check`: exit 0.
- `cargo clippy --profile uat -p foundation_core --all-targets --no-deps --features multi -- -D warnings`:
  finished, exit 0.
- `cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings`:
  finished, exit 0.
- `cargo test --profile uat -p foundation_core --features multi --test mod dead_bridged`:
  1 passed, 0 failed.
- `cargo test --profile uat -p foundation_core --features multi`: all suites
  passed; the `mod` suite ran 794 tests, 0 failed. Doc tests: 74 passed.
- `cargo test --profile uat -p foundation_connectrpc`: all suites passed, 0
  failed; 1 test ignored (pre-existing).
- `RUSTFLAGS='-D warnings' cargo check --profile uat --target wasm32-unknown-unknown -p foundation_connectrpc --no-default-features --features rpc`:
  finished, exit 0.
- `cargo metadata --format-version 1 --offline`: exit 0.

New bug recorded, not fixed (backlog §4): `ReadinessNotifier::register` skips a
push when `will_wake` matches, so two waits behind one task waker share one
stored waker and the stored token belongs to the older wait. If the older wait
drops first, `notify` prunes the registration and the newer wait never wakes.
No in-tree caller selects two cancellations at once; the likely fix is to
replace the stored waker with the newest on `will_wake`, plus a liveness state
shared by the waits behind one task waker.
