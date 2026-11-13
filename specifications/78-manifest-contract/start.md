---
workspace_name: "ewe_platform"
spec_directory: "specifications/78-manifest-contract"
this_file: "specifications/78-manifest-contract/start.md"
created: 2026-11-14
---

# Start: Manifest Contract

## Agent Workflow

1. Read `requirements.md` (the format, the rules, the fixtures and tasks M1 to M4).
2. Read `../../.agents/skills/rust-clean-code/skill.md` only if the task adds the drift script
   in Rust. The schema and fixtures are JSON and Python.
3. Read `LEARNINGS.md` (why the contract is separate from the installer).
4. Read `.agents/AGENTS.md` and your agent file.
5. Generate `compacted.md` with `.agents/skills/context-compaction/skill.md`.
6. Work on one task at a time. Add the schema first, then the fixtures, then the drift check.
7. Report to the Main Agent. Do not commit.
8. After the commit, update `progress.md` and `LEARNINGS.md`.

**Order:** this specification runs before 79 and 82. Do not change a consumer parser here; the
consumer specs own that work.

**Workflow:** Requirements → Select task → Implement → Report → Verify → Commit → Update progress

---

_Created: 2026-11-14_
