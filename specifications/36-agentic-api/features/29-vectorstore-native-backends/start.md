---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/29-vectorstore-native-backends"
this_file: "specifications/36-agentic-api/features/29-vectorstore-native-backends/start.md"
created: 2026-06-14
---

# Start: VectorStore native backends

## Agent Workflow

1. Read `feature.md` + Decision 07. Confirm F28 (trait) + F25 (IVF) landed.
2. **Do the research pass first** (Turso `vector_top_k`, sqlite-vec, fjall layout) — document in
   `feature.md` before coding (Decision 07 mandate).
3. **Stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md`. Reuse foundation_db's existing
   SQL connection/query layer; follow `feedback_target_gate_native_tooling`.
4. Read `../../LEARNINGS.md`. Resolve OD-29-1..5.
5. Generate `compacted.md`, clear, reload.
6. **One backend at a time:** Turso → SQLite → fjall, each with parity + persistence + namespace tests.
7. **Author `fundamentals/` docs.**
8. Report; verify; update `../../LEARNINGS.md`; move to Feature 30.

---

**Workflow:** feature.md → research pass → Decision 07 → Resolve OD-29 → Compact → ONE BACKEND AT A TIME → fundamentals → Report → Verify → 14

---

_Created: 2026-06-14_
