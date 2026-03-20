---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-valtron-async-iterators"
feature_directory: "specifications/08-valtron-async-iterators/features/00-foundation"
this_file: "specifications/08-valtron-async-iterators/features/00-foundation/start.md"
created: 2026-03-20
---

# Start: Foundation Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. Read `../../LEARNINGS.md` (past discoveries and design decisions)
3. Read `../../requirements.md` (spec overview and architecture)
4. Read `.agents/AGENTS.md` to identify your agent type
5. Read your agent file in `.agents/agents/[agent-name].md`
6. Read skills specified in your agent documentation
7. **MANDATORY**: Generate `compacted.md` with all context using `.agents/skills/context-compaction/skill.md`
8. Clear context, reload from `compacted.md` only, start work
9. **Work on ONE item at a time** - one test, one function, one file - finish completely before next
10. Implement following TDD (test first, then code) - **one test at a time**
11. **Place tests in correct location** - `tests/backends/foundation_core/units/valtron/`
12. Report to Main Agent when done (DO NOT commit)
13. Wait for verification to pass
14. After commit: delete `compacted.md`, update `PROGRESS.md`, move to next task
15. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

**Workflow:** Feature.md → Learnings → Requirements → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-03-20_
