---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/20-agent-session-api"
this_file: "specifications/36-agentic-api/features/20-agent-session-api/start.md"
created: 2026-06-14
---

# Start: Agent Session API + Resume Protocol

## Agent Workflow

1. Read `feature.md` + Decision 18 (note the user TODOs: REJECTED `tools(vec![])` → require a ToolShed;
   `Arc<dyn ModelProvider>` impossible → require a ProviderRouter F12) + Decision 01 (exact resume order).
   Read F08 `recent`/`flush`/`semantic_search`, F16 assemble, F15 `latest_*`, F13 `drain_*`, F19
   `AgentLoop`.
2. **Stack:** Rust + valtron. Read `.agents/skills/rust-clean-code/skill.md` + `rust-valtron-*`.
   ids = `foundation_compact` (the RNG fold). `end()` flushes synchronously (no valtron join).
3. Read `../../LEARNINGS.md`. Resolve OD-20-1..5. **OD-20-2 (preflight before scheduling) is the user's
   hard requirement** — verify tools-registered + access + budget fail BEFORE `execute`.
4. Generate `compacted.md`, clear, reload.
5. **One item at a time:** builder (router+toolshed required) → build+wire SessionInner → preflight →
   run_turn/run_turn_stream → steer/follow_up → resume (Decision 01 order) → end (teardown).
6. **Author `fundamentals/` docs.**
7. Report; verify (native+wasm); update `../../LEARNINGS.md`; move to Feature 21.

---

**Workflow:** feature.md → Decision 18 (corrected) + Decision 01 → F08/F16/F15/F12/F13/F19 → Resolve OD-20 (flag 2) → Compact → ONE ITEM → fundamentals → Report → Verify → 32

---

_Created: 2026-06-14_
