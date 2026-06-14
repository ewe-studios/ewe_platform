---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/03-token-accounting-budget"
this_file: "specifications/36-agentic-api/features/03-token-accounting-budget/start.md"
created: 2026-06-14
---

# Start: Token Accounting & Budget

## Agent Workflow

1. Read `feature.md` + Decisions 02/03 (the token-accumulation TODO #5).
2. **Stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md`. Read `UsageReport`
   (`types/mod.rs:751`), `costing.rs`, `ModelState::GeneratingTokens`, `ModelParams::max_tokens`.
3. Read `../../LEARNINGS.md`. Confirm F01 landed.
4. Resolve OD-03-1..4 (folding, hard cap, rolling definition, persistence) before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** ledger atomics + `record` → budget API → snapshot → tests (incl. concurrent).
7. Report; verify; update `../../LEARNINGS.md`; move to Feature 04.

---

**Workflow:** feature.md → Decisions 02/03 → Rust skill → UsageReport grounding → Resolve OD-03 → Compact → ONE ITEM → Report → Verify → 04

---

_Created: 2026-06-14_
