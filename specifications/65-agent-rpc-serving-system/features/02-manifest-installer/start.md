---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/02-manifest-installer"
this_file: "specifications/65-agent-rpc-serving-system/features/02-manifest-installer/start.md"
created: 2026-10-19
---

# Start: Manifest Installer Feature

## Agent Workflow

1. Read `feature.md` (requirements + architecture + tasks).
2. **Identify language stack** — Rust. Read `.agents/skills/rust-clean-code/skill.md`.
3. Read `../../LEARNINGS.md` (past discoveries and mistakes).
4. Read `.agents/AGENTS.md` to identify your agent type, then your agent file in
   `.agents/agents/[agent-name].md`.
5. Read skills specified in your agent documentation.
6. **MANDATORY**: Generate `compacted.md` with all info using
   `.agents/skills/context-compaction/skill.md`.
7. Clear context, reload from `compacted.md` only, start work.
8. **Work on ONE item at a time** — one test, one function, one file.
9. Implement following TDD (test first, then code) — **one test at a time**.
10. Report to Main Agent when done (DO NOT commit).
11. Wait for verification to pass.
12. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task.
13. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone.

---

**Workflow:** feature.md → Rust skills → Learnings → AGENTS.md → Agent Doc → Skills → Compact → Clear → Reload → ONE ITEM AT A TIME → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-10-19_
