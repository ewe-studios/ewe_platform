---
feature: "valtron owns executor wake-up: no crate outside valtron calls request_executor_resume or touches idle_wake; every wake goes through a valtron construct (ResumeSignal, an event queue, a readiness notifier) that wakes the executor itself"
status: "pending"
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

## Design

1. **The wake becomes private to valtron.** `request_executor_resume` and `idle_wake` become
   `pub(crate)` (keep a `#[doc(hidden)]` re-export only if feature 81's crates still need one
   during the change, and delete it when 81 lands). The compiler then rejects any outside call.
2. **Self-waking constructs.** Each one wakes the executor inside its own operation, so the
   caller never does:
   - `ResumeSignal` (exists): `set`/`turn_on` wakes. Use it for every flag a foreign thread flips.
   - `BoolSignal`: remove the raw `clone_inner` write path. Writers get a handle whose `set`
     wakes (or `BoolSignal` becomes `ResumeSignal`). Readers keep read access.
   - **Event queue** (new, valtron-owned, for example `valtron::EventQueue<T>`): wraps the
     concurrent queue; `push`, `force_push` and `close` wake when a consumer or producer is
     parked on it; `readiness()` and `vacancy()` return readiness with `needs_polling() == false`.
     Replace the hand-placed calls in `task.rs`, the extensions and `pipe.rs` with it, so those
     ~80 calls disappear. Feature 77's gating for `pipe.rs` (wake only when a readiness or
     vacancy was taken) moves inside the type.
   - **Readiness notifier** (new, for external event sources such as the reactor, fds, OS
     signals, shutdown): a valtron handle (for example `valtron::ReadinessNotifier`) that an
     event source holds and calls `notify()` on after it latches readiness. `foundation_reactor`
     and `foundation_nativeapis` use it instead of calling the wake.
3. **Delete the polling constructors over raw queues** (`QueueReadiness::new`,
   `QueueVacancyReadiness::new`) once lane A and lane B (feature 81) have moved to the event
   queue. Until 81 lands, mark them `#[deprecated]` with the replacement named.
4. **No outside wake in tests either.** Tests use the same constructs.
5. **Guard in CI.** Add a check (a small script in `scripts/ci/` run by `ci.py`) that fails if
   `request_executor_resume`, `resume_idle` or `idle_wake::` appear outside
   `backends/foundation_core/src/valtron/`.

Keep feature 75's, 77's and 46's guarantees: deadlines wake on time, idle workers use no
CPU, an async consumer of a parked `DrivenStreamIterator` never hangs, and a wake meant for
one consumer does not wake every thread (the 46+77 storm).

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

- [ ] No crate outside valtron calls the executor wake (compiler and CI guard); the ~80
  hand-placed calls are gone into the event queue; reactor and nativeapis use the notifier;
  lane A raw signal and queue sites converted; backlog entries deleted; CI green.
