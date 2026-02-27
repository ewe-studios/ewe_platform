---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/public-api"
this_file: "specifications/02-build-http-client/features/public-api/start.md"
created: 2026-02-28
---

# Start: Public Api Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. Read `../../LEARNINGS.md` (past discoveries and mistakes)
3. Read `./VERIFICATION.md` (verification requirements)
3. Read `./PROGRESS.md` (last progress of this feature if present)
4. Read `.agents/AGENTS.md` to identify your agent type
5. Read your agent file in `.agents/agents/[agent-name].md`
6. Read skills specified in your agent documentation
7. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
8. Clear context, reload from `compacted.md` only, start work
9. **Work on ONE item at a time** - one test, one function, one file - finish it completely before next
10. Implement following TDD (test first, then code) - **one test at a time**
11. Report to Main Agent when done (DO NOT commit)
12. Wait for verification to pass
13. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task

---

**Workflow:** Feature.md → Learnings → Verification → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-02-28_
