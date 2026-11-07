---
feature: "valtron owns executor wake-up: the executor registers a per-task waker with each readiness (Waker pattern); sources wake that waker, never an executor function; the wake functions are private to valtron"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_core", "foundation_reactor", "foundation_nativeapis", "foundation_netio", "foundation_http", "foundation_proxy"]
catalogue_rows: [104, 105, 106]
---

# Feature 80: valtron owns executor wake-up

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Owner rule (2026-11-04)

Logic that wakes the executor is encapsulated in valtron. Users work with tasks, `ResumeSignal`
and the other constructs valtron gives, and the wake happens on its own. Executor wake must not
leak into other crates. Every place where it leaks is redesigned so that a valtron construct
does the wake.

## Where it leaks today (audit, lane A head 67e022f54 + feature 46)

- **Other crates call the executor wake directly:** `foundation_reactor` `src/reactor.rs` (2),
  `src/shutdown_signal.rs` (1); `foundation_nativeapis` `src/native/fd/mod.rs` (1),
  `src/valtron/stop_signal.rs` (1), `tests/fd_event_driven_tests.rs` (1).
- **Inside valtron the wake is hand-placed at every producer:** about 80
  `request_executor_resume()` calls after queue pushes and closes in `valtron/task.rs` (16),
  `extensions/tasks/{sendable,non_sendable}.rs` (19 + 19),
  `extensions/streams/{sendable,non_sendable}.rs` (12 + 12), `pipe.rs` (1). A producer that
  forgets one is a lost wake (feature 75's stall and the SSE stall on PR #173 are this class).
- **Raw signal and queue access that cannot wake:** `BoolSignal` with `clone_inner`, and
  `QueueReadiness::new` / `QueueVacancyReadiness::new` over a plain queue. These keep the 1 ms
  re-check because nothing wakes on change. Lane A sites: `foundation_http`
  `native/server/h2_connection.rs`, `foundation_nativeapis` `signal/task.rs`, the
  `task_iters.rs` and extension vacancy sides. Lane B sites are feature 81.

## Design: valtron hands a waker to the readiness, the readiness never calls valtron

The rule is inverted from "whoever flips the state must wake the executor" to "the executor
asks the readiness to wake it". This is the `core::task::Waker` pattern that futures already
use, applied to `EventReadiness`.

1. **`EventReadiness` gains a registration method** (name final in the PR), for example
   `fn register(&self, waker: &ReadinessWaker) -> bool`:
   - When a task parks on a readiness, valtron calls `register` with a waker that belongs to
     **that task** (it is built on `core::task::Waker`, so it works in `no_std` and on wasm).
   - The readiness stores the waker and calls `waker.wake()` when it becomes ready (or is
     closed). That is the only thing a source ever does; it never calls an executor function.
   - `register` returns `true` if the readiness will wake on change. Then valtron does not
     re-check it on a timer: this replaces `needs_polling()`, which goes away.
   - The default returns `false`, so a readiness that cannot wake is still polled (safe), and
     custom implementations outside valtron keep working until they opt in.
   - A re-register replaces the stored waker (`Waker::will_wake` avoids clones).
2. **The waker wakes one task, not every thread.** `ReadinessWaker::wake` marks that task
   runnable in its own executor (the worker or the single-thread engine that parked it) and
   unparks only that executor's thread. It does not call the process-wide `resume_idle`.
   This removes the class of bug in the 46+77 merge (a wake for one consumer interrupted every
   idle thread and made a parked task re-check ~3000 times). Combinators forward it:
   `AnyReadiness` registers the same waker with every child.
3. **valtron's own readiness types all register** and so need no help from callers:
   `ResumeSignal`, `WakeSignal`, the deadline readiness (the timer thread holds the waker),
   `AnyReadiness`, and a new valtron-owned **event queue** whose `push` / `close` and
   pop (for vacancy) wake the registered waker. The ~80 hand-placed executor wakes in
   `task.rs`, the task and stream extensions and `pipe.rs` disappear into the queue type.
   `BoolSignal` loses its raw `clone_inner` write path (writers get a handle whose `set` wakes
   the registered waker), or is folded into `ResumeSignal`.
4. **Outside event sources implement `register`, they do not call valtron:**
   - `foundation_reactor`: keep the waker per registered fd; wake it when the fd latches ready.
     Its shutdown signal does the same.
   - `foundation_nativeapis`: `FdRegistration` / `RegisteredFd` (reactor path), `StopSignal`,
     the OS signal handler (`signal/task.rs`).
   - `foundation_netio`, `foundation_http`: their connection and body readiness types; the
     http `AsyncReadiness` forwards `register`.
   For a source that only knows "something changed somewhere" (no per-item waker), valtron
   offers one construct: a `ReadinessSource`/notifier that the source holds and `notify()`s;
   it fans out to the wakers registered on it. Use it only where a per-item waker is not
   possible, and say why in a comment.
5. **The executor wake functions become private to valtron.** `request_executor_resume`,
   `resume_idle` and `idle_wake` are `pub(crate)`. If a case is truly unavoidable (for example
   the single-thread wasm engine handing control back to the JS event loop, or a reactor edge
   case that cannot hold a waker), keep one narrowly named public function for it, document
   the case on it, and list every caller in this feature's report. The default answer is "no
   such case".
6. **Feature 46's resume listener and timer** become ordinary registrations: the
   `DrivenStreamIterator` consumer's waker is registered with the parked tasks' readiness (and
   the deadline timer), not with a global listener list.
7. **Guard in CI**: a check in `scripts/ci/` (run by `ci.py`) fails if the executor wake
   functions appear outside `backends/foundation_core/src/valtron/`, except a listed,
   justified exception from step 5.

Keep the guarantees of features 75, 77 and 46: deadlines wake on time; idle workers use no
CPU; an async consumer of a parked `DrivenStreamIterator` never hangs.

## Order of work

E1 the `register` API, `ReadinessWaker` and the per-task targeted wake in both executors, with
valtron's own readiness types converted (`needs_polling` removed). E2 the event queue, and
the ~80 hand-placed wakes removed. E3 reactor, nativeapis, netio, http, proxy implement
`register`. E4 make the wake functions private, add the CI guard, deprecate the raw-queue
readiness constructors for feature 81. Commit and push after each.

## Tests (`backends/foundation_core/tests/valtron/`, both feature sets)

- Per construct (`ResumeSignal`, the event queue push and close, its vacancy, the readiness
  notifier): a task parked on it from another thread wakes, with the feature 77 check count
  bound; nothing outside valtron calls the wake.
- The ~80 sites: the existing extension, stream and task tests still pass; add one test per
  producer kind (observer push, close, pipe send) that would hang if the queue did not wake.
- `foundation_reactor` and `foundation_nativeapis`: their fd and stop-signal tests pass using
  the notifier.
- The CI guard fails on a planted outside call (test the script).

## Verify

Focused local checks (requirements.md §8.2 rule 4), 20 runs of each new timing test, full
`foundation_core --test mod` in both feature sets 5 times. GitHub CI on the PR runs the rest.

## Done when

- [x] `EventReadiness::register` with a per-task targeted waker in both executors; `needs_polling` removed; valtron's readiness types and the event queue self-wake; the ~80 hand-placed wakes gone; reactor, nativeapis, netio and http implement `register`; the wake functions are `pub(crate)` (any exception listed and justified); CI guard; the 46+77 storm cannot recur (test); backlog entries deleted; CI green.
