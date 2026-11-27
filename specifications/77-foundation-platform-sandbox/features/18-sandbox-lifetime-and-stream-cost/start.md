---
workspace_name: "ewe_platform"
spec_directory: "specifications/77-foundation-platform-sandbox"
feature_directory: "specifications/77-foundation-platform-sandbox/features/18-sandbox-lifetime-and-stream-cost"
this_file: "specifications/77-foundation-platform-sandbox/features/18-sandbox-lifetime-and-stream-cost/start.md"
created: 2026-11-27
---

# Start: Sandbox lifetime and stream cost Feature

## Read first

- `feature.md` (goal, story, rules, tasks and "done when").
- `../../research.md` (the Durable Streams study and the reason for each choice).
- `../../requirements.md` (laws, invariants and the feature order).
- `../../progress.md` and `../../learnings.md`.
- The feature files this feature builds on: `../07-cloudflare-control-plane/feature.md`
  (sections 6.6 and 6.10) and `../08-cloudflare-worker/feature.md`.
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

**Order:** Features 07 and 08 land first. The fakes of feature 07 are the test tools here.

---

_Created: 2026-11-27_
