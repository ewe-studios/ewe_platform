---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/14-input-output-processors"
this_file: "specifications/36-agentic-api/features/14-input-output-processors/start.md"
created: 2026-06-14
---

# Start: Input/Output Processors

## Agent Workflow

1. Read `feature.md` + Decision 11 (Mastra processors). Read F16 `AgentContext`/`assemble`, F15
   `check_triggers`/`generate`, F08 `append`, F31 `embed`. Confirm the input pipeline mirrors F16's
   Decision 03 assembly order (replay determinism).
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
   Output processors SPAWN valtron tasks (SpawnSink) — never block `next_status`.
3. Read `../../LEARNINGS.md`. Resolve OD-14-1..5. **OD-14-1 (assemble == input pipeline) needs a
   ruling; OD-14-4 (LoopDetector placement) is DEFERRED to F17 — leave the slot open.**
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** ProcessorOutcome + traits → pipelines (dedup/priority/critical) → SpawnSink
   → default input processors → default output processors.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 19.

---

**Workflow:** feature.md → Decision 11 → F16/F15/F08/F31 → Resolve OD-14 (flag 1, defer 4 to F17) → Compact → ONE ITEM → fundamentals → Report → Verify → 27

---

_Created: 2026-06-14_
