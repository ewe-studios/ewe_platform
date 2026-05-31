---
workspace_name: "ewe_platform"
spec_directory: "specifications/32-cf-serve-app"
this_file: "specifications/32-cf-serve-app/start.md"
created: 2026-05-31
---

# Start: CfServe / CfHttpApp Entry Point

## Agent Workflow

1. Read `requirements.md`
2. Read language skill: `.agents/skills/rust-clean-code/skill.md`
3. Read `LEARNINGS.md` (empty for new spec)
4. Implement directly from `requirements.md` — all implementation detail is there
5. **ALWAYS UPDATE LEARNINGS.md** after each completed task/milestone

## Dependencies

This spec depends on completed features from [spec 28](../28-cloudflare-workers-readiness/requirements.md):
- `CfServe`, `CfConn`, `CfHttpApp` — `foundation_http/src/wasm/bridge/cf.rs`
- `HttpApp<Arc<dyn CfServe>>` — `foundation_http/src/shared/app/mod.rs`
- `CfHttpAppDispatch` — `foundation_http/src/wasm/dispatch.rs`
- `D1Database::from_env` — `foundation_db/src/wasm/bindgen/cf/d1.rs`

All dependencies are already implemented and compiling.

---

**Workflow:** Requirements → Read deps → Implement fetch() + CfHttpAppSingleton → Verify

---

_Created: 2026-05-31_
