---
feature: "The executor names a poll that blocks a worker for too long"
status: "done"
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

- [x] `PoolConfig::slow_poll_warning` exists with a documented default and builder.
- [x] The three tests exist; the first fails on the old code.
- [x] The macro accepts `slow_poll` and defaults the test harness to 100 ms.
- [x] The rule is in the valtron skill and the macro docs.
- [x] The backlog §4 entry is deleted.
- [ ] CI green. (Pending the main session's Linux gate and GitHub run.)

## Result

2026-11-06. Landed on `spec70/priority-followups`.

What changed:

- `PoolConfig::slow_poll_warning: Option<std::time::Duration>` and
  `with_slow_poll_warning` in
  `backends/foundation_core/src/valtron/executors/multi/mod.rs`. The default is
  `None`; the doc comment gives the reason (two clock reads per poll) and the
  100 ms test default.
- `ExecutorState` reads the threshold in `do_work`
  (`executors/local.rs`). This is the one place a pool worker invokes a task
  step (`iter.next(top_entry, engine, ctx)`), so a blocking call in any inner
  poll is measured. Two `foundation_compact::Instant` reads wrap the step only
  when the threshold is `Some`.
- `warn_slow_poll` emits
  `tracing::warn!(target: "valtron::executor", task = %name, micros = <elapsed microseconds>, "a task poll blocked a worker")`.
  `micros` is `u64`. A per-entry `slow_poll_last_warn` map limits one task to
  one warning per second, and the entry is removed with the task.
- `ExecutionIterator::name()` is a provided method. The default returns
  `core::any::type_name::<Self>()`; the forwarding impls (`Box`, `&mut`,
  `RefCell`, `Rc`, `Mutex`, `Arc`, `CanCloneExecutionIterator`) forward it, so
  the boxed drive point names the concrete task type.
- `#[valtron]` and `#[valtron_test]` accept `slow_poll = <ms>` and default to
  100 ms. They call `initialize_pool_with_slow_poll_warning` and
  `initialize_current_thread_with_slow_poll_warning`, which exist in both the
  multi and single builds. The key is documented beside `threads` and `timeout`,
  with the "a task poll must not block" rule.
- The rule is in `.agents/skills/rust-valtron-iterator/skill.md` and in the
  `#[valtron_test]` docs.
- The backlog section 4 entry is deleted. Feature 139 closes it: the threshold,
  the task name, the test-harness default and the rule all landed.
- Tests in `backends/foundation_core/tests/valtron/slow_poll_warning.rs`,
  registered in `tests/valtron/mod.rs`:
  `a_slow_poll_is_reported`, `a_fast_poll_is_silent` and
  `the_warning_repeats_at_most_once_per_second`. They use a scoped
  `tracing_subscriber` layer; no generic field-capture helper existed, so the
  file holds one. `a_slow_poll_is_reported` does not compile on the old code
  (no `with_slow_poll_warning`). A fourth test,
  `valtron_macro_tests::slow_poll_argument_is_accepted`, proves the macro key
  parses and runs.

Verify on macOS, profile `uat`:

| Command | Real outcome |
|---------|--------------|
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --profile uat -p foundation_core --all-targets --no-deps -- -D warnings` | Passed (default features). |
| `cargo clippy --profile uat -p foundation_core --features multi --all-targets --no-deps -- -D warnings` | Passed. |
| `cargo clippy --profile uat -p foundation_core -p foundation_macros --all-targets --no-deps -- -D warnings` | Failed in `foundation_macros`: the 62 pre-existing pedantic warnings recorded in `docs/bug-backlog.md` section 30 and scheduled as feature 120. This change adds none; `foundation_core` passes alone in both feature trees. |
| `cargo test --profile uat -p foundation_core --test mod slow_poll` | 0 tests matched: these tests are `#[cfg(feature = "multi")]` and the default test build has no `multi`. |
| `cargo test --profile uat -p foundation_core --features multi --test mod slow_poll` | 4 passed, 0 failed (`a_slow_poll_is_reported`, `a_fast_poll_is_silent`, `the_warning_repeats_at_most_once_per_second`, `slow_poll_argument_is_accepted`). |
| `cargo test --profile uat -p foundation_core` | All targets passed; the `mod` target ran 749 tests, 0 failed. |
| `cargo test --profile uat -p foundation_core --features multi` | All targets passed; the `mod` target ran 793 tests, 0 failed on two later full runs. The first full run timed out `targeted_wake::a_wake_for_another_task_does_not_make_a_parked_task_re_check` at its 20 s bound; the test passes alone, so it is a timing flake under full-suite load. |
| `cargo test --profile uat -p foundation_macros` | All targets passed (33 + 54 + 10 + 1 passed, 40 ignored). |

Not run here:

- `scripts/ci/linux.sh foundation_core`: the Linux Docker gate runs after the
  commit in the main session.
- The version bump for the new public API (rule 12). It needs every workspace
  consumer's `foundation_core` requirement to move in the same change, which is
  outside this feature's crate ownership. The main session must schedule it.

Follow-ups found:

- `.agents` is a git submodule. The skill rule is an uncommitted change inside
  it; commit it in the submodule and bump the gitlink with this feature.
- The `targeted_wake` timing flake under full-suite load (see above).
- `foundation_macros` clippy debt (feature 120) blocks any `-D warnings` run
  that includes that crate.

