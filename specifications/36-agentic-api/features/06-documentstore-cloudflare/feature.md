---
feature: "DocumentStore: Cloudflare D1 + KV (AsyncDocumentStore)"
description: "Implement AsyncDocumentStore for Cloudflare D1 (SQL, async) and Cloudflare KV (key-list), with scan_from_async, reusing the existing D1 bindings — completing the DocumentStore backend set for serverless/wasm"
status: "pending"
priority: "medium"
depends_on: ["04-documentstore-trait-sql-memory"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Feature 06: DocumentStore — Cloudflare D1 + KV

> **Review status (2026-06-14) — re-scoped (major gaps found).** The CF bindings are weaker than the
> draft assumed: (1) `scan_from_async` does **not** exist on `AsyncDocumentStore` yet — F04 must add
> it first (hard dep). (2) The **KV** binding (`wasm/bindgen/cf/kv.rs`) passes only `prefix` —
> no `cursor`/`start` wired, caps at **1000 keys/list**, discards the cursor, and is **eventually
> consistent** (no read-after-write) → KV cannot honor the strict ordered `scan_from`/parity
> contract without binding work, and even then is best-effort. (3) **D1** has no `exec()`/batch — only
> `prepare().run()` single statements — so the multi-statement `020`/`021` migrations won't apply
> as-is. (4) Migration `020` isn't registered + `021` doesn't exist (F04's job). (5) a pre-existing
> `AsyncQueryStore` trait/impl signature mismatch + unconfirmed wasm-bindgen-storage compile. D1 is
> the viable ordered backend; KV is downgraded to best-effort. See OD-06-6..10.

> Implements Decision 13's Cloudflare backends via the **`AsyncDocumentStore`** trait (KV/D1 are
> Promise-based, async-only on wasm). No `AsyncDocumentStore` impl exists yet (verified). Reuses the
> existing CF D1 bindings (`wasm/workers_rs/d1.rs`, `wasm/wasm_storage/d1_wasm.rs`). Completes the
> DocumentStore backend set begun in F04 (SQL+Memory) and F05 (VFS).

## WHY: Problem Statement

For serverless/CF Workers deployments, sessions persist to Cloudflare D1 (SQLite-compatible) or KV.
Both are **async-only** in the Workers runtime, so they implement `AsyncDocumentStore`
(`storage_provider.rs:514`), not the sync `DocumentStore`. The trait + the `scan_from_async`
signature come from F04; F06 supplies the CF implementations. D1 bindings already exist; KV needs the
key-list document pattern from Decision 13.

## WHAT: Solution

### D1 backend (SQL, async) — `D1DocumentStore`

D1 is SQLite-compatible, so this mirrors `SqlDocumentStore` over the async D1 binding:

- Reuse the **same `documents` schema** (F04's migration `020`+`021`, incl. promoted columns) — D1
  runs the same SQL.
- `append_async`/`scan_async`/`scan_all_async`/`scan_from_async`/`delete_async`/`count_async` via the
  existing D1 query binding (`wasm/workers_rs/d1.rs`).
- `scan_from_async`: `WHERE collection_key=? AND doc_id>=? ORDER BY doc_id ASC LIMIT ?` — same as SQL
  (F04 doc_id ordering), returning `Vec<V>` (async trait returns Vec, not a stream).
- D1 migrations: ensure F04's `020`/`021` are applied in the D1 schema-init path (OD-06-1).

### KV backend (key-list) — `KvDocumentStore`

CF KV is pure key-value; documents use the Decision 13 layout:

| Key | Value |
|-----|-------|
| `doc:{collection}:{doc_id}` | JSON document |
| `list:{collection}` | ordered index of doc_ids (or rely on KV `list({prefix})`) |

- **`append_async`:** `kv.put("doc:{c}:{id}", json)`; doc_id = scru128 (F04). Because scru128 sorts
  chronologically and KV `list(prefix)` returns keys **lexicographically**, `list("doc:{c}:")` is
  already time-ordered — so a separate `list:` key may be unnecessary (OD-06-2).
- **`scan_from_async`:** `kv.list({ prefix: "doc:{c}:", start: "doc:{c}:{from_id}" })` → take `limit`
  → bulk `get` each. KV `list` supports a cursor/`start` for range.
- **`scan_async`(last N):** KV `list` is ascending only; for "last N" either keep a reverse index or
  `list` all + take tail (bounded by collection size — OD-06-3).
- **Promoted columns:** KV can't index by column; store `record_type`/`title` in the value and filter
  client-side, or encode `record_type` into the key (`doc:{c}:{type}:{id}`) for prefix filtering
  (OD-06-4).

### Platform gating

Both are `#[cfg(target_arch = "wasm32")]` (CF Workers) and behind the relevant `foundation_db` wasm
feature. They implement `AsyncDocumentStore` (`?Send`, per the trait's `async_trait(?Send)`).

## Architecture

```mermaid
graph TD
    AS[AsyncDocumentStore trait - F04] --> D1[D1DocumentStore: SQL over D1 binding]
    AS --> KV[KvDocumentStore: doc:{c}:{scru128} + list cursor]
    D1 --> SCH[(documents schema 020+021)]
    KV --> CFKV[(CF KV: lexicographic list = chronological via scru128)]
```

## HOW: Implementation Steps

1. `D1DocumentStore` impl `AsyncDocumentStore` over `wasm/workers_rs/d1.rs`; reuse `documents` schema;
   ensure `020`/`021` migrations run in the D1 init path.
2. `scan_from_async` (D1): range query, `Vec<V>`.
3. `KvDocumentStore` impl `AsyncDocumentStore` over the CF KV binding; `doc:{c}:{scru128}` keys.
4. `scan_from_async` (KV): `list` with `start` cursor + bulk get.
5. Resolve "last N" on KV (OD-06-3) and promoted-field handling (OD-06-4).
6. Tests: against D1/KV bindings (gated; may need miniflare/worker test harness or mock bindings) —
   append→scan_from ordering parity with SQL/Memory; KV list-cursor correctness.

## Open Decisions

- **OD-06-1 — D1 migration application:** how/where F04's `020`/`021` run on D1 (schema-init path,
  one-time). Confirm the D1 binding exposes batch DDL.
- **OD-06-2 — KV: separate `list:` index or rely on `list(prefix)`:** scru128 keys make
  `list(prefix)` chronological → a separate index is likely unnecessary. Rec: rely on `list`.
- **OD-06-3 — KV "last N":** reverse-index key vs `list`+tail. Rec: for agent sessions (bounded),
  `list`+tail is acceptable; revisit if collections grow large.
- **OD-06-4 — KV promoted fields:** value-stored + client filter vs `record_type` in the key. Rec:
  value-stored now; key-encode only if typed scans dominate.
- **OD-06-5 — testing harness:** real miniflare/wrangler vs mock KV/D1 bindings. Rec: mock bindings
  for unit parity; integration behind an opt-in flag.
- **OD-06-6 (D1 DDL) — needs binding work:** D1 has only `prepare().run()` (single statement), no
  `exec()`/batch (`bindgen/cf/d1.rs`). The multi-statement `020`/`021` migrations won't apply. Add a
  D1 `exec()` binding (CF `D1Database.exec`) OR split migrations into per-statement `run()`s. **Plus
  register `020`+`021` and ensure `init_schema_async` runs the documents schema on D1 (today it only
  creates the KV table).**
- **OD-06-7 (KV pagination) — needs binding work:** the KV `list` wrapper passes only `prefix`,
  ignores the returned `cursor`, and caps at 1000 keys (`kv_wasm.rs:202-243`). Extend it to loop on
  `cursor`/`list_complete` and accept `limit` before any `scan_*` is correct.
- **OD-06-8 (KV consistency) — relax parity:** CF KV is eventually consistent (no read-after-write)
  and transactionless. So KV **cannot** honor the strict ordered `scan_from`/parity Done-When. KV is
  **best-effort**; D1 is the strictly-ordered CF backend. Document the divergence (don't claim KV
  parity with SQL/Memory/VFS).
- **OD-06-9 (KV range) — skip-until:** CF KV `list` has no native `start`/`startAfter` for arbitrary
  range — only `prefix`+`cursor`. So `scan_from(from_id)` on KV is a **client-side skip-until-from_id**
  while paginating, not a native range seek.
- **OD-06-10 — pre-existing mismatch:** `AsyncQueryStore::query_async` is declared
  `-> AsyncQueryStream` but D1 impls `-> Vec<SqlRow>` (`d1_wasm.rs:606`); confirm the
  wasm-bindgen-storage path actually compiles for wasm32 before building `D1DocumentStore` on it.
- **OD-06-11 — `?Send`:** `AsyncDocumentStore` is `async_trait(?Send)` and `StorageItemStream` is
  non-`Send` on wasm; reconcile with the mostly-`Send` agentic layer (how a `Send` caller drives a
  `!Send` future on Workers).

## Target Files

- `backends/foundation_db/src/wasm/` — `d1_document_store.rs`, `kv_document_store.rs` (new)
- reuse `wasm/workers_rs/d1.rs`, the CF KV binding; `documents` schema from F04
- coordinates with F04 (`AsyncDocumentStore` + `scan_from_async` + doc_id ordering + promoted columns)

## Tests

```bash
cargo build -p foundation_db --target wasm32-unknown-unknown   # (with CF features)
# unit parity via mock bindings:
cargo test -p foundation_db -- document_store::cf
```

## Verification

```bash
cargo build -p foundation_db --target wasm32-unknown-unknown --features <cf-features>
cargo clippy -p foundation_db --target wasm32-unknown-unknown --features <cf-features> -- -D warnings
cargo test  -p foundation_db -- document_store::cf
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: the Cloudflare Workers runtime & storage model; **D1** (SQLite at
the edge, prepared statements, no batch DDL); **KV** (eventual consistency, list pagination/cursors,
1000-key caps, no transactions); read-after-write hazards; async `?Send` bindings on wasm; when KV is
"best-effort" vs D1 "ordered". (Task — see list.)

## Done When

- `D1DocumentStore` + `KvDocumentStore` implement `AsyncDocumentStore` incl. `scan_from_async`, with
  ordering parity (doc_id/scru128) with the SQL/Memory/VFS backends.
- D1 reuses the `documents` schema (020+021); KV uses the `doc:{c}:{scru128}` layout.
- Builds for `wasm32` under the CF features; native unaffected.
- OD-06-1..5 resolved.
