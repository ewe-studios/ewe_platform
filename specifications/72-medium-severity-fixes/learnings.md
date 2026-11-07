# Learnings — medium-severity defect fixes

_Seeded at spec creation. Updated after each feature PR. See
`.agents/skills/specifications-management/skill.md`._

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
