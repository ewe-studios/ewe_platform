# Feature 00: Host Bootstrap — Agent Workflow

## Steps

1. **Read `feature.md`** — understand requirements: install mise + nushell + pitchfork on the host machine
2. **Read spec-level `LEARNINGS.md`** (parent directory) — design decisions and past learnings
3. **Read AGENTS.md** and project skills:
   - `.agents/AGENTS.md`
   - `.agents/skills/rust-clean-code/skill.md`
   - `.agents/skills/rust-clean-code/implementation/skill.md`
4. **Generate `compacted.md`** for context optimization
5. **Clear context and reload**
6. **Work on ONE task at a time** from the Implementation Phases in `feature.md`
7. **Use TDD** — write tests before implementation for each task
8. **Report to Main Agent** when task is complete
9. **Wait for verification** before starting next task
10. **Delete `compacted.md`** after each commit

## Tasks to Implement

See "Implementation Phases" section in `feature.md`. Implement in order:
- Phase 1: Host bootstrap orchestrator, platform-specific installers, doctor check
- Phase 2: pitchfork integration for process management (replaces raw `std::process::Child`)
- Phase 3: nushell execution layer (unified `nu -c` for all VM-side commands)

## Reminders

- **No coding without reading the spec first**
- **Update LEARNINGS.md after each milestone**
- **feature.md is the single source of truth — never split architecture into separate files**
- **Use `foundation_core::wire::simple_http` for all HTTP — no curl subprocess, no reqwest**
