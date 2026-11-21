---
workspace_name: "ewe_platform"
spec_directory: "specifications/77-foundation-platform-sandbox"
feature_directory: "specifications/77-foundation-platform-sandbox/features/12-sandbox-images"
this_file: "specifications/77-foundation-platform-sandbox/features/12-sandbox-images/start.md"
created: 2026-11-14
---

# Start: Sandbox images Feature

## Read first

- `feature.md` (goal, story, rules, tasks and "done when").
- `../../requirements.md` (laws, invariants and the feature order).
- `../../progress.md` and `../../learnings.md`.
- The feature files this feature builds on: `../06-programs-rpc-provider/feature.md`, `../09-libkrun-driver/feature.md`.
- `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md`,
  `.agents/skills/rust-valtron-usage/skill.md`,
  `.agents/skills/rust-valtron-iterator/skill.md` and
  `.agents/skills/test-driven-development/skill.md`.

## Agent Workflow

1. Generate `compacted.md` with `.agents/skills/context-compaction/skill.md`.
2. Work on one task at a time. Write the test first.
3. Keep every limit in the configuration, as `feature.md` says. Stream every body.
4. Run the tests and verify commands that `feature.md` names.
5. Report to the Main Agent: the changed files, the commands you ran and their results.
6. After the commit, update `../../progress.md` and `../../learnings.md`.

**Order:** Features 06 and 09 land first.

---

_Created: 2026-11-14_
