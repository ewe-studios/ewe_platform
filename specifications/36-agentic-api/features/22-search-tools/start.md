---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/22-search-tools"
this_file: "specifications/36-agentic-api/features/22-search-tools/start.md"
created: 2026-06-14
---

# Start: Search Tools

## Agent Workflow

1. Read `feature.md` + Decision 14 (TODO #6 split). Read F18 `ContextProvider::search`/`search_file`
   + `FffSearch`. Inspect the external fff workspace at
   `/home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.Search/fff` (crates `fff-core`,
   `fff-grep`, `fff-query-parser`) — confirm its native-only deps (heed/memmap2/git2/rayon/notify).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*` +
   memory `feedback_target_gate_native_tooling` (target-gate fff, never feature-gate).
3. Read `../../LEARNINGS.md`. Resolve OD-22-1..5. **OD-22-1 (how to consume the external fff) needs a
   user ruling** — surface it. wasm `search_file` returns unsupported, never panics.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** SearchTool (wraps F18) → SearchFileTool (target-gated) → fff wiring →
   descriptions → register via F21.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm; wasm MUST exclude fff); update `../../LEARNINGS.md`; move to Feature 23.

---

**Workflow:** feature.md → Decision 14 #6 → F18 + external fff → Resolve OD-22 (flag 1) → Compact → ONE ITEM → fundamentals → Report → Verify → 23

---

_Created: 2026-06-14_
