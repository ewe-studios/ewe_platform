---
workspace_name: "ewe_platform"
spec_directory: "specifications/49-crates-io-publishing"
feature_directory: "specifications/49-crates-io-publishing/features/00-crate-names-and-publish-plan"
this_file: "specifications/49-crates-io-publishing/features/00-crate-names-and-publish-plan/start.md"
created: 2026-06-16
---

# Start: Crate Names & Publish Plan Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. **Identify language stack** from feature.md or parent requirements.md — Rust
3. **Read/generate language skills** — for Rust:
   - Read `.agents/skills/rust-clean-code/skill.md`
   - Add this item to the workflow list so future agents remember
4. Read `../../LEARNINGS.md` (past discoveries and mistakes)
5. Read `./VERIFICATION.md` (verification requirements)
6. Read `./PROGRESS.md` (last progress of this feature if present)
7. Read `.agents/AGENTS.md` to identify your agent type
8. Read your agent file in `.agents/agents/[agent-name].md`
9. Read skills specified in your agent documentation
10. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
11. Clear context, reload from `compacted.md` only, start work
12. **Work on ONE item at a time** — one test, one function, one file — finish it completely before next
13. Implement following TDD (test first, then code) — **one test at a time**
14. **Place tests in correct location** — follow language testing skill or project test structure
15. Report to Main Agent when done (DO NOT commit)
16. Wait for verification to pass
17. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task
18. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

**Workflow:** Requirements → **Language Stack → Skills** → Learnings → Verification → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-06-16_
