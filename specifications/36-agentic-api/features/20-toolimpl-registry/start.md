---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/20-toolimpl-registry"
this_file: "specifications/36-agentic-api/features/20-toolimpl-registry/start.md"
created: 2026-06-14
---

# Start: ToolImpl & Registry

## Agent Workflow

1. Read `feature.md` + Decision 15 + Decision 04 (DAG fields). Read the REAL `Tool`/`ToolShed`/
   `ToolFormatter`/`Args`/`ArgType` in `backends/foundation_ai/src/types/mod.rs` — reuse, don't redefine.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-20-1..5. **OD-20-1 (sync vs async execute) and OD-20-4
   (ToolShed optionality) need a user ruling** — surface them. `register` is `&self` (Decision 08).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** ToolImpl trait → ToolDefinition→Tool conversion → registry → execute_one +
   arg validation → build_toolshed (no others) → register discovery hook.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 21.

---

**Workflow:** feature.md → Decision 15/04 → real types/mod.rs → Resolve OD-20 (flag 1+4) → Compact → ONE ITEM → fundamentals → Report → Verify → 21

---

_Created: 2026-06-14_
