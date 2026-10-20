---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
feature_directory: "specifications/62-macos-native-build/features/F05-cranelift-toolchain"
this_file: "specifications/62-macos-native-build/features/F05-cranelift-toolchain/start.md"
created: 2026-10-17
---

# Start: F05 — Cranelift toolchain

## Agent Workflow

1. Read `feature.md` (detailed requirements + plan).
2. Read `../../LEARNINGS.md` (toolchain root causes #8–#9).
3. Read `.agents/AGENTS.md` and your agent doc in `.agents/agents/`.
4. Work one step of `feature.md` at a time.
5. Verify with a real `cargo build` (default `dev` profile) before committing.
6. Report to Main Agent when done — do not commit without verification.
7. Update `../../LEARNINGS.md` with any new discovery.

**Workflow:** feature.md → learnings → AGENTS.md → implement → verify → report → learnings

_Created: 2026-10-17_
