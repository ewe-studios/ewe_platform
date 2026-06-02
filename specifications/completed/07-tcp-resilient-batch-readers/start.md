---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-tcp-resilient-batch-readers"
this_file: "specifications/07-tcp-resilient-batch-readers/start.md"
created: 2026-03-14
---

# Start: TCP-Resilient Batch Readers

## Agent Workflow

1. Read `requirements.md`
2. Read `LEARNINGS.md` (past discoveries and mistakes)
3. Read `progress.md` (current state)
4. Read `.agents/AGENTS.md` to identify your agent type
5. Read your agent file in `.agents/agents/[agent-name].md`
6. Read skills specified in your agent documentation
7. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
8. Clear context, reload from `compacted.md` only, start work
9. **Work on ONE item at a time** - one test, one function, one file - finish it completely before next
10. Implement following TDD (test first, then code) - **one test at a time**
11. **Place tests in correct location** - follow language testing skill or project test structure
12. Report to Main Agent when done (DO NOT commit)
12. Wait for verification to pass
13. After commit: delete `compacted.md`, update `progress.md`, move to next task

## Key Code to Read First

Before implementing, read these files to understand the patterns:

1. **Canonical pattern**: `backends/foundation_core/src/wire/websocket/frame.rs:188-221` — how WouldBlock/TimedOut are handled correctly with `read()` instead of `read_exact()`
2. **What to fix**: `backends/foundation_core/src/wire/simple_http/impls.rs:4964-5014` — current `SimpleHttpBody` using brittle `read_exact()`
3. **IO module structure**: `backends/foundation_core/src/io/mod.rs` — where to add the new `readers` module
4. **Read extensions**: `backends/foundation_core/src/io/stream_ext.rs` — existing Read extension patterns
5. **Error types**: `backends/foundation_core/src/wire/simple_http/errors.rs:365` — `HttpReaderError`

---

**Workflow:** Requirements → Learnings → Progress → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-03-14_
