---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/11-toolcall-execution-dag"
this_file: "specifications/36-agentic-api/features/11-toolcall-execution-dag/start.md"
created: 2026-06-14
---

# Start: ToolCall Execution DAG

## Agent Workflow

1. Read `feature.md` + Decision 04 + Decision 16. Confirm F01 added `depends_on`/`execution_hint` to
   `ModelOutput::ToolCall` (real enum `types/mod.rs:870` had neither). Read F13 (PriorityQueue/
   CancelCode/Depends) + F08 (`append`).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
   CRITICAL: backoff = `TaskStatus::Delayed`, never `sleep()`/`SleepIterator` (blocks executor; breaks
   wasm where valtron yields to the JS loop). Memory `feedback_async_iterators`.
3. Read `../../LEARNINGS.md`. Resolve OD-11-1..5. **OD-11-3 (persist durability) + OD-11-4 (backoff)
   need confirmation** — surface them.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** build_workflow (topo) → execute_workflow (valtron) → parallel/sequential →
   persist-before-deliver → retry (Delayed) → tool-error-to-LLM → interruption (Depends).
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 12.

---

**Workflow:** feature.md → Decision 04/16 → F01 fields + F13 + F08 → Resolve OD-11 (flag 3+4) → Compact → ONE ITEM → fundamentals → Report → Verify → 24

---

_Created: 2026-06-14_
