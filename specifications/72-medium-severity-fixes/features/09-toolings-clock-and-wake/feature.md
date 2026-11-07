---
feature: "The toolings sleeper uses the injected clock and the builder task wakes"
status: "pending"
priority: "medium"
depends_on: []
crates: ["foundation_toolings"]
issues: [201, 203]
model: "sonnet"
branch: "spec72/f09-toolings-clock-and-wake"
---

# Feature 09: Toolings clock and wake

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Fix two defects in `foundation_toolings`: a sleeper that reads the wall clock, and a builder
task that never wakes and re-runs in a hot loop. Close issues
[#201](https://github.com/ewe-studios/ewe_platform/issues/201) and
[#203](https://github.com/ewe-studios/ewe_platform/issues/203).

## 2. Problem

1. **Clock.** `backends/foundation_toolings/src/runner/mod.rs` calls
   `std::time::SystemTime::now()` in `DurationSleeper::start` (`:36`) and
   `DurationSleeper::is_done` (`:49`). Root rule 11 requires the clock from the `Env` bundle
   in `Ctx`. The sleeper cannot be driven by a simulated clock, and
   `tests/integration.rs:116` depends on the real clock.
2. **Wake and queue.** `ProjectBuilderTask::next_status` parks on `change_queue` and
   `result_queue` readiness. Watch events arrive on `change_rx`, an `mpp::Receiver` with no
   readiness of its own, so nothing wakes the task when the watcher sends.
   `drain_watch_events` (`backends/foundation_toolings/src/builder/mod.rs:110`) pushes each
   event into `change_queue` but never pops it. Once one change is queued, the composite
   readiness stays ready and the executor re-runs the task in a hot loop. `tests/wake.rs`
   calls `next_status` by hand, so it does not see the missing wake.

## 3. Requirements

1. `DurationSleeper` reads a monotonic clock from `Ctx`/`Env` and stores a monotonic deadline.
   No `SystemTime::now()` remains.
2. The integration test uses a simulated clock.
3. A watch event wakes the builder task exactly once.
4. `change_queue` is popped when its content is consumed, so the composite readiness drops.
5. Two watch events produce two builds.
6. No more than one wake per event; a hot loop is a defect.

## 4. Design

- **Sleeper.** Add a clock field. Store `deadline = clock.now() + duration`. `is_done`
  compares `clock.now()` with the deadline.
- **Wake.** The preferred fix is a readiness over `change_rx`, or a subscriber on the watcher
  that pushes into `change_queue` through the queue's own wake path. Do not build a manual
  polling loop.
- **Queue.** Pop each entry when the task consumes it, so the readiness reflects only
  unprocessed events.
- Check whether `mpp::Receiver` already exposes a wakeable form; if it does not, add one in
  `foundation_core` as a small public readiness, with a test, and record it in `progress.md`.

## 5. Tasks

- [ ] Read `runner/mod.rs`, `builder/mod.rs` and the dev service call site
      (`service/mod.rs:59`).
- [ ] Move the sleeper to the injected clock.
- [ ] Fix the wake and pop the queue.
- [ ] Add the regression tests.
- [ ] Run the verify commands. Record the results.
- [ ] Close #201 and #203 in the PR body.

## 6. Tests

In `backends/foundation_toolings/tests/`:

1. `the_sleeper_follows_the_simulated_clock`: advance the simulated clock and assert
   `is_done` flips at the deadline. On the old code the test depends on wall time.
2. `two_watch_events_produce_two_builds`: send two events through the watcher and assert two
   builds. On the old code the second event is lost and the task hot-loops.
3. `the_task_reports_not_ready_after_it_drains_the_queue`: assert the composite readiness
   drops after the queue is popped.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_toolings
cargo clippy --profile uat -p foundation_toolings --all-targets -- -D warnings
cargo test --profile uat -p foundation_toolings
```

Run the wake test 20 times in a row.

## 8. Done when

- No `SystemTime::now()` remains in `foundation_toolings/src`.
- A watch event wakes the task once, and the queue empties as it is consumed.
- The regression tests failed on the old code.
- Issues #201 and #203 are closed by the PR.
- `progress.md` and `learnings.md` record the change.
