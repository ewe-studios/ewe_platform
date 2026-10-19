---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
this_file: "specifications/65-agent-rpc-serving-system/start.md"
created: 2026-10-19
---

# Start: Agent RPC Serving System

## Agent Workflow

1. Read `requirements.md` (overview + feature index + spec-wide architecture).
2. **Identify language stack** — Rust. Read `.agents/skills/rust-clean-code/skill.md`
   and `.agents/skills/rust-valtron-usage/skill.md` (async/valtron boundaries).
3. Read `LEARNINGS.md` (past discoveries and mistakes).
4. Pick a feature from the Feature Index (respect dependency order).
5. Navigate to `features/[feature]/start.md` and follow that feature's workflow.
6. **ALWAYS UPDATE `LEARNINGS.md`** after each completed task/milestone.

**Feature order:** 0 (agent-rpc) and 1 (installer) are independent leaves; 2
(proxy registration) extends the proxy; 3 (registrar) composes 1 + 2.

---

**Workflow:** Requirements → Select Feature → Feature start.md → Follow feature workflow

---

_Created: 2026-10-19_
