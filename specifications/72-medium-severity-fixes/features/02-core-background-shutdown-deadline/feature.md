---
feature: "shutdown_within bounds the background job registry"
status: "pending"
priority: "medium"
depends_on: []
crates: ["foundation_core"]
issues: [233]
model: "sonnet"
branch: "spec72/f02-core-background-shutdown-deadline"
---

# Feature 02: Bounded background shutdown

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. Goal

Make `PoolGuard::shutdown_within(timeout)` bound the whole shutdown, including the background
job registry. Close issue
[#233](https://github.com/ewe-studios/ewe_platform/issues/233).

## 2. Problem

`shutdown_within` bounds the worker registry. `BackgroundJobRegistry::shutdown` uses its own
configured timeout, which defaults to 30 s. A stuck background job can hold the pool lifecycle
gate up to 30 s past `DEFAULT_POOL_SHUTDOWN_GRACE`.

File: `backends/foundation_core/src/valtron/executors/multi/mod.rs`, `shutdown_within`.

## 3. Requirements

1. The background registry receives a deadline, not only a duration.
2. The deadline is `min(timeout, config.background_shutdown_timeout)` measured from the call.
3. The registry checks the deadline between jobs. It does not wait for a job that already
   passed the deadline.
4. The configured 30 s default stays for the path that calls `shutdown` alone.
5. Both values are configuration with documented defaults (root rule 8).

## 4. Design

Change the registry shutdown entry point to accept an `Instant` deadline, or pass the effective
duration computed at the call site. Keep the existing `shutdown()` as a wrapper that computes
its own deadline from the configured timeout, so a caller that does not bound it keeps today's
behaviour.

The pool path computes `let deadline = clock.now() + min(timeout, cfg.background_shutdown_timeout)`
and passes it down. The clock comes from the injected bundle (root rule 11).

## 5. Tasks

- [ ] Read `multi/mod.rs::shutdown_within` and `BackgroundJobRegistry::shutdown`. Record the
      current signature and the timeout fields.
- [ ] Add the deadline parameter and the `min` computation.
- [ ] Keep `shutdown()` working with the configured timeout.
- [ ] Add the regression test.
- [ ] Run the verify commands. Record the results.
- [ ] Close #233 in the PR body.

## 6. Tests

In `backends/foundation_core/tests/`:

1. `shutdown_within_bounds_a_stuck_background_job`: start a background job that blocks on a
   channel, call `shutdown_within(50 ms)`, and assert the call returns in less than 500 ms
   using the simulated or monotonic clock. On the old code it waits for the configured
   timeout. If the default is 30 s the test hangs, so give the test its own timeout.
2. `background_shutdown_alone_keeps_its_timeout`: the plain `shutdown()` still uses the
   configured value.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_core
cargo clippy --profile uat -p foundation_core --all-targets -- -D warnings
cargo test --profile uat -p foundation_core
```

Also run the tests of the crates that depend on the changed API:
`cargo tree -i foundation_core --workspace --depth 1`.

## 8. Done when

- `shutdown_within` never waits past its argument because of a background job.
- The new test fails on the old code and passes on the new code.
- Issue #233 is closed by the PR.
- `progress.md` and `learnings.md` record the change.
