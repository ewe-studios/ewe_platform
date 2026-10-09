---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/17-loop-detection"
this_file: "specifications/36-agentic-api/features/17-loop-detection/start.md"
created: 2026-06-14
---

# Start: Loop Detection

## Agent Workflow

1. Read `feature.md` + Decision 09. Note the Decision 08-vs-11 execution-model conflict — **OD-17-1
   NEEDS A USER RULING** (rec: output processor). Read the real `ModelOutput` (`types/mod.rs:863`),
   `ModelParams.temperature` (:382), F15 `latest_working`/`latest_reflection`, F14 `OutputProcessor`.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-17-1..5. Reuse F31's 64-bit hasher (don't add a second).
   `LoopDetection` MUST be `Clone+PartialEq+Debug` (rides `AgenticError`).
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** exact → SimHash fuzzy → tool-call sig → build_redirect (F15) → escalation
   ladder → wire as F14 output processor (if OD-17-1 = processor).
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 18.

---

**Workflow:** feature.md → Decision 09 → real ModelOutput/ModelParams + F15/F14 → Resolve OD-17 (USER RULING on 1) → Compact → ONE ITEM → fundamentals → Report → Verify → 29

---

_Created: 2026-06-14_
