---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/04-native-host-and-dst"
this_file: "specifications/65-agent-rpc-serving-system/features/04-native-host-and-dst/start.md"
created: 2026-11-14
---

# Start: Native Host and DST Feature

## Agent Workflow

1. Read `feature.md` (tasks T17 and T18, what to read, "done when").
2. Read `hosts.md` (what each host gives the core, Cargo features, the native host and the
   native client).
3. Read `../03-agent-rpc-core/feature.md`, `proto.md` and `registry-and-models.md` (the core it
   hosts).
4. Read `../../agent-rpc-architecture.md` (shared design and owner decisions).
5. Read `../../../80-platform-prerequisites/features/00-env-and-side-effects/env.md` and
   `deterministic-simulation-testing.md` (the injected side effects and the simulation harness).
6. Read `../../../81-foundation-auth/requirements.md` (enrollment and mTLS).
7. Read `../../LEARNINGS.md`. Read `.agents/AGENTS.md` and the Rust skills. Generate
   `compacted.md` with `.agents/skills/context-compaction/skill.md`.
8. Work on one task at a time, test first. Tests live in
   `backends/foundation_agent_rpc/tests/`.
9. Report to the Main Agent. After the commit, update `../../progress.md` and
   `../../LEARNINGS.md`.

**Order:** feature 03 lands first. This feature makes the agent RPC proof of concept work.

---

_Created: 2026-11-14_
