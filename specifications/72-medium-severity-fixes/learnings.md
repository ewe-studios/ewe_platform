# Learnings — medium-severity defect fixes

_Seeded at spec creation. Updated after each feature PR. See
`.agents/skills/specifications-management/skill.md`._

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
