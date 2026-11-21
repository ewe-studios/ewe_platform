---
workspace_name: "ewe_platform"
spec_directory: "specifications/completed/80-platform-prerequisites"
feature_directory: "specifications/completed/80-platform-prerequisites/features/00-transport-prerequisites"
this_file: "specifications/completed/80-platform-prerequisites/features/00-transport-prerequisites/start.md"
created: 2026-11-14
---

# Start: Transport Prerequisites Feature

## Agent Workflow

1. Read `feature.md` (tasks T3 and T4).
2. Read `../../../76-shared-context/requirements.md` (`RpcCtx`, `Clock`) and
   `../../../84-context-injection/requirements.md` section 3 (the `Env` bundle). The clock is
   `ctx.clock()`; the `Env` of spec 84 has no clock of its own.
3. Read `../../../87-mutual-tls/requirements.md` section 4.2 and `mtls.md` section 3.4. They
   are the consumers of the peer facts that T4 adds.
4. Read `../../LEARNINGS.md`.
5. Read `.agents/AGENTS.md`, `.agents/skills/rust-clean-code/skill.md` and
   `.agents/skills/rust-valtron-usage/skill.md`.
6. **Baseline first:** check whether spec 76 already moved the connectrpc clock. Write only the
   remaining delta.
7. Work on one task at a time, test first, tests in `tests/`.
8. Report, verify, commit; update `../../progress.md` and `../../LEARNINGS.md`.

**Done when:** a handler reads the request extensions and the peer TLS certificate from its
request context, and connectrpc deadlines use the `Ctx` clock.

---

_Created: 2026-11-14_
_Revised: 2026-11-15 (directory renumbered to 00; the Env design moved to spec 84)._
