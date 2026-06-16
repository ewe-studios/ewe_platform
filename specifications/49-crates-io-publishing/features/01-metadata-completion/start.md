---
workspace_name: "ewe_platform"
spec_directory: "specifications/49-crates-io-publishing"
feature_directory: "specifications/49-crates-io-publishing/features/01-metadata-completion"
this_file: "specifications/49-crates-io-publishing/features/01-metadata-completion/start.md"
created: 2026-06-16
---

# Start: Metadata Completion Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks)
2. **Identify language stack** — Rust
3. **Read/generate language skills** — Read `.agents/skills/rust-clean-code/skill.md`
4. Read `../../LEARNINGS.md` (past discoveries and mistakes)
5. Read `./VERIFICATION.md` (verification requirements)
6. Read `./PROGRESS.md` (last progress if present)
7. Read `.agents/AGENTS.md` to identify your agent type
8. Read your agent file in `.agents/agents/[agent-name].md`
9. Read skills specified in your agent documentation
10. **MANDATORY**: Generate `compacted.md` with all info
11. Clear context, reload from `compacted.md` only, start work
12. **Work on ONE item at a time**
13. Implement following TDD — **one test at a time**
14. Report to Main Agent when done (DO NOT commit)
15. Wait for verification to pass
16. After commit: delete `compacted.md`, update `./PROGRESS.md`, move to next task
17. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

_Created: 2026-06-16_
