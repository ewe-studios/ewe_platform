---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-foundation-errstacks"
this_file: "specifications/08-foundation-errstacks/start.md"
created: 2026-04-12
---

# Start: Foundation ErrStacks

## Agent Workflow

1. Read `requirements.md` (complete requirements, architecture, API, and tasks)
2. **Identify language stack** from requirements.md "Language Stack" section
3. **Read/generate language skills** - for each language in stack:
   - If skill exists: `.agents/skills/[language]-clean-code/skill.md` → read it
   - If skill missing: launch an agent to generate one first, then read it
4. Read `../../LEARNINGS.md` (workspace-wide learnings, if present)
5. Read `./LEARNINGS.md` (spec-specific learnings, if present)
6. Read `./PROGRESS.md` (last progress for this spec, if present)
7. Read `./VERIFICATION.md` (verification requirements, if present)
8. Read `.agents/AGENTS.md` to identify your agent type
9. Read your agent file in `.agents/agents/[agent-name].md`
10. Read skills specified in your agent documentation
11. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
12. Clear context, reload from `compacted.md` only, start work
13. **Work on ONE item at a time** - one test, one function, one file - finish it completely before next
14. Implement following TDD (test first, then code) - **one test at a time**
15. **Place tests in correct location** - tests live in `tests/` directory per Rust clean-code skill
16. Report to Main Agent when done (DO NOT commit)
17. Wait for verification to pass
18. After commit: delete `compacted.md`, update `PROGRESS.md`, move to next task

---

**Workflow:** Requirements → **Language Stack → Skills** → Learnings → Verification → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

## Quick Reference

- **Target crate:** `backends/foundation_errstacks`
- **MSRV:** 1.81.0 (required for `core::error::Error`)
- **Primary dependency:** `derive_more` (with `default-features = false`)
- **Features:** `alloc` (baseline), `std` (default), `serde`, `backtrace` (requires `std`), `async`, `slack`
- **Phases:** 6 (Core Types → Formatting → Serialization → Testing → Integration → no_std Support)
- **Total tasks:** 28

---

_Created: 2026-04-12_
