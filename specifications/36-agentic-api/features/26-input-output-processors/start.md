---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/26-input-output-processors"
this_file: "specifications/36-agentic-api/features/26-input-output-processors/start.md"
created: 2026-06-14
---

# Start: Input/Output Processors

## Agent Workflow

1. Read `feature.md` + Decision 11 (Mastra processors). Read F18 `AgentContext`/`assemble`, F19
   `check_triggers`/`generate`, F16 `append`, F15 `embed`. Confirm the input pipeline mirrors F18's
   Decision 03 assembly order (replay determinism).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
   Output processors SPAWN valtron tasks (SpawnSink) — never block `next_status`.
3. Read `../../LEARNINGS.md`. Resolve OD-26-1..5. **OD-26-1 (assemble == input pipeline) needs a
   ruling; OD-26-4 (LoopDetector placement) is DEFERRED to F28 — leave the slot open.**
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** ProcessorOutcome + traits → pipelines (dedup/priority/critical) → SpawnSink
   → default input processors → default output processors.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 27.

---

**Workflow:** feature.md → Decision 11 → F18/F19/F16/F15 → Resolve OD-26 (flag 1, defer 4 to F28) → Compact → ONE ITEM → fundamentals → Report → Verify → 27

---

_Created: 2026-06-14_
