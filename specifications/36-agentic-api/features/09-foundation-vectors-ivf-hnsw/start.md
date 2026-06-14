---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/09-foundation-vectors-ivf-hnsw"
this_file: "specifications/36-agentic-api/features/09-foundation-vectors-ivf-hnsw/start.md"
created: 2026-06-14
---

# Start: foundation_vectors IVF + HNSW

## Agent Workflow

1. Read `feature.md` + Decision 07. Study Chroma's HNSW for reference (algorithms owned by us).
2. **Stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md`. Confirm F08 landed (metrics, flat).
3. Read `../../LEARNINGS.md`. Resolve OD-09-1..6 before coding.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** `VectorIndex` trait + `FlatIndex` → IVF → HNSW → serialize → recall bench.
6. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 10.

---

**Workflow:** feature.md → Decision 07 + Chroma → Rust skill → Resolve OD-09 → Compact → ONE ITEM → recall bench → Report → Verify → 10

---

_Created: 2026-06-14_
