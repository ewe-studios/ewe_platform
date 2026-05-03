# Feature 09: Project Mount & Artifact Layer — Agent Workflow

## Steps

1. **Read `feature.md`** — understand requirements: host directory mounting via 9p/virtiofs (QEMU) or shared directories (UTM), artifact mirroring, testbed.toml parsing
2. **Read spec-level `LEARNINGS.md`** (parent directory) — design decisions and past learnings
3. **Read spec-level `requirements.md`** (parent directory) — overall architecture context
4. **Read AGENTS.md** and project skills:
   - `.agents/AGENTS.md`
   - `.agents/skills/rust-clean-code/skill.md`
   - `.agents/skills/rust-clean-code/implementation/skill.md`
5. **Generate `compacted.md`** for context optimization
6. **Clear context and reload**
7. **Work on ONE task at a time** from the Implementation Phases in `feature.md`
8. **Use TDD** — write tests before implementation for each task
9. **Report to Main Agent** when task is complete
10. **Wait for verification** before starting next task
11. **Delete `compacted.md`** after each commit

## Tasks to Implement

See "Implementation Phases" section in `feature.md`. Implement in order:
- Phase 1: Config parsing — `TestbedConfig` from `.testbed/testbed.toml`, `cmd_init()` handler
- Phase 2: QEMU 9p mount — mount args builder, guest auto-mount via fstab, build integration
- Phase 3: UTM shared directories — AppleScript config, guest mount verification
- Phase 4: Artifacts — scanning, mirroring, symlink management, CLI mount commands

## Reminders

- **No coding without reading the spec first**
- **Update LEARNINGS.md after each milestone**
- **feature.md is the single source of truth — never split architecture into separate files**
- **testbed.toml `[[vms]]` is an array of tables — each entry is an independent VM**
- **Use `foundation_core::wire::simple_http` for all HTTP — no curl subprocess, no reqwest**
