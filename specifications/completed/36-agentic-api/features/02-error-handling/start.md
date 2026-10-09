---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/02-error-handling"
this_file: "specifications/36-agentic-api/features/02-error-handling/start.md"
created: 2026-06-14
---

# Start: Error Handling

## Agent Workflow

1. Read `feature.md` + Decision 16. Read the REAL `GenerationError` (`errors/mod.rs:18` — NOT Clone/
   PartialEq, NO ContextOverflow/RateLimit) and `Messages::is_context_overflow(context_window: u64)`
   (`types/mod.rs:967`). Audit `StorageError`/`VectorStoreError`/`EmbeddingError` for Clone/PartialEq.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-02-1..5. **OD-02-1 (flatten non-Clone sources) + OD-02-2
   (detect overflow/rate-limit) depart from Decision 16's `#[from]` — surface for a user ruling.**
   `AgenticError` MUST be `Clone+PartialEq+Debug` (F03).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** AgenticError + GenerationFailure → flatten sources → from_generation
   (detect overflow/rate-limit) → ErrorPolicy/AgentAction → CircuitBreaker.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 20.

---

**Workflow:** feature.md → Decision 16 → real GenerationError + is_context_overflow → Resolve OD-02 (flag 1+2) → Compact → ONE ITEM → fundamentals → Report → Verify → 31

---

_Created: 2026-06-14_
