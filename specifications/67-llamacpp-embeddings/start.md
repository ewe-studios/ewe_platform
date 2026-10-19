---
workspace_name: "ewe_platform"
spec_directory: "specifications/64-llamacpp-embeddings"
this_file: "specifications/64-llamacpp-embeddings/start.md"
created: 2026-10-18
---

# Start: LlamaCpp Embeddings

## Agent Workflow

1. Read `requirements.md` (complete requirements + tasks).
2. **Identify language stack** — this spec is Rust-only.
3. **Read the Rust skills**: `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md` (valtron boundaries apply to the stream/agentic layer).
4. Read `LEARNINGS.md` (past discoveries and mistakes).
5. Read `VERIFICATION.md` (verification requirements, if present).
6. Read `.agents/AGENTS.md` to identify your agent type.
7. Read your agent file in `.agents/agents/[agent-name].md`.
8. Read skills specified in your agent documentation.
9. **MANDATORY**: Generate `compacted.md` with all info using
   `.agents/skills/context-compaction/skill.md`.
10. Clear context, reload from `compacted.md` only, start work.
11. **Work on ONE item at a time** — one test, one function, one file — finish it completely before next.
12. Implement following TDD (test first, then code) — **one test at a time**.
13. **Place tests in correct location** — integration test under
    `backends/foundation_ai/tests/harness/integrations/`, following `mtp_generate.rs`.
14. Report to Main Agent when done (DO NOT commit).
15. Wait for verification to pass.
16. After commit: delete `compacted.md`, update `PROGRESS.md`, move to next task.
17. **ALWAYS UPDATE LEARNINGS.md** after each completed task/milestone.

---

**Workflow:** Requirements → **Language Stack → Skills** → Learnings → Verification → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

_Created: 2026-10-18_
