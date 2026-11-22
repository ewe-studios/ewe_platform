---
workspace_name: "ewe_platform"
spec_directory: "specifications/84-context-injection"
this_file: "specifications/84-context-injection/start.md"
created: 2026-11-15
---

# Start: Context Injection

## Agent Workflow

1. Read `requirements.md` in full. It is the authority for the design and for the six tasks.
2. Read `env.md` section "Revision 2" first. The rest of the file is the original design; where
   it disagrees with `requirements.md`, `requirements.md` wins.
3. Read `deterministic-simulation-testing.md` for the reason that every effect is a trait.
4. Read `../76-shared-context/requirements.md` section 4. `Ctx` already exists; do not add a
   second context type.
5. Read `../80-platform-prerequisites/requirements.md`. It consumes this specification.
6. Read `.agents/skills/rust-valtron-usage/skill.md` before T2 and T3, and
   `.agents/skills/rust-clean-code/skill.md` before T4.
7. Read `.agents/AGENTS.md` and the agent file for the crate you touch.
8. Work on one task at a time, in the order of the next section.
9. Report to the Main Agent. Do not commit.
10. After the commit, update `progress.md` and `LEARNINGS.md`.

**Order:** T1 → T2 → T3 → T4 → T5 → T6. T1 and T2 can run together in one worktree. T3 needs
T2. T4 needs T2 and T3. T5 needs T2 and T3. T6 is last.

**Owned crates:** `foundation_context`, `foundation_core`, `foundation_macros`,
`foundation_ai`, `foundation_connectrpc` (only the deadline read), and the six `clippy.toml`
files. Do not change `foundation_auth`, `foundation_platform_sandbox` or `foundation_http` in
this specification; their specs record the dependency and do their own change.

**Workflow:** Requirements → Select task → Implement → Report → Verify → Commit → Update
progress

---

_Created: 2026-11-15_
