# Learnings — medium-severity defect fixes

_Seeded at spec creation. Updated after each feature PR. See
`.agents/skills/specifications-management/skill.md`._

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
