---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/13-steering-queues-depends"
this_file: "specifications/36-agentic-api/features/13-steering-queues-depends/start.md"
created: 2026-06-14
---

# Start: Steering Queues & Depends

## Agent Workflow

1. Read `feature.md` + Decision 05 + Decision 08. Read the REAL valtron primitives:
   `EventReadiness`/`QueueReadiness`/`BoolSignal` (`task.rs:30-116`), `TaskStatus::Depends` (:253),
   `concurrent_queue::ConcurrentQueue`. Read `dependent_lift.rs` for the sequenced spawn API (OD-13-4).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
   CRITICAL: queue waits = `TaskStatus::Depends(QueueReadiness)`, never `Pending`/`Delayed`/`SleepIterator` spin.
3. Read `../../LEARNINGS.md`. Resolve OD-13-1..5. **OD-13-4 (confirm sequenced spawn API name)** —
   verify in code before coding. `CancelCode` MUST be `#[repr(u32)]` (Decision 05's `enum: u32` is invalid).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** CancelCode + AtomicU32 → SteeringQueues → readiness signals → drain →
   sequenced composition helper.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 14.

---

**Workflow:** feature.md → Decision 05/08 → real Depends/QueueReadiness + dependent_lift → Resolve OD-13 (flag 4) → Compact → ONE ITEM → fundamentals → Report → Verify → 26

---

_Created: 2026-06-14_
