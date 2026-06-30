---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/26-foundation-vectors-bm25-hybrid"
this_file: "specifications/36-agentic-api/features/26-foundation-vectors-bm25-hybrid/start.md"
created: 2026-06-14
---

# Start: foundation_vectors BM25 + hybrid

## Agent Workflow

1. Read `feature.md` + Decision 07 (TODO #7 hybrid/RRF).
2. **Stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md`. Confirm F24/F25 landed.
3. Read `../../LEARNINGS.md`. Resolve OD-26-1..5 before coding.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** BM25 index → RRF/alpha fuse → Reranker hook + hybrid_search → tests.
6. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 27.

---

**Workflow:** feature.md → Decision 07 → Rust skill → Resolve OD-26 → Compact → ONE ITEM → Report → Verify → 11

---

_Created: 2026-06-14_
