---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/27-agentic-loop"
this_file: "specifications/36-agentic-api/features/27-agentic-loop/start.md"
created: 2026-06-14
---

# Start: Agentic Loop

## Agent Workflow

1. Read `feature.md` + Decision 11 + Decision 08 + Decision 05. Read F02 stream contract
   (`StreamIterator<D=Result<SessionRecord,AgenticError>, P=AgentProgress>`) — the `AgentEvent` enum is
   DEAD. Read the real `TaskIterator` (`task.rs:392`) + confirm the `Spawner` type. Read F23/F24/F25/
   F26/F28/F30 surfaces.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-iterator`
   + `rust-valtron-usage`. NEVER block `next_status`: pump streams step-wise, wait via `Depends`,
   backoff via `Delayed`, wasm yields to JS loop. Memory `feedback_async_iterators`.
3. Read `../../LEARNINGS.md`. Resolve OD-27-1..5. **OD-27-2 (AgentEvent dead) + OD-27-3 (mid-gen
   interruption)** — confirm before coding.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** state machine skeleton → OuterBoundary → InnerAssemble (priority front +
   F26) → InnerGenerate (pump F24, interrupt) → tool DAG (F23) → OutputProcessing (F26/F19/F28) →
   handle_error/circuit breaker (F30/F24) → Ending.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 28.

---

**Workflow:** feature.md → Decision 11/08/05 → F02 contract + real TaskIterator → Resolve OD-27 (flag 2+3) → Compact → ONE ITEM → fundamentals → Report → Verify → 28

---

_Created: 2026-06-14_
