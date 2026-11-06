---
feature: "The executor names a poll that blocks a worker for too long"
status: "pending"
lane: "priority follow-up (found under the feature 131 CI failure)"
model: "sonnet"
branch: "spec70/priority-followups"
crates: ["foundation_core", "foundation_macros"]
catalogue_rows: ["4"]
---

# Feature 139: a slow poll is named

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **poll**: one call of `Future::poll` or `Stream::poll_next` for a task. The executor
  worker runs it.
- **worker**: a valtron pool thread that polls tasks.
- **blocking call**: a function that parks the calling thread, for example `TcpStream::read`
  on a blocking socket, `File::read` or `std::thread::JoinHandle::join`.
- **watchdog**: a check that measures how long an operation takes and reports the slow ones.

## Problem (found on 2026-11-06, backlog §4)

A blocking call inside an async body blocks the valtron worker that polls that body.
Nothing measures the poll, so the pool is starved silently. The effect depends on the pool
size, which makes it look like a flaky test:

- CI runs `cargo test` with `--test-threads=2` on a 2-vCPU machine, so the multi pool
  starts two task workers. Two test bodies that block in `join()` hold both workers. The
  H2 pump task never runs, the client never closes its socket, and the peer's 2 s bound
  expires.
- A macOS run has many workers, so the blocked ones are not noticed.

Evidence, 2026-11-06:

- CI run `42175894858` failed
  `client_deadline_tests::a_timed_out_receive_cancels_the_call` twice, at
  `tests/client_deadline_tests.rs:821`.
- The same binary, with `#[valtron_test(threads = 1)]` and `--test-threads=2`, failed 3 of
  3 local runs with the same panic. With `threads = 2` or a single test thread it passed.
- Feature 131 now waits on the peer without blocking a worker
  (`tests/client_deadline_tests.rs`, `join_within`). The hazard remains for every other
  test and for production code: nothing reports it.

The executor has no slow-poll detection. `FutureTask` polls at
`backends/foundation_core/src/valtron/executors/future_task.rs:268,325,398,437,841,935`
and `StateTask` at `state_task.rs:171,224`. None of them measures the elapsed time.

## Design

1. Add a field to `PoolConfig`
   (`backends/foundation_core/src/valtron/executors/multi/mod.rs:297`):

   ```rust
   /// Warn when one poll takes longer than this. `None` disables the check.
   pub slow_poll_warning: Option<std::time::Duration>,
   ```

   Builder: `with_slow_poll_warning(Option<Duration>)`. Default `None` in the library:
   the check costs two clock reads per poll, and valtron's steady state polls in
   microseconds. State this reason in the doc comment. The test harness turns it on (item
   4).

2. One private helper in the executor wraps every poll:

   ```rust
   fn poll_timed<F: FnOnce() -> Poll<T>, T>(&self, poll: F) -> Poll<T>
   ```

   When the field is `None` it calls the closure directly. Otherwise it reads the
   monotonic clock before and after, and on a poll over the threshold emits
   `tracing::warn!(target: "valtron::executor", task = %name, micros = elapsed, "a task poll blocked a worker")`
   at most once per task per second (a small `last_warn` field on the task) so a blocked
   poll on every turn does not flood the log. The timer uses the same monotonic source the
   executor already reads for its idle wait. Do not add a second clock backend.

3. `TaskStatus` and the public task API do not change.

4. The `#[valtron_test]` and `#[valtron]` macros set the test default
   (`slow_poll_warning: Some(Duration::from_millis(100))`) unless the attribute names its
   own value. 100 ms is ten times the executor's 10 ms readiness quantum and well under
   the 2 s peer bound that the 2026-11-06 failure hit. Add `slow_poll = <ms>` to the macro
   keys, documented beside `threads` and `timeout` in
   `backends/foundation_macros/src/valtron_entry.rs`.

5. Documentation: add one rule to
   `.agents/skills/rust-valtron-iterator/skill.md` and to the `#[valtron_test]` docs — a
   task poll must not block; wait with a readiness, a timer or an async equivalent, never
   with `join()`, `recv()` on a blocking socket or a blocking file call.

## Tests

In `backends/foundation_core/tests/valtron/` (register in `tests/valtron/mod.rs`):

1. `a_slow_poll_is_reported`: a pool with `slow_poll_warning: Some(10 ms)` runs a task
   whose first poll sleeps 50 ms once, then returns `Ready`. Capture `tracing` output (the
   helper from feature 119 is in the tree) and assert the warning names the task and
   reports at least 50 ms. This test fails on the old code, which has no field and no
   warning (it does not compile against the old config).
2. `a_fast_poll_is_silent`: the same pool runs a task that polls quickly 100 times; assert
   no warning.
3. `the_warning_repeats_at_most_once_per_second`: a task that blocks 50 ms on every poll
   for 300 ms produces at most two warnings.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core -p foundation_macros --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_core --test mod slow_poll
cargo test --profile uat -p foundation_core
cargo test --profile uat -p foundation_connectrpc
scripts/ci/linux.sh foundation_core
```

## Done when

- [ ] `PoolConfig::slow_poll_warning` exists with a documented default and builder.
- [ ] The three tests exist; the first fails on the old code.
- [ ] The macro accepts `slow_poll` and defaults the test harness to 100 ms.
- [ ] The rule is in the valtron skill and the macro docs.
- [ ] The backlog §4 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Written from the feature 131 CI failure.
