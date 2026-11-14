---
workspace_name: "ewe_platform"
spec_directory: "specifications/83-agent-rpc-serving-system"
feature_directory: "specifications/83-agent-rpc-serving-system/features/05-worker-host-and-web-client"
this_file: "specifications/83-agent-rpc-serving-system/features/05-worker-host-and-web-client/start.md"
created: 2026-11-14
---

# Start: Worker Host and Web Client Feature

## Agent Workflow

1. Read `feature.md` (tasks T19 to T22 and T26, what to read, "done when").
2. Read `worker-and-web.md` (the Worker host, Durable Objects, the web client and the R2
   history archive).
3. Read `../03-agent-rpc-core/feature.md` and `registry-and-models.md` (the core it hosts).
4. Read `../04-native-host-and-dst/hosts.md` (what each host gives the core).
5. Read `../../../80-platform-prerequisites/features/01-transport-prerequisites/feature.md`
   (request extensions and TLS peer facts).
6. Read `../../agent-rpc-architecture.md` and `../../LEARNINGS.md`. Read `.agents/AGENTS.md` and
   the Rust skills. Generate `compacted.md` with `.agents/skills/context-compaction/skill.md`.
7. The Worker is single-threaded. Do not add valtron threading there.
8. Work on one task at a time, test first. Tests live in `backends/foundation_agent_rpc/tests/`
   and in the `cf-agent-rpc` example.
9. Report to the Main Agent. After the commit, update `../../progress.md` and
   `../../LEARNINGS.md`.

**Order:** feature 03 and spec 80 feature 01 land first. Feature 06 runs after this feature.

---

_Created: 2026-11-14_
