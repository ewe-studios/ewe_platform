# Learnings — medium-severity defect fixes

_Seeded at spec creation. Updated after each feature PR. See
`.agents/skills/specifications-management/skill.md`._

## Feature 14 (2026-11-07)

- Bind the listener before the serve thread starts, then connect early. The kernel accepts a
  connect into the accept backlog, so the socket, not a timer, signals readiness. A fixed
  sleep after the spawn adds time and still races a slow machine.
- Not every sleep needs a readiness channel in its place. If no assertion needs to know that
  the loop started, and the first request has its own timeout, deleting the sleep is the
  whole fix. Add a channel only when a later assertion must observe the start.

## Feature 13 (2026-11-07)

- A shared read lock is not safe for every driver. Turso 0.8.3 keeps one `ConcurrentGuard`
  per connection and every statement clones it; `execute`, `step` and `query` call
  `try_use`. The guard is per statement, not per connection phase, so two reads collide
  exactly like a read and a write. Read the driver before you assume that a read can share.
- The smallest correct shape is serialization. A read takes the exclusive side and still
  reports `LockMode::Read`, so the timeout tells the caller which side it waited on. A
  connection pool is the parallel shape, and it is a larger change.
- A shared test macro needs a per-driver switch, not a second copy. `$reads_share` keeps the
  batch, rollback, timeout and ordering cases on both backends and branches only where the
  drivers differ.
- Two helpers that hide a sentinel from `fmt::Debug` (`AsyncQueryStream`, `AsyncListStream`)
  cannot use `expect_err`. Use `let Err(x) = .. else { panic!(..) }`.
- A one-connection store with serialized reads makes an 8-thread test slow, not flaky:
  `kv_take_tests` went from a few seconds to 36 s. Keep the timeout inside the test.

## CI follow-ups (2026-11-07)

- A crate version bump must update the example lock files (`examples/devbinary`,
  `examples/web/wello`, `examples/platform_android/src-tauri`) and the project templates in
  `bin/platform/templates`, or the plan step and `templates_generate_projects_that_build`
  fail. The templates patch crates.io to the local path, so the caret requirement must match
  the local version exactly: `0.0.16` does not select `0.0.17`.
- Several branches change the same template version lines. Merge one, rebase the next, and
  keep the highest crate version.
- Every merge moves master and re-conflicts every branch still open. One merged crate bump
  (macros 0.0.11, core 0.0.18) conflicts with 44 generated manifests at once, because each
  one lists both the bumped crate and the branch's crate on adjacent lines. Check
  `git merge-tree --write-tree origin/master origin/<branch>` after each merge; it is faster
  and more reliable than the GitHub `mergeable` field, which reports `UNKNOWN` while it
  recalculates.
- Resolve these manifest conflicts by crate key, not by whole hunk: parse each conflict block,
  key every dependency line by its crate name, and keep the line with the highest version.
  `--ours` and `--theirs` each lose one of the two bumps.
- A rebase can leave a requirement behind that no conflict touched: feature 06's new
  `foundation_core` dev-dependency kept `0.0.16` while master had moved to `0.0.18`, so
  `cargo metadata --locked` failed with "failed to select a version". Run the locked metadata
  check on the workspace and all three example projects after every rebase; it finds the
  stale line.
- The owner wants one PR per spec, not one per feature. The 13 feature PRs of this spec cost
  a rebase cycle for every merge, and a single spec PR would not have.
- A container job runs `run:` with `sh`, not bash. Name `shell: bash` before you use
  `pipefail` or a bash array.

## Feature 10 (2026-11-07)

- macOS reports an expired `SO_RCVTIMEO` as `WouldBlock`, so a helper that maps both cases to
  one error cannot report a timeout. A typed `ReadError` is the only way to keep the two
  apart, and the test needs the timeout to be visible.
- A progress assertion needs a synchronization point. `mpsc` ready channels replace the sleep
  and the absolute duration, and the test can then hold the slow uploads open for as long as
  it needs.
