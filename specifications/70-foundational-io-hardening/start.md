---
workspace_name: "ewe_platform"
spec_directory: "specifications/70-foundational-io-hardening"
this_file: "specifications/70-foundational-io-hardening/start.md"
created: 2026-10-28
---

# Start: Spec 70, foundational I/O hardening

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** input/output (I/O); pull request (PR); continuous integration
(CI); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

You implement **one feature** of this spec. The main session gave you its number.

## Read first, in this order

1. `CLAUDE.md` and `.agents/AGENTS.md` at the repository root. If `.agents/` is an empty
   submodule in your worktree, read `/home/user/ewe_platform/.agents/AGENTS.md` (read only).
2. `.agents/skills/rust-valtron-usage/` if it exists.
3. [`requirements.md`](./requirements.md): sections 2 (delegation model), 3 (rules) and
   4 (verification protocol). They apply to every feature.
4. Your feature file: `features/NN-<slug>/feature.md`. Read all of it before you change code.
5. The backlog entries your feature names, in `docs/bug-backlog.md`.
6. [`progress.md`](./progress.md) and [`learnings.md`](./learnings.md).

## Workflow

1. Create your branch from the base that `requirements.md` section 6 names.
2. Do the tasks in the order the feature file gives. For each bug: write the regression test,
   see it fail on the old code, fix, see it pass.
3. Run the feature's verification commands exactly. Record the results.
4. Tick the tasks in your feature file. Add a short entry to `progress.md`. Add what you learned
   to `learnings.md`. Delete the backlog entries you fixed.
5. Commit, push, open the PR. The PR body has: what changed, the before/after result of each
   regression test, the flake-check counts, the commands you ran and their results.
6. Report to the main session. Do not merge.

## Stop and report, do not guess

Stop and report to the main session when:
- the design in the feature file does not fit the code (give file, line and the reason);
- a fix needs a change outside the feature's named files that is larger than a few lines;
- a test fails and you cannot find the root cause in a reasonable time;
- the disk runs low and `cargo clean -p` on finished packages does not free enough.
