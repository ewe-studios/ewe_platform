---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/10-toolshed-shed-metatool"
this_file: "specifications/36-agentic-api/features/10-toolshed-shed-metatool/start.md"
created: 2026-06-14
---

# Start: ToolShed & shed meta-tool

## Agent Workflow

1. Read `feature.md` + Decision 15. Read the REAL `ToolShed` (`types/mod.rs:1067`) + `ToolFormatter`
   (:1140). Confirm `others` is removed by F01 and `shed: Tool` is non-optional.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-10-1..5. **OD-10-1 (ToolShed field optionality) + OD-10-3
   (summary vs schema) need a user ruling** — surface them. Tool vectors use ns=`"tools"`.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** ToolDiscovery (F31+F28) → ShedTool → with_defaults → ToolShed::all_tools.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 32.

---

**Workflow:** feature.md → Decision 15 → real ToolShed/ToolFormatter → Resolve OD-10 (flag 1+3) → Compact → ONE ITEM → fundamentals → Report → Verify → 22

---

_Created: 2026-06-14_
