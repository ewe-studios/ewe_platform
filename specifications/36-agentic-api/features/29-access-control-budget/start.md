---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/29-access-control-budget"
this_file: "specifications/36-agentic-api/features/29-access-control-budget/start.md"
created: 2026-06-14
---

# Start: Access Control & Budget Surfacing

## Agent Workflow

1. Read `feature.md` + Decision 12 (note the user TODOs: local=allow-all, "Agent owns the session").
   Verify the collision: `foundation_ai::AuthProvider` (`types/mod.rs:1457`, provider credentials) vs
   the new `SessionAccessProvider`. Read `foundation_auth` (`shared/auth_token.rs`) + F03 `TokenLedger`
   `set_budget`.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
3. Read `../../LEARNINGS.md`. Resolve OD-29-1..5. `AuthError` MUST be `Clone+PartialEq+Debug` (F30 stream).
   **OD-29-4 (budget surfacing: cap ledger + system note)** — confirm.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** types (UserId/AuthError/TokenBudget) → SessionAccessProvider trait →
   AllowAllAccess → budget surfacing (F03 cap + note) → foundation_auth bridge → tool-gating hook.
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 30.

---

**Workflow:** feature.md → Decision 12 + collision check → foundation_auth + F03 → Resolve OD-29 (flag 4) → Compact → ONE ITEM → fundamentals → Report → Verify → 30

---

_Created: 2026-06-14_
