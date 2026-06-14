---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/01-message-model"
this_file: "specifications/36-agentic-api/features/01-message-model/start.md"
created: 2026-06-14
---

# Start: Message Model & Types Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks + Open Decisions).
2. **Language stack:** Rust. Read `.agents/skills/rust-clean-code/skill.md` before writing code.
3. Read the cited decisions for rationale: `../../decisions/02-message-api.md`,
   `../../decisions/04-toolcall-manager.md`, `../../decisions/01-session-architecture.md`.
4. Read `../../LEARNINGS.md` and `./PROGRESS.md` if present.
5. Read `.agents/AGENTS.md` and your agent doc; read skills it specifies.
6. **Confirm Open Decisions (OD-1..OD-5) are resolved** in `feature.md` before coding — these
   change the type shapes.
7. Generate `compacted.md` (context-compaction skill), clear context, reload.
8. **Work one item at a time** (one type, one migration site) — TDD: serde round-trip test first.
9. Place tests per the Rust testing convention (`tests/` dir).
10. Report to Main Agent when done (do not commit). Wait for verification.
11. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to Feature 02.
12. **ALWAYS UPDATE ../../LEARNINGS.md** after each milestone.

## Critical Notes

- This is **pure type substrate** — no behavior. Every later feature depends on it.
- `MessageRole` and `ToolShed.others` removal are **breaking**: ~39 `role:` sites + 5
  `flatten_tools` functions. Keep serde wire-compatible (legacy JSON must still parse).

---

**Workflow:** feature.md → Rust skill → Decisions → Resolve OD-1..OD-5 → Compact → ONE ITEM AT A TIME → TDD → Report → Verify → Next

---

_Created: 2026-06-14_
