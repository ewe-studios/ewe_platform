---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
this_file: "specifications/36-agentic-api/start.md"
created: 2026-06-14
---

# Start: Agentic API for foundation_ai

## Agent Workflow

1. Read `requirements.md` (high-level overview + Feature Index + architecture).
2. **Language stack:** Rust (native + `wasm32` + Cloudflare Workers). Read the Rust skills:
   - `.agents/skills/rust-clean-code/skill.md`
   - `.agents/skills/rust-valtron-iterator/skill.md`
   - `.agents/skills/rust-valtron-usage/skill.md`
3. Skim `decisions/01-18` for the rationale behind the feature you'll implement (each feature.md
   cites the decisions it implements).
4. Read `LEARNINGS.md` (past discoveries and mistakes) if present.
5. Identify your feature from the **Feature Index** in `requirements.md`.
6. Navigate to `features/[NN-feature-name]/start.md` and follow that feature's workflow.
7. **Build in dependency order** — Phase 0 → Phase 7. Do not start a downstream feature before
   its prerequisites land (see the Feature Index dependency column).
8. **ALWAYS UPDATE LEARNINGS.md** after each completed task/milestone.

## Notes

- The `decisions/` directory is the resolved ADR record (rationale). `features/*/feature.md` is
  the implementation contract (what/how). `plan.md` is the original brainstorm (historical).
- Native-only tooling (fff, fjall, heed, memmap2, rayon) is **target-gated**, never
  feature-gated; every component must have a WASM-capable path or fallback.

---

**Workflow:** Requirements → Rust skills → Decisions (rationale) → Learnings → Select Feature →
Feature start.md → Follow feature workflow (dependency order)

---

_Created: 2026-06-14_
