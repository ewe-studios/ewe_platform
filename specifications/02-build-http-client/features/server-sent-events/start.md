---
workspace_name: "ewe_platform"
spec_directory: "specifications/02-build-http-client"
feature_directory: "specifications/02-build-http-client/features/server-sent-events"
this_file: "specifications/02-build-http-client/features/server-sent-events/start.md"
created: 2026-03-03
---

# Start: Server-Sent Events Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. Read `../../LEARNINGS.md` (past discoveries and mistakes)
3. Read `./VERIFICATION.md` (verification requirements)
4. Read `./PROGRESS.md` (last progress of this feature if present)
5. Read `./ARCHITECTURE.md` (infrastructure analysis and design)
6. Read `.agents/AGENTS.md` to identify your agent type
7. Read your agent file in `.agents/agents/[agent-name].md`
8. Read skills specified in your agent documentation
9. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
10. Clear context, reload from `compacted.md` only, start work
11. **Work on ONE item at a time** - one test, one function, one file - finish it completely before next
12. Implement following TDD (test first, then code) - **one test at a time**
13. **Place tests in correct location** - follow language testing skill or project test structure
14. Report to Main Agent when done (DO NOT commit)
14. Wait for verification to pass
15. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task
16. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

**Workflow:** Feature.md → Learnings → Verification → ARCHITECTURE.md → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → **Update LEARNINGS.md** → Next

---

_Created: 2026-03-03_
