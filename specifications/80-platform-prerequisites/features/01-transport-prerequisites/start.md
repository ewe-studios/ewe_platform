---
workspace_name: "ewe_platform"
spec_directory: "specifications/80-platform-prerequisites"
feature_directory: "specifications/80-platform-prerequisites/features/01-transport-prerequisites"
this_file: "specifications/80-platform-prerequisites/features/01-transport-prerequisites/start.md"
created: 2026-11-14
---

# Start: Transport Prerequisites Feature

## Agent Workflow

1. Read `feature.md` (tasks T3 and T4).
2. Read `../../../76-shared-context/requirements.md` (`RpcCtx`) and the Env feature
   (`../00-env-and-side-effects/feature.md`).
3. Read `../../LEARNINGS.md`.
4. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
5. **Baseline first:** check whether spec 76 already moved the connectrpc clock. Write only the
   remaining delta.
6. Work on one task at a time, test first, tests in `tests/`.
7. Report, verify, commit; update `../../progress.md` and `../../LEARNINGS.md`.

**Done when:** a handler reads the request extensions and the peer TLS certificate from its
request context, and connectrpc deadlines use the Env clock.

---

_Created: 2026-11-14_
