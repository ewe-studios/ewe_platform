---
feature: "Event-driven readiness follow-ups from feature 75: find the 20 ms/s idle loop, move the remaining polling signals to ResumeSignal, nativeapis FdRegistration resumes the executor, and tests that fail on the old 1 ms polling"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_core", "foundation_netio", "foundation_http", "foundation_nativeapis"]
catalogue_rows: [95, 96, 97, 98]
---

# Feature 77: event-driven readiness follow-ups

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 75 (2026-11-04). Feature 75 added `EventReadiness::needs_polling()`,
`ResumeSignal` and `request_executor_resume`, and stopped the 1 ms re-check for signals that
resume the executor. These items finish that work. Their backlog entries are in `docs/bug-backlog.md` §9.

## E1. Tests that fail on the old 1 ms polling (do this first)

`task_parked_on_deadline_readiness_wakes_on_time` and
`any_readiness_wakes_at_the_earliest_child_deadline` pass on the old executor, because the old
worker re-checked readiness every 1 ms and woke on time. Make them discriminate:
- Count the readiness checks. Wrap the readiness under test in a counting adapter (a test
  type in `tests/`, public trait only) that counts `is_ready()` / poll calls while the task is
  parked. Assert a small bound (for example ≤ 5 checks over a 300 ms park). The old code
  makes about 300.
- Do this in both feature sets (default and `multi`). This replaces the process-wide CPU
  measure, which is noisy when other tests run in parallel.
- Prove it: with `needs_polling()` forced to `true` (a test-only readiness that returns
  `true`), the same test must fail. Write the before and after numbers in the report.

## E2. A `multi` worker parked on a `ResumeSignal` still uses about 20 ms of CPU per second

`worker_with_event_driven_sleeper_uses_less_cpu_than_a_polling_one` measured 21 to 31 ms/s
(event-driven) against 51 to 66 ms/s (polling). An idle worker must use about none.
- Find the loop: sample `/proc/self/task/<tid>/schedstat` per thread, and count the wake-ups
  of the worker loop (a debug counter or a `tracing` span) during a 1 s park. Likely causes to
  check: the idle back-off (`SpinWait`) arm, the `max_yield_duration` wake-up, a
  `thread::yield_now` loop, the shared queue's condvar timeout.
- Fix the cause. Each wake-up period that stays is executor configuration with a builder and
  a documented default (CLAUDE.md rule 8), not a `const`.
- Test: the same per-thread measure asserts under 2 ms of CPU per second for an idle worker
  parked on a `ResumeSignal`, and the count of loop wake-ups stays under a small bound.

## E3. `BoolSignal` and the queue readiness types still poll

`foundation_http` `connection.rs` and `foundation_netio` `native/connection.rs` store into a
`BoolSignal` through `clone_inner` without `request_executor_resume`. Other code pushes to a
raw queue.
- Replace those raw stores with `ResumeSignal` (or a `set` method that resumes the
  executor). Then mark the queue readiness types `needs_polling() == false`, with a resume
  call on every push.
- Grep the workspace (lane A crates) for every `clone_inner` / raw store into a readiness
  flag and convert each one. List them in the report.
- Test: per type, a task parked on it wakes after a store from another thread, and the
  readiness check count from E1 stays small.

## E4. `FdRegistration` and `RegisteredFd` (`foundation_nativeapis`) still poll

The reactor latches readiness without `request_executor_resume`, and the legacy path polls
with a syscall.
- Call `request_executor_resume` after the latch. Then return `false` from `needs_polling`
  when the registration is on the reactor path. The legacy syscall path keeps `true`.
- Test: a task parked on an fd registration wakes when the fd becomes readable (a pipe or
  socketpair write from another thread), with a small check count.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p foundation_core --profile dev --test <the readiness test target>
cargo test -p foundation_core --profile dev --features multi --test <the readiness test target>
cargo test -p foundation_nativeapis --profile dev
cargo clippy -p foundation_core -p foundation_netio -p foundation_http -p foundation_nativeapis --profile dev --all-targets -- -D warnings
```
Run each new test 20 times. The full CI runs on GitHub (see requirements.md §8.2 rule 9); do not run `ci.py` locally.

## Done when

- [ ] E1 tests fail with polling forced on and pass without it; E2 idle worker under 2 ms/s; E3 and E4 types event-driven with tests; the four backlog entries deleted; GitHub CI green.
