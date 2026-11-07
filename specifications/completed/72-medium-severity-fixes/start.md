---
workspace_name: "ewe_platform"
spec_directory: "specifications/72-medium-severity-fixes"
this_file: "specifications/72-medium-severity-fixes/start.md"
created: 2026-11-07
---

# Start: Spec 72, medium-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** pull request (PR); continuous integration (CI); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

You implement **one feature** of this spec. The main session gave you its number.

## Read first, in this order

1. `CLAUDE.md` and `.agents/AGENTS.md` at the repository root.
2. `.agents/skills/rust-valtron-usage/skill.md` and `.agents/skills/rust-clean-code/skill.md`
   if they exist.
3. [`requirements.md`](./requirements.md): sections 3 (rules), 4 (verification) and 6 (done).
   They apply to every feature.
4. Your feature file: `features/NN-<slug>/feature.md`. Read all of it before you change code.
5. The GitHub issues your feature names. Read the full issue body.
6. [`progress.md`](./progress.md) and [`learnings.md`](./learnings.md).

## Workflow

1. Create your branch from `master`, named `spec72/fNN-<slug>`.
2. Do the tasks in the order the feature file gives. For each defect: write the regression
   test, see it fail on the old code, fix, see it pass.
3. Run the feature's verification commands exactly. Record the results.
4. Tick the tasks in your feature file. Add a short entry to `progress.md`. Add what you
   learned to `learnings.md`. Close the GitHub issue in the PR body (`Closes #NNN`).
5. Commit, push, open the PR. The PR body has: what changed, the before/after result of each
   regression test, the flake-check counts, and the commands you ran with their results.
6. Report to the main session. Do not merge.

## Stop and report, do not guess

Stop and report to the main session when:

- the design in the feature file does not fit the code (give the file, the line and the
  reason);
- a fix needs a change outside the feature's named files that is larger than a few lines;
- a test fails and you cannot find the root cause in a reasonable time;
- the disk runs low and `cargo clean -p` on finished packages does not free enough.
