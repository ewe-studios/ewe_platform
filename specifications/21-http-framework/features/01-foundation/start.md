---
workspace_name: "ewe_platform"
spec_directory: "specifications/21-http-framework"
feature_directory: "specifications/21-http-framework/features/01-foundation"
this_file: "specifications/21-http-framework/features/01-foundation/start.md"
created: 2026-05-07
---

# Start: Foundation Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. **Identify language stack** from feature.md or parent requirements.md - document languages clearly
3. **Read/generate language skills** - for each language in stack:
   - If skill exists: `.agents/skills/[language]-clean-code/skill.md` → read it
   - If skill missing: launch agent to generate one first, then read it
   - Add this item to the workflow list so future agents remember
4. Read `../../LEARNINGS.md` (past discoveries and mistakes)
5. Read `./VERIFICATION.md` (verification requirements)
6. Read `./PROGRESS.md` (last progress of this feature if present)
7. Read `.agents/AGENTS.md` to identify your agent type
8. Read your agent file in `.agents/agents/[agent-name].md`
9. Read skills specified in your agent documentation
10. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
11. Clear context, reload from `compacted.md` only, start work
12. **Work on ONE item at a time** - one test, one function, one file - finish it completely before next
13. Implement following TDD (test first, then code) - **one test at a time**
14. **Place tests in correct location** - follow language testing skill or project test structure
15. Report to Main Agent when done (DO NOT commit)
16. Wait for verification to pass
17. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task

---

**Workflow:** Feature.md → **Language Stack → Skills** → Learnings → Verification → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-05-07_
