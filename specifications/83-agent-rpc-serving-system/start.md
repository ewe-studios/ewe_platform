---
workspace_name: "ewe_platform"
spec_directory: "specifications/83-agent-rpc-serving-system"
this_file: "specifications/83-agent-rpc-serving-system/start.md"
created: 2026-10-19
---

# Start: Agent RPC

## Agent Workflow

1. Read `requirements.md` (mission, feature index, spec order, success criteria).
2. **Identify language stack** — Rust. Read `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
3. Read `LEARNINGS.md` (past discoveries and mistakes).
4. Read `agent-rpc-architecture.md` for the shared design of the pluggable server.
5. Read the requirements of the two specs this one builds on:
   `../80-platform-prerequisites/requirements.md` and
   `../81-foundation-auth/requirements.md`.
6. Pick a feature from the Feature Index and respect the order: 03 before 04 and 05; 06 after 05;
   08 last.
7. Navigate to `features/[feature]/start.md`.
8. **ALWAYS UPDATE `LEARNINGS.md`** after each completed task or milestone.

**Spec order:** 80 → 81 → 65. The operator path is separate: 78 → 79, and 82 after 78 and 81.

**Feature order:** 00, 01 and 02 are complete. 03 (core) is next. 04 (native host) makes the
proof of concept work. 05 (Worker host) follows 03. 06 (Cloudflare) follows 05. 08 (key
protection) is last.

---

**Workflow:** Requirements → Shared design → 80 and 81 baselines → Select feature →
Feature start.md → Follow feature workflow

---

_Created: 2026-10-19_
_Last updated: 2026-11-14 (split; this spec is now agent RPC only)_
