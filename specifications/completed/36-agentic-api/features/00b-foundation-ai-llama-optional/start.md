---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/00b-foundation-ai-llama-optional"
this_file: "specifications/36-agentic-api/features/00b-foundation-ai-llama-optional/start.md"
created: 2026-06-14
---

# Start: foundation_ai — optional llama + compat wiring

## Agent Workflow

1. Read `feature.md` (llama gating + error enums + SystemTime migration + Open Decisions).
2. **Stack:** Rust + Cargo features. Read `.agents/skills/rust-clean-code/skill.md`.
3. Read `backends/foundation_ai/{Cargo.toml,src/errors/mod.rs,src/backends/mod.rs}` fully; grep all
   `match` sites on `GenerationError`/`ModelErrors` before editing.
4. Read `../../LEARNINGS.md`; follow `feedback_target_gate_native_tooling`.
5. Confirm Feature 00 landed (foundation_compact — folds in scru128/RNG).
6. Generate `compacted.md`, clear, reload.
7. **One item at a time:** make llama optional → green default build → gate error enums → green →
   gate backend modules → green → migrate SystemTime → green → backend-less build green. A green
   build between each.
8. Report to Main Agent (no commit). Wait for verification.
9. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to Feature 00c.
10. **ALWAYS UPDATE ../../LEARNINGS.md.**

## Critical Notes

- **Native default behavior must not change** (`default = ["llamacpp"]`).
- This feature does NOT make the wasm target build (providers' `thread::sleep`,
  `foundation_deployment`/`foundation_auth` are 00c). Goal: native green + backend-less compile.

---

**Workflow:** feature.md → errors/backends grounding → Rust skill → Compact → ONE ITEM AT A TIME → green between each → Report → Verify → 00c

---

_Created: 2026-06-14_
