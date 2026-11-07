---
feature: "LoadTracker computes its connection rate"
status: "in-review"
priority: "medium"
depends_on: []
crates: ["foundation_netio"]
issues: [214]
model: "sonnet"
branch: "spec72/f06-netio-load-tracker-rate"
---

# Feature 06: Load tracker rate

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Make `LoadTracker::current_rate` report the real connection rate. Close issue
[#214](https://github.com/ewe-studios/ewe_platform/issues/214).

## 2. Problem

`backends/foundation_netio/src/shared/http/load_tracker.rs:119` sets `connection_rate_milli` to
0 and no code computes it. `current_rate` (`:150`) therefore always returns 0.0, so a
load-based timeout adjustment never sees load. The field `last_calculation` (`:101`) was an
unused sentinel; feature 132 replaced its clock read with an explicit 0 and left a comment.

## 3. Requirements

1. The tracker counts connections since `last_calculation` and divides by the elapsed
   monotonic time.
2. The result is stored in `connection_rate_milli`, in the existing unit.
3. The tracker holds an `Arc<dyn Clock>` or the crate's clock trait, like `LatencyTracker`
   does. The clock comes from the `Ctx` bundle (root rule 11).
4. The rate is computed when `track_connection` runs, or on read, whichever the existing code
   shape allows without a lock inversion.
5. A zero elapsed time returns the previous rate. Never a division by zero and never a NaN.

## 4. Design

- Add the clock field next to the existing counters.
- On each tracked connection, increment the counter. On read, compute
  `count_since_last / elapsed.as_secs_f64()`, then store it and reset the window.
- Keep the existing atomic representation. Document the unit in the field comment.
- The default window and the rate cap, if any, are configuration (root rule 8).

## 5. Tasks

- [x] Read the tracker and how the rate is consumed. The real API is
      `LoadTracker::connection_rate() -> u64`; there is no `current_rate`.
- [x] Add the clock and the window computation.
- [x] Add the regression test.
- [x] Run the verify commands. Record the results.
- [x] Close #214 in the PR body.

## 6. Tests

In `backends/foundation_netio/tests/`:

1. `the_connection_rate_is_not_always_zero`: with a simulated clock, track two connections
   across one simulated second and assert `current_rate` is 2.0 (or the documented unit). On
   the old code it is 0.0.
2. `a_zero_elapsed_window_returns_the_previous_rate`: track twice with no clock advance and
   assert no NaN and no panic.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_netio
cargo clippy --profile uat -p foundation_netio --all-targets -- -D warnings
cargo test --profile uat -p foundation_netio
```

## 8. Done when

- `current_rate` changes when connections arrive.
- The new test fails on the old code and passes on the new code.
- Issue #214 is closed by the PR.
- `progress.md` and `learnings.md` record the change.
