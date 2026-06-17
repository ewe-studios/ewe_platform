---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/22b-documentstore-sql-async"
this_file: "specifications/36-agentic-api/features/22b-documentstore-sql-async/start.md"
created: 2026-06-17
---

# Start: DocumentStore SQL async backend (Turso/Libsql/D1)

## Agent Workflow

1. Read `feature.md` + F06 (`06-documentstore-trait-sql-memory`) — especially **OD-06-8** (async
   canonical, sync wraps via valtron; streams not `Vec`) and the sync `SqlDocumentStore` SQL.
2. **Stack:** Rust, native + wasm. Read `.agents/skills/rust-clean-code/skill.md`. Read the existing
   `AsyncQueryStore` impls (`turso_backend.rs`, `libsql_store.rs`, `d1_kvstore.rs`,
   `wasm/wasm_storage/d1_wasm.rs`), `AsyncQueryStream`, and the sync `SqlDocumentStore`.
3. Confirm F06 landed: `AsyncDocumentStore` + `AsyncStorageItemStream`, migrations 020/021 apply via the
   fixed `MigrationRunner`/`init_schema`, `Option<String>` NULL read-back.
4. Resolve OD-22b-1..4 before coding.
5. **One item at a time:** trait promotable parity → `AsyncSqlDocumentStore<Q>` → conformance (Turso
   async) → Libsql (sync+async) → wasm build.
6. Report; verify; update `../../LEARNINGS.md`; hand the D1 reuse to F23.

---

**Workflow:** feature.md → F06/OD-06-8 → AsyncQueryStore impls → Resolve OD-22b → ONE ITEM → Report → Verify → F23 reuse

---

_Created: 2026-06-17_
