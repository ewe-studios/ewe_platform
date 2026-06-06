---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/00-query-store-stream-parity/start.md"
created: 2026-06-06
---

# Start: Feature 00 — QueryStore Stream Parity

## Agent Workflow

1. Read `feature.md` in this directory
2. Read `requirements.md` in the parent directory for overall context
3. Read the valtron skill: `.agents/skills/rust-valtron-usage/skill.md`
4. Read the rust-clean-code skill: `.agents/skills/rust-clean-code/skill.md`
5. Read the current `AsyncQueryStore` trait: `backends/foundation_db/src/core/storage_provider.rs`
6. Read the `StreamAsFutureStream` type: `backends/foundation_core/src/valtron/stream_future.rs`
7. Implement `AsyncQueryStream` and update `AsyncQueryStore`
8. Update all backend implementations (Turso, libsql, D1 wasm)
9. Write tests
10. Run `cargo check` and fix any issues

---

**This is a preceding feature — must be complete before Features 10-12 (IdP models/services/handlers).**

_Created: 2026-06-06_
