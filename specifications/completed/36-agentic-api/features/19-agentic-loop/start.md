---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/19-agentic-loop"
this_file: "specifications/36-agentic-api/features/19-agentic-loop/start.md"
created: 2026-06-14
---

# Start: Agentic Loop

## Agent Workflow

1. Read `feature.md` + Decision 11 + Decision 08 + Decision 05. Read F03 stream contract
   (`StreamIterator<D=Result<SessionRecord,AgenticError>, P=AgentProgress>`) — the `AgentEvent` enum is
   DEAD. Read the real `TaskIterator` (`task.rs:392`) + confirm the `Spawner` type. Read F11/F12/F13/
   F14/F17/F02 surfaces.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-iterator`
   + `rust-valtron-usage`. NEVER block `next_status`: pump streams step-wise, wait via `Depends`,
   backoff via `Delayed`, wasm yields to JS loop. Memory `feedback_async_iterators`.
3. Read `../../LEARNINGS.md`. Resolve OD-19-1..5. **OD-19-2 (AgentEvent dead) + OD-19-3 (mid-gen
   interruption)** — confirm before coding.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** state machine skeleton → OuterBoundary → InnerAssemble (priority front +
   F14) → InnerGenerate (pump F12, interrupt) → tool DAG (F11) → OutputProcessing (F14/F15/F17) →
   handle_error/circuit breaker (F02/F12) → Ending.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 17.

---

**Workflow:** feature.md → Decision 11/08/05 → F03 contract + real TaskIterator → Resolve OD-19 (flag 2+3) → Compact → ONE ITEM → fundamentals → Report → Verify → 28

---

_Created: 2026-06-14_
