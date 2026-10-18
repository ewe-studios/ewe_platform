---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F06-kqueue-selector-compile"
this_file: "specifications/62-macos-native-build/features/F06-kqueue-selector-compile/start.md"
created: 2026-10-17
---

# Start: F06 — Kqueue selector compile fixes

## Agent Workflow

1. Read `feature.md` (the 16-error inventory and 4-step plan are fully specified).
2. Read `../../LEARNINGS.md` (kqueue root causes #4–#7).
3. Read `.agents/AGENTS.md` and your agent doc in `.agents/agents/`.
4. Apply Step 1 → Step 4 in order; one step at a time.
5. Verify: `LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib cargo build -p foundation_nativeapis --profile uat`
   must pass with zero errors.
6. Report to Main Agent when done — do not commit without verification.
7. Update `../../LEARNINGS.md` if the build surfaces anything new.

**Workflow:** feature.md → learnings → AGENTS.md → implement → verify → report → learnings

_Created: 2026-10-17_
