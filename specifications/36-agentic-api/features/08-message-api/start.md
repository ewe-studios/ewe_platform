---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/08-message-api"
this_file: "specifications/36-agentic-api/features/08-message-api/start.md"
created: 2026-06-14
---

# Start: Message API

## Agent Workflow

1. Read `feature.md` + Decision 02 (+ CRIT-02 Broadcaster). Confirm F01/F06/F28/F31 landed.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
   Read `foundation_core/src/synca/mpp.rs` (Broadcaster) for the `&self` pub/sub fix.
3. Read `../../LEARNINGS.md`. Resolve OD-08-1..5 (esp. pub/sub mechanism + backpressure).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** store + buffer → flush task → pub/sub → reads → semantic_search.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 05.

---

**Workflow:** feature.md → Decision 02 + CRIT-02 → F01/F06/F28/F31 → Resolve OD-08 → Compact → ONE ITEM → fundamentals → Report → Verify → 17

---

_Created: 2026-06-14_
