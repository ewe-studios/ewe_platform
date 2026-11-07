# Learnings — medium-severity defect fixes

_Seeded at spec creation. Updated after each feature PR. See
`.agents/skills/specifications-management/skill.md`._

## Creation (2026-11-07)

- The 17 medium issues are what spec 70 left open after its audit. They share one root
  pattern: a side effect (a clock, a rate, a wake) was left out of the injection seam, or a
  syscall dispatch path skipped a rule that the kernel applies.
- Six of them sit in one file, `foundation_nativeapis/src/native/vfs/ptrace/syscall_dispatch.rs`.
  Fixing them as one feature avoids four agents editing the same dispatch arms at once.
- Three fixes need a decision, not a patch: the WASI clock (an extern-C ABI has no place for
  a clock argument), the Docker container clock (a required parameter is a breaking change),
  and the toolings builder wake (a new readiness over an mpsc receiver). The feature files
  record the chosen direction.
