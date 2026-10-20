---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F09-build-verification-loop"
this_file: "specifications/62-macos-native-build/features/F09-build-verification-loop/start.md"
created: 2026-10-17
---

# Start: F09 — Build verification loop

## Agent Workflow

1. Read `feature.md` (the loop rules + at-risk surface).
2. Read `../../LEARNINGS.md` and `../../requirements.md` (Feature Index).
3. Read `.agents/AGENTS.md` and your agent doc in `.agents/agents/`.
4. Run the `uat` builds with output captured to a log (capture `$?` directly —
   do not pipe through `tail`).
5. For each new error: **file a new feature** in this spec (F10-…, same shape as
   F06), then fix it. One feature per distinct breakage.
6. Repeat until both packages build clean under `uat` and `dev`.
7. Update `../../LEARNINGS.md` and the Feature Index as features are added.

**Workflow:** feature.md → build → file new feature per error → fix → rebuild → learnings

_Created: 2026-10-17_
