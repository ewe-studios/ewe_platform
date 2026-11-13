---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/08-key-protection-at-rest"
this_file: "specifications/65-agent-rpc-serving-system/features/08-key-protection-at-rest/start.md"
created: 2026-11-14
---

# Start: Key Protection at Rest Feature

## Agent Workflow

1. Read `feature.md` (tasks K1 to K6, what to read, "done when"). The feature is `todo` and runs
   last.
2. Read `keystore.md` (the key store and the sealed key records) and `key-protection.md` (the
   KEK sources, WebCrypto, envelope encryption and rotation).
3. Read `../04-native-host-and-dst/hosts.md` (the native host and the Worker host key paths).
4. Read `../../../81-foundation-auth/features/00-foundation-auth/auth.md` (the auth primitives
   that this feature extends).
5. Read `../../agent-rpc-architecture.md` (owner decision 11 and the deferred key work).
6. Read `../../LEARNINGS.md`. Read `.agents/AGENTS.md` and the Rust skills. Generate
   `compacted.md` with `.agents/skills/context-compaction/skill.md`.
7. Work on one task at a time, test first. Tests live in
   `backends/foundation_auth/tests/` and `backends/foundation_agent_rpc/tests/`.
8. Report to the Main Agent. After the commit, update `../../progress.md` and
   `../../LEARNINGS.md`.

**Order:** feature 04 must work end to end first. This feature is the last of spec 65.

---

_Created: 2026-11-14_
