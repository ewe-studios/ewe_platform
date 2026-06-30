---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/05-serialization-json-arrow"
this_file: "specifications/36-agentic-api/features/05-serialization-json-arrow/start.md"
created: 2026-06-14
---

# Start: Serialization JSON + Arrow

## Agent Workflow

1. Read `feature.md` + Decision 10 (TODO #3 columns, #4 drop FlatBuffers). Confirm F01/F06 columns.
2. **Stack:** Rust + Arrow. Read `.agents/skills/rust-clean-code/skill.md`.
3. Read `../../LEARNINGS.md`. Resolve OD-05-1..4 (Arrow crate + wasm; column set vs F06).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** JSON round-trip → Arrow schema → to/from RecordBatch → analytics helpers.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm Arrow build); update `../../LEARNINGS.md`; move to Feature 16.

---

**Workflow:** feature.md → Decision 10 → F01/F06 columns → Resolve OD-05 → Compact → ONE ITEM → fundamentals → Report → Verify → 18

---

_Created: 2026-06-14_
