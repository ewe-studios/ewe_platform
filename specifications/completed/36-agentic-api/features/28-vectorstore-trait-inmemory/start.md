---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/28-vectorstore-trait-inmemory"
this_file: "specifications/36-agentic-api/features/28-vectorstore-trait-inmemory/start.md"
created: 2026-06-14
---

# Start: VectorStore trait + in-memory

## Agent Workflow

1. Read `feature.md` + Decision 07/12. Confirm F24 (foundation_vectors) landed.
2. **Stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md`. Read the existing storage traits
   (`foundation_db/src/core/storage_provider.rs`) for house style.
3. Read `../../LEARNINGS.md`. Resolve OD-28-1..4 before coding.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** trait + types → InMemoryVectorStore shards → dimension/namespace → tests.
6. **Author the `fundamentals/` docs** (zero-to-expert) as part of completion.
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 29.

---

**Workflow:** feature.md → Decision 07 → storage traits → Resolve OD-28 → Compact → ONE ITEM → fundamentals → Report → Verify → 13

---

_Created: 2026-06-14_
