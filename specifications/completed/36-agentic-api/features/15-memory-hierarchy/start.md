---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/15-memory-hierarchy"
this_file: "specifications/36-agentic-api/features/15-memory-hierarchy/start.md"
created: 2026-06-14
---

# Start: Memory Hierarchy

## Agent Workflow

1. Read `feature.md` + Decision 03. Read F04 `TokenLedger` (`rolling`/`reset_rolling`), F07
   `MemoryStore`, F08 Message API (`append` returns `Scru128`), F01 `SessionRecord` memory variants.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-15-1..5. Generation runs as a valtron sub-task — NEVER block
   `next_status`; use `schedule`/`lift`, `TaskStatus::Delayed`/`Wait` (never `SleepIterator`).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** triggers → observation gen → reflection gen (replace) → working memory →
   latest getters → valtron `MemoryGenTask`.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 09.

---

**Workflow:** feature.md → Decision 03 → F04/F07/F08/F01 → Resolve OD-15 → Compact → ONE ITEM → fundamentals → Report → Verify → 20

---

_Created: 2026-06-14_
