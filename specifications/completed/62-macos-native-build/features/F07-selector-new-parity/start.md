---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F07-selector-new-parity"
this_file: "specifications/62-macos-native-build/features/F07-selector-new-parity/start.md"
created: 2026-10-17
---

# Start: F07 — Selector::new() parity

## Agent Workflow

1. Read `feature.md`.
2. Read `../../LEARNINGS.md` (kqueue root causes #6–#7 apply here too).
3. Read `.agents/AGENTS.md` and your agent doc in `.agents/agents/`.
4. Confirm the windows/shell selectors have the same `new_with_registry`-only shape, then apply the plan.
5. Verify with `cargo check` where a target is available; note in the report if a target is not installed.
6. Report to Main Agent when done — do not commit without verification.
7. Update `../../LEARNINGS.md`.

**Workflow:** feature.md → learnings → AGENTS.md → implement → verify → report → learnings

_Created: 2026-10-17_
