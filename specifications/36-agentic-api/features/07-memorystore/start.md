---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/07-memorystore"
this_file: "specifications/36-agentic-api/features/07-memorystore/start.md"
created: 2026-06-14
---

# Start: MemoryStore

## Agent Workflow

1. Read `feature.md` + Decision 01 (resume) + Decision 03 (memory tiers).
2. **Stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md`. Read the existing
   `KeyValueStore` trait (`foundation_db/src/core/storage_provider.rs:309`) and F01's memory types.
3. Confirm F01 (memory payloads) + F04 (DocumentStore fallback) landed.
4. Read `../../LEARNINGS.md`. Resolve OD-07-1..5 before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** factor shared snapshots → trait → `KvMemoryStore` → fjall opt → hydrate.
7. Report; verify; update `../../LEARNINGS.md`; move to Feature 08.

---

**Workflow:** feature.md → Decisions 01/03 → KeyValueStore + F01 types → Resolve OD-07 → Compact → ONE ITEM → Report → Verify → 08

---

_Created: 2026-06-14_
