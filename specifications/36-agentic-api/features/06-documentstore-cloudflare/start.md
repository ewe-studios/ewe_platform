---
workspace_name: "ewe_platform"
spec_directory: "specifications/36-agentic-api"
feature_directory: "specifications/36-agentic-api/features/06-documentstore-cloudflare"
this_file: "specifications/36-agentic-api/features/06-documentstore-cloudflare/start.md"
created: 2026-06-14
---

# Start: DocumentStore Cloudflare D1 + KV

## Agent Workflow

1. Read `feature.md` + Decision 13 (CF backends).
2. **Stack:** Rust + wasm/CF Workers. Read `.agents/skills/rust-clean-code/skill.md`. Read the
   existing D1 bindings (`foundation_db/src/wasm/workers_rs/d1.rs`, `wasm/wasm_storage/d1_wasm.rs`)
   and the CF KV binding; the `AsyncDocumentStore` trait (`storage_provider.rs:514`).
3. Confirm F04 landed (`scan_from_async` signature, doc_id=scru128, `020`/`021` schema).
4. Read `../../LEARNINGS.md`. Resolve OD-06-1..5 before coding.
5. Generate `compacted.md`, clear, reload.
6. **One item at a time:** D1 backend (reuse SQL) → KV backend (key-list) → scan_from_async each.
7. Report; verify; update `../../LEARNINGS.md`; move to Feature 07.

---

**Workflow:** feature.md → Decision 13 → D1/KV bindings → Resolve OD-06 → Compact → ONE ITEM → Report → Verify → 07

---

_Created: 2026-06-14_
