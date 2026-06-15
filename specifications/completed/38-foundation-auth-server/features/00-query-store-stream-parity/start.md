---
workspace_name: "ewe_platform"
spec_directory: "specifications/38-foundation-auth-server"
this_file: "specifications/38-foundation-auth-server/features/00-query-store-stream-parity/start.md"
created: 2026-06-06
---

# Start: Feature 00 — Storage Trait Stream Parity

## Agent Workflow

1. Read `feature.md` in this directory
2. Read `requirements.md` in the parent directory for overall context
3. Read the valtron skill: `.agents/skills/rust-valtron-usage/skill.md`
4. Read the rust-clean-code skill: `.agents/skills/rust-clean-code/skill.md`

5. **Read current implementations:**
   - `backends/foundation_db/src/native/turso_backend.rs`
   - `backends/foundation_db/src/native/libsql_store.rs`
   - `backends/foundation_db/src/native/d1_kvstore.rs`
   - `backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs`
   - `backends/foundation_db/src/native/rows_stream.rs`
   - `backends/foundation_db/src/core/storage_provider.rs`

6. **Implement:**
   - Add `AsyncQueryStream` and `AsyncListStream` types
   - Update `AsyncQueryStore` and `AsyncKeyValueStore` traits
   - Turso: add async stream generators, update both traits, rewrite sync to wrap async
   - Libsql: same pattern
   - D1Store: update to return buffered streams
   - D1Wasm: update return types

7. **Run tests** — existing tests must pass, add parity tests

8. **Verify:** `cargo check 2>&1 | tee /tmp/cargo-check.log` (5-6 min timeout)

---

**This is a preceding feature — must be complete before Features 10-12 (IdP models/services/handlers).**

_Created: 2026-06-06_
