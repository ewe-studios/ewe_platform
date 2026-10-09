---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/06-documentstore-trait-sql-memory"
this_file: "specifications/36-agentic-api/features/06-documentstore-trait-sql-memory/start.md"
created: 2026-06-14
---

# Start: DocumentStore — scan_from + promoted columns

## Agent Workflow

1. Read `feature.md` + Decision 13 + Decision 10b (the column TODO).
2. **Stack:** Rust + SQL. Read `.agents/skills/rust-clean-code/skill.md`.
3. Read the **existing** `DocumentStore` (`foundation_db/src/core/storage_provider.rs:471`),
   `MemoryDocumentStore`, `SqlDocumentStore`, and `schema/sql/020_create_documents.sql` — this is an
   EXTENSION, do not rewrite them.
4. Read `../../LEARNINGS.md`. Resolve OD-06-1..3 before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** `scan_from` on trait + both backends (with tests) → migration + columns →
   `Document` fields → write path. Additive only; existing tests stay green.
7. Report; verify; update `../../LEARNINGS.md`; move to Feature 22.

---

**Workflow:** feature.md → Decisions 13/10b → existing DocumentStore → Resolve OD-06 → Compact → ONE ITEM → Report → Verify → 05

---

_Created: 2026-06-14_
