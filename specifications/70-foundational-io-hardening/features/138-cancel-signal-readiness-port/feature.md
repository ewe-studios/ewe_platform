---
feature: "The call cancel signal uses valtron's readiness port, not a private waker vector"
status: "done"
lane: "priority follow-up (found under the feature 129 valtron-surface audit)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_connectrpc"]
catalogue_rows: ["1"]
---

# Feature 138: the cancel signal uses the readiness port

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **surface**: the small set of public methods, traits and types that valtron provides.
- **port**: the part of the surface that a caller uses to cause an action. The caller
  reports a change; valtron decides how to wake.
- **waker**: the handle a waiting task gives to a source, so the source can make that task
  runnable again (`std::task::Waker`, `ReadinessWaker`).
- **readiness**: a value that answers "is this source ready?" and can register a waker
  (`EventReadiness`).

## Problem (found on 2026-11-06, backlog §1)

`CancelSignal` keeps its own waker list and calls `wake()` itself
(`backends/foundation_connectrpc/src/shared/context/cancel.rs:24-139`):

- `Inner` holds `Mutex<Vec<Waker>>`.
- `cancel()` takes the vector and calls `waker.wake()` on every entry (line 69).
- `Cancelled::poll` pushes `cx.waker().clone()` into the vector (line 131), on every poll.

This is hand-rolled wake plumbing outside valtron. It is not a leak of valtron internals:
no valtron type is used. But it does the job that the valtron surface already provides,
and it duplicates the logic that valtron owns:

- no de-duplication: each poll pushes a new clone, so many wakers accumulate for one
  waiter;
- no stale-waker cleanup after a waiter is dropped;
- the wake target and the wake policy live in `foundation_connectrpc` instead of valtron.

The valtron surface already has the port:

- `ResumeSignal` (`backends/foundation_core/src/valtron/task.rs:99`) is a flag plus a
  `ReadinessNotifier`. `set(true)` fires the notifier. It implements `EventReadiness`.
- `ReadinessNotifier` (`backends/foundation_core/src/valtron/readiness_wake.rs:202`) keeps
  one waker per task, drops wakers that are not live, and calls `wake()` inside valtron.
- `ReadinessWaker::from_waker` (`readiness_wake.rs:170`) is the documented bridge for a
  source that sits behind a `Future`: waking it wakes the future's waker.

## Design

1. Replace `Inner`:

   ```rust
   struct Inner {
       fired: ResumeSignal,
       /// Opt-in one-way-down link; `is_canceled` also consults the parent.
       parent: Option<CancelSignal>,
   }
   ```

2. `cancel()` sets the flag once:

   ```rust
   pub fn cancel(&self) {
       if !self.0.fired.get() {
           self.0.fired.set(true);
       }
   }
   ```

   `ResumeSignal::set(true)` fires the notifier. A second concurrent `cancel` can fire it
   again; a repeated wake is harmless because the future re-checks `is_canceled`. State
   this in a comment; do not add a private atomic for it.

3. `is_canceled()` reads `self.0.fired.get()`, then the parent, as now.

4. `register(&self, waker: &ReadinessWaker)` registers on the signal **and every
   ancestor**, through the trait method:

   ```rust
   fn register(&self, waker: &ReadinessWaker) {
       let _ = EventReadiness::register(&self.0.fired, waker);
       if let Some(parent) = &self.0.parent {
           parent.register(waker);
       }
   }
   ```

   The return value is ignored on purpose: `Cancelled::poll` re-checks `is_canceled()`
   after the registration, which is the register-then-recheck protocol.

5. `Cancelled::poll`:

   ```rust
   fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
       if self.signal.is_canceled() {
           return Poll::Ready(());
       }
       let waker = ReadinessWaker::from_waker(cx.waker().clone());
       self.signal.register(&waker);
       if self.signal.is_canceled() {
           Poll::Ready(())
       } else {
           Poll::Pending
       }
   }
   ```

6. Remove the `AtomicBool`, `Mutex`, `Waker` and `Ordering` imports. Update the module
   docs: the HOW paragraph describes the valtron port, not a parked-waker vector.

The public API does not change: `new`, `linked`, `cancel`, `is_canceled`, `cancelled`,
`Clone`, `Default` and `Debug` keep their signatures. `foundation_connectrpc` stays
0.0.6.

## Tests

`backends/foundation_connectrpc/tests/context_tests.rs` already holds the behavioural
guards (`cancel_fires_across_clones`, `with_cancellation_detaches`, the parent/child pair,
`cancelled_await_wakes_on_cross_clone_cancel`, `cancelled_await_wakes_via_parent_link`).
They must pass unchanged.

Add two:

1. `cancelled_await_wakes_every_waiter`: two threads each block on a clone's
   `cancelled()`. Cancel once. Both return. On the old code both also pass, so this is a
   guard for the notifier contract (it keeps a waker per task), not an old-code failure.
2. `cancel_twice_wakes_and_stays_canceled`: cancel twice; the second call must not panic
   and `is_canceled()` stays true.

This feature is a structural cleanup: no test fails on the old code. The proof of the
cleanup is the source: after the change the crate stores no `std::task::Waker` and calls
no `wake()`. Record that in the Result, with the grep output.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_connectrpc --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_connectrpc --test context_tests
cargo test --profile uat -p foundation_connectrpc
RUSTFLAGS='-D warnings' cargo check --profile uat --target wasm32-unknown-unknown \
  -p foundation_connectrpc --no-default-features --features rpc
scripts/ci/linux.sh foundation_connectrpc
```

## Done when

- [ ] `cancel.rs` holds no `Vec<Waker>` and calls no `wake()`.
- [ ] The parent/child wake path still works and is covered by a test.
- [ ] The two new tests exist.
- [ ] All checks above pass, including Linux CI.
- [ ] The backlog §1 entry is deleted.

## Result

2026-11-06. Done on branch `spec70/priority-followups`.

- `Inner` is now `{ fired: ResumeSignal, parent: Option<CancelSignal> }`. The
  `Mutex<Vec<Waker>>`, the `AtomicBool` and every direct `wake()` call are gone from
  `backends/foundation_connectrpc/src/shared/context/cancel.rs`.
- `cancel()` sets the flag through `ResumeSignal`, whose notifier wakes the tasks.
  `Cancelled::poll` registers `ReadinessWaker::from_waker(cx.waker().clone())` on the
  signal and every ancestor, then re-checks `is_canceled`.
- `grep -n "Vec<Waker>\|\.wake()\|Mutex" cancel.rs` returns nothing.
- Public API and crate version unchanged (`foundation_connectrpc` 0.0.6).
- Tests: the five existing cancel tests pass unchanged. Two new tests in
  `tests/context_tests.rs`: `cancelled_await_wakes_every_waiter` (two blocked waiters, one
  cancel, both return) and `cancel_twice_stays_canceled`. `context_tests` 10 passed; the
  full crate 253 passed, 0 failed, 1 ignored.
- No test fails on the old code: this feature is a structural cleanup, as the plan states.
  The proof is the source, not a red test.
- `cargo fmt --all --check`, clippy (all targets, `-D warnings`) and the wasm32 rpc check
  (`RUSTFLAGS=-D warnings`) are clean.
- Linux CI: pending the push; the pull request proof table is updated with the result.
