---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-valtron-async-iterators"
feature_directory: "specifications/08-valtron-async-iterators/features/01-task-iterators"
this_file: "specifications/08-valtron-async-iterators/features/01-task-iterators/start.md"
created: 2026-03-20
---

# Start: Task Iterators Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. Read `../../LEARNINGS.md` (past discoveries and design decisions)
3. Read `../../requirements.md` (spec overview and architecture)
4. Read `../00-foundation/feature.md` (foundation feature - dependencies)
5. Read `.agents/AGENTS.md` to identify your agent type
6. Read your agent file in `.agents/agents/[agent-name].md`
7. Read skills specified in your agent documentation
8. **MANDATORY**: Generate `compacted.md` with all context using `.agents/skills/context-compaction/skill.md`
9. Clear context, reload from `compacted.md` only, start work
10. **Work on ONE item at a time** - one test, one function, one file - finish completely before next
11. Implement following TDD (test first, then code) - **one test at a time**
12. **Place tests in correct location** - `tests/backends/foundation_core/units/valtron/`
13. Report to Main Agent when done (DO NOT commit)
14. Wait for verification to pass
15. After commit: delete `compacted.md`, update `PROGRESS.md`, move to next task
16. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

**Workflow:** Feature.md → Learnings → Requirements → Foundation → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-03-20_