- The test found a real production defect: the Turso store rejects a second concurrent
  statement with "concurrent use forbidden" (#357, high). Two keychain workers are enough to
  hit it. The test serializes its uploads to stay green; the store still needs the fix.

## Feature 09 (2026-11-07)

- `mpp::Receiver` is a plain queue receiver with no readiness. A task that parks on a queue
  must own a readiness that the sender wakes. Adding `readiness()` to the receiver, and waking
  it in `send`/`force_send`/`close`, fixes every caller of the same shape.
- A queue that is pushed and never popped stays ready, so the executor re-runs the task in a
  hot loop. Drain with `try_iter` and let the readiness drop. A test that calls
  `next_status` by hand cannot see the loop; the test must run the task as the executor does.
- The sleeper defect was already fixed on master. Re-check a defect against master before you
  plan the change; the remaining work here was the test, not the clock.
- Two features that both bump `foundation_core` conflict on every version line (PR #339 sets
  0.0.17, PR #349 sets 0.0.18). Merge one, then rebase the other.

## Feature 04 (2026-11-07)

- There is no `Env` bundle and no `ContainerConfigError` in this workspace. The valtron `Ctx`
  is the bundle and `DockerError` is the crate's error type. Check the names before you write
  a design against them.
- Keeping the clock as `Option` is not a fallback. The value must still be present at start,
  and a missing one is a typed error. A required constructor argument would make that error
  unreachable.
- A macro that only accepts `()` functions cannot return an error. Add a `DockerResult<()>`
  arm and keep the `()` arm as a logged skip; both must be non-panicking.
- `is_pool_initialized()` followed by `fork_ctx()` is a check-then-use race, because the pool
  can be dropped between the calls (#346). A non-panicking `try_fork_ctx()` is the missing
  primitive.

## Feature 07 (2026-11-07)

- The syscall dispatch file carries its own permission conversion, so a mode fix is a
  parameter change, not a new helper. `handle_creat` already had the correct shape; copy it.
- `RLIMIT_NOFILE` belongs to the tracee, not the tracer. Read `/proc/<pid>/limits` for the
  tracee. A test that uses this process's pid passes while proving nothing.
- Some dispatch tests cannot run on an arm64 host under amd64 emulation: `PTRACE_GETREGS`
  returns `EIO` for the 13 spawn tests. Run the non-spawn subset locally and let native CI
  prove the rest.
- A fix can expose its own edge: `dup3` now reads `arg2`, so it must also reject flags other
  than `O_CLOEXEC` (#342).

## Feature 02 (2026-11-07)

- `PoolGuard` does not hold `PoolConfig`. A deadline computed at the pool guard must ask the
  registry for its configured timeout; do not thread the config through a type that never
  carried it.
- A test that proves an unbounded wait needs its own deadline. The old code waits 30 s, so the
  regression test times out on a channel instead of hanging the suite.
- Bounding one phase is not bounding the call. `shutdown_within` now limits the background
  registry, but the worker registry still receives the same full duration, so the total can
  reach twice the argument (#340). An absolute deadline computed once is the correct shape.

## Feature 05 (2026-11-07)

- Verify the premise of an issue before you design to it. #219 called `wasi_host.rs:41` an
  extern-C ABI boundary; it was a plain Rust helper. The fix is much smaller than the issue
  implies, and the ABI-argument shape was never needed.
- `foundation_wasm` keeps its timer state in process-global statics and has no host struct, so
  the host environment publishes its `Arc<dyn TimeSource>` into the module's existing install
  point. The installed source is written only by `WasiHost::install`; a missing one is a typed
  error, not a hidden global.
- A crate that was never clippy-clean on its own files blocks its verify command. Seven small
  lints had to be fixed in the crate before the feature could prove green; record them in the
  PR so they are not mistaken for scope creep.

## Feature 06 (2026-11-07)

- The tracker's real API is `connection_rate() -> u64`, not `current_rate() -> f64`. Read the
  type before you plan an `f64` fix.
- `LoadTrackerConfig::calculation_interval` was dead configuration that feature 132 had
  disabled. Reusing it as the minimum window, default `Duration::ZERO`, keeps the rule-8 shape
  without inventing a new field.
- Integer `u128` fixed-point math avoids the workspace's pedantic `cast_precision_loss` and
  `cast_sign_loss` lints that an `f64` rate trips.
- A clock-injected constructor is a breaking change even when no external crate calls it: every
  workspace manifest's `version` requirement moves with the crate. `rg` for real call sites
  first; the 67 manifest edits are bookkeeping.
- The same cleanup found a real soundness bug: `ConnectionGuard` held `*const LoadTracker` with
  `unsafe impl Send/Sync`. Borrowing the tracker removes the unsafe block.

## Feature 01 (2026-11-07)

- `Ctx::host_with_clock` does not exist. `Ctx::new(Arc<dyn Clock>)` is the constructor that
  installs an injected clock and the seeded RNG.
- A wasm per-request `ContextBag` must `fork()` the app's bag. A fresh `ContextBag` from
  `Ctx::host()` silently loses the injected clock and every init-time binding.
- `compati::Mutex::lock()` returns a `Result`, and `rand_core` 0.10 moves `next_u64` behind
  the `Rng` trait. Both cost a compile round when the feature file does not say so.
- The strict clippy command needs `--no-deps` because dev-dependencies reach
  `foundation_boringtun` (#330). `ci/packages.toml` already uses `--no-deps`.

## Feature 08 (2026-11-07)

- The `Skip`, `SkipError` and `CloseThenSkip` arms held three copies of the same
  setregs call. One helper removes the copies and gives all three arms the same
  error path.
- `ptrace::setregs` fails with `ESRCH` for a pid that is not traced. That failure
  gives a direct test without a child process.
- `truncate` and `ftruncate` are C entry points in the shim. The file's convention
  is `set_errno` plus `return -1`. `return -EINVAL` is wrong there.
- `usize::try_from(length.cast_unsigned())` hides a negative length. Check the sign
  first, then convert without `cast_unsigned`.
- The shim statics read the environment once. A direct test must set the variables
  before the first shim call, and its test target must hold no other test.
- The Linux CI script cannot run from a worktree. The bind-mounted `.git` file
  points at the host absolute path, so `link-submodules.nu` reports
  `fatal: not a git repository` before the build. Run the image from the main
  checkout, or use the GitHub CI.

## Feature 03 (2026-11-07)

- The panic was a bare `Option::expect` in the connectivity ladder. The clean fix is a
  dedicated error variant plus a documented `Disconnected` fallback, not a retry loop.
- A strict `cargo clippy -p <crate> --all-targets -D warnings` also lints workspace path
  dependencies. Seven pre-existing lints in `foundation_boringtun` stop the command, so the
  proof for this crate is `--no-deps` on its own targets plus the issue record (#330).

## Creation (2026-11-07)

- The retired backlog lived in two places, not one. The file was a stub, but the per-crate
  umbrella issues still carried unchecked checklists. The conversion found 76 pending defects
  behind 66 checkboxes. A retirement note is not the same as a completed migration: check the
  tracker's own bodies.
- Splitting a checklist item into nested defects can multiply the count. One `BytesPool` line
  held four. File one issue per independently fixable defect, or the close-out stays vague.
- 12 of the 76 split items are `severity: high`. They were latent inside umbrellas, so no one
  scheduled them. Label the split issues immediately; do not let a high defect hide in a
  tracking issue.

## Key finding (creation, 2026-11-07)

- The 17 medium issues are what spec 70 left open after its audit. They share one root
  pattern: a side effect (a clock, a rate, a wake) was left out of the injection seam, or a
  syscall dispatch path skipped a rule that the kernel applies.
- Six of them sit in one file, `foundation_nativeapis/src/native/vfs/ptrace/syscall_dispatch.rs`.
  Fixing them as one feature avoids four agents editing the same dispatch arms at once.
- Three fixes need a decision, not a patch: the WASI clock (an extern-C ABI has no place for
  a clock argument), the Docker container clock (a required parameter is a breaking change),
  and the toolings builder wake (a new readiness over an mpsc receiver). The feature files
  record the chosen direction.
