---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F08-android-ndk-linkers"
this_file: "specifications/62-macos-native-build/features/F08-android-ndk-linkers/start.md"
created: 2026-10-17
---

# Start: F08 — Android NDK linkers

## Agent Workflow

1. Read `feature.md`.
2. Read `../../LEARNINGS.md`.
3. Read `.agents/AGENTS.md` and your agent doc in `.agents/agents/`.
4. Make the NDK paths OS-relative and reconcile the NDK version comment.
5. Verify no `/home/darkvoid/...` path remains in `.cargo/config.toml`.
6. Report to Main Agent when done — do not commit without verification.
7. Update `../../LEARNINGS.md`.

**Workflow:** feature.md → learnings → AGENTS.md → implement → verify → report → learnings

_Created: 2026-10-17_
