---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/03-agent-rpc-core"
this_file: "specifications/65-agent-rpc-serving-system/features/03-agent-rpc-core/start.md"
created: 2026-11-14
---

# Start: Agent RPC Core Feature

## Agent Workflow

1. Read `feature.md` (tasks T13 to T16, what to read, "done when").
2. Read `proto.md` (the wire contract) and `registry-and-models.md` (the model catalog and
   the instance registry).
3. Read `../../agent-rpc-architecture.md` (shared design, owner decisions, open questions).
4. Read `../../../80-platform-prerequisites/requirements.md` and
   `../../../81-foundation-auth/requirements.md` (the base this feature builds on).
5. Read `../../LEARNINGS.md` (past discoveries and mistakes).
6. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`. Generate `compacted.md` with
   `.agents/skills/context-compaction/skill.md`.
7. Work on one task at a time, test first. Tests live in
   `backends/foundation_agent_rpc/tests/` and `proto/`.
8. Report to the Main Agent. After the commit, update `../../progress.md` and
   `../../LEARNINGS.md`.

**Order:** spec 80 and spec 81 land first. This feature runs before feature 04 and feature 05.

---

_Created: 2026-11-14_
