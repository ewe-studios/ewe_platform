---
workspace_name: "ewe_platform"
spec_directory: "specifications/33-valtron-singleton"
this_file: "specifications/33-valtron-singleton/start.md"
created: 2026-06-01
---

# Start: Valtron Single-Threaded Singleton

## Agent Workflow

1. Read `requirements.md`
2. Read language skill: `.agents/skills/rust-clean-code/skill.md`
3. Read existing code: `backends/foundation_core/src/valtron/executors/single/`
4. Implement directly from `requirements.md` — all implementation detail is there
5. **ALWAYS UPDATE LEARNINGS.md** after each completed task/milestone

## Dependencies

All types are already implemented in the codebase:
- `LocalThreadExecutor` — `foundation_core/src/valtron/local.rs`
- `PoolGuard` stub — `foundation_core/src/valtron/executors/single/pool_guard.rs`
- `NoThreadController`, `DefaultController` — `foundation_core/src/valtron/executors/single/mod.rs`
- `ExecutionTaskIteratorBuilder`, `SharedTaskQueue`, etc. — `foundation_core/src/valtron/`

---

**Workflow:** Requirements → Read deps → Rewrite PoolGuard + add ValtronSingleton → Verify

---

_Created: 2026-06-01_
