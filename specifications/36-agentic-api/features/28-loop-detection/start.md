---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/28-loop-detection"
this_file: "specifications/36-agentic-api/features/28-loop-detection/start.md"
created: 2026-06-14
---

# Start: Loop Detection

## Agent Workflow

1. Read `feature.md` + Decision 09. Note the Decision 08-vs-11 execution-model conflict — **OD-28-1
   NEEDS A USER RULING** (rec: output processor). Read the real `ModelOutput` (`types/mod.rs:863`),
   `ModelParams.temperature` (:382), F19 `latest_working`/`latest_reflection`, F26 `OutputProcessor`.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-28-1..5. Reuse F15's 64-bit hasher (don't add a second).
   `LoopDetection` MUST be `Clone+PartialEq+Debug` (rides `AgenticError`).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** exact → SimHash fuzzy → tool-call sig → build_redirect (F19) → escalation
   ladder → wire as F26 output processor (if OD-28-1 = processor).
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 29.

---

**Workflow:** feature.md → Decision 09 → real ModelOutput/ModelParams + F19/F26 → Resolve OD-28 (USER RULING on 1) → Compact → ONE ITEM → fundamentals → Report → Verify → 29

---

_Created: 2026-06-14_
