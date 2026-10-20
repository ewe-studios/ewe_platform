---
workspace_name: "ewe_platform"
spec_directory: "specifications/62-macos-native-build"
this_file: "specifications/62-macos-native-build/start.md"
created: 2026-10-17
---

# Start: 62 — macOS native build

## Agent Workflow

1. Read `requirements.md` (overview + feature index).
2. Read `LEARNINGS.md` (already-diagnosed root causes — do not re-derive them).
3. Identify which feature you are working on from the Feature Index.
4. Navigate to `features/<feature>/start.md` and follow that feature's workflow.

**Suggested order:** F06 (unblocks the `uat` build) → F09 (rebuild + enumerate
new breakages) → F05 (dev-profile cranelift) → F07 (selector parity) → F08
(optional, non-blocking).

**Always update `LEARNINGS.md`** after each completed task/milestone.

---

**Workflow:** Requirements → Select feature → Feature start.md → Follow feature workflow

---

_Created: 2026-10-17_
