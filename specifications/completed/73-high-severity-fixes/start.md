---
workspace_name: "ewe_platform"
spec_directory: "specifications/73-high-severity-fixes"
this_file: "specifications/73-high-severity-fixes/start.md"
created: 2026-11-07
---

# Start: Spec 73, high-severity defect fixes

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** continuous integration (CI); pull request (PR); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

You implement **one feature** of this spec. The main session gave you its number.

## Read first, in this order

1. `CLAUDE.md` and `.agents/AGENTS.md` at the repository root.
2. `.agents/skills/rust-valtron-usage/skill.md` and `.agents/skills/rust-clean-code/skill.md`
   if they exist.
3. [`requirements.md`](./requirements.md): sections 4 (rules), 5 (verification) and 6 (done).
   They apply to every feature.
4. Your feature file: `features/NN-<slug>/feature.md`. Read all of it before you change code.
5. The GitHub issues your feature names. Read the full issue body.
6. [`progress.md`](./progress.md) and [`learnings.md`](./learnings.md).

## Workflow

1. Do not start until the main session hands off the feature. The feature file must hold the
   measured cause, the fix, the regression test and the verify commands before handoff.
2. Create your branch from `master`, named `spec73/fNN-<slug>`.
3. Do the tasks in the order the feature file gives. For each defect: write the regression
   test, see it fail on the old code, fix, see it pass.
4. Run the feature's verification commands exactly. Record the results.
5. Tick the tasks in your feature file. Add a short entry to `progress.md`. Add what you
   learned to `learnings.md`. Close the GitHub issue in the PR body (`Closes #NNN`).
6. Commit, push, report to the main session. Do not merge.

## Stop and report, do not guess

Stop and report to the main session when:

- the design in the feature file does not fit the code (give the file, the line and the
  reason);
- a fix needs a change outside the feature's named files that is larger than a few lines;
- a test fails and you cannot find the root cause in a reasonable time;
- the disk runs low and `cargo clean -p` on finished packages does not free enough.

## Lane order

The main session starts features in the order in `requirements.md` section 3. At most three
agents run at once. A feature that changes a shared crate waits until the previous feature in
that crate merges.

| Lane | Feature | Work | Crate |
| --- | --- | --- | --- |
| 1 | 01 gRPC status conversion | code | foundation_connectrpc |
| 2 | 02 client correctness | code | foundation_connectrpc |
| 3 | 03 handler streaming headers | code | foundation_connectrpc |
| 4 | 04 HTTP connection correctness | code | foundation_http, foundation_netio |
| 5 | 05 HTTP/1.1 worker parking | verify | foundation_http |
| 6 | 06 H3 WebTransport accept | verify | foundation_netio |
| 7 | 07 OpenAPI booleans | verify | foundation_openapi |
| 8 | 08 worktree targets | verify | workspace |
| 9 | 09 iOS gating | code | foundation_platform |
| 10 | 10 macOS VM SDK | CI | CI images |

"verify" means the defect is already fixed on master. Prove it, add a pinning test, and close
the issue with the evidence.
