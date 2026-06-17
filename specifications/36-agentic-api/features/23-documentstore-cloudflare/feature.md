---
feature: "DocumentStore: Cloudflare D1 + KV (AsyncDocumentStore)"
description: "Async-first AsyncDocumentStore for Cloudflare: D1 (SQLite, the primary ordered backend) + R2 for large document blobs (e.g. big Message-API records that don't fit SQLite), with KV as an optional best-effort key-value path. scan_from_async; the sync DocumentStore is a valtron wrapper over async. Completes the DocumentStore backend set for serverless/wasm"
status: "pending"
priority: "medium"
depends_on: ["06-documentstore-trait-sql-memory", "22b-documentstore-sql-async"]
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

# Feature 23: DocumentStore — Cloudflare D1 + KV

> **RESOLVED (user, 2026-06-15) — design is the target; build/fix/rebuild reality to match. No
> half-assed work.** The spec states the end desire; where the current code doesn't meet it we build it
> properly, rebuilding from scratch if that's what getting it right takes. If a CF capability is missing
> (D1 batch DDL, KV cursor pagination), **we add it** — we own these bindings; we don't declare "this
> won't work." But we **select tooling sensibly** to what each store is actually good at.
>
> **RESOLVED — async-first, sync wraps async via valtron (house rule).** Build for the **async world**:
> the canonical implementation is **`AsyncDocumentStore`** (`*_async` methods). The **sync `DocumentStore`
> is a thin valtron wrapper** around the async impl (the same pattern used for every other async trait in
> the platform). If `AsyncDocumentStore` is missing, F06 adds the trait; F23 supplies CF impls. So we
> have two traits — `DocumentStore` (sync) and `AsyncDocumentStore` (async) — and async-first means
> everything just works where needed, with valtron bridging to sync where sensible.
>
> **RESOLVED — use the right CF API per job; D1+KV are NOT both mandatory.** foundation_db's CF layer
> supporting both KV and D1 does not mean F23 must use both. Plan:
> - **D1 (SQLite, full relational) is the primary ordered backend** — it satisfies the strict
>   `scan_from`/ordering contract. Even if F23 ships **only D1**, that's a complete win.
> - **R2 for large documents** — Messages persisted by the Message API (F08) can be large and don't fit
>   SQLite well. Store the big blob in **R2** (object storage), keyed by doc_id, with D1 holding the
>   row + promoted columns + the R2 key. (New `R2DocumentStore` / R2-backed blob path.)
> - **KV only where quick key-value lookup genuinely helps** — best-effort (eventually consistent), not
>   the ordered backbone. Optional.

> **Review status (2026-06-14) — re-scoped (major gaps found).** The CF bindings are weaker than the
> draft assumed: (1) `scan_from_async` does **not** exist on `AsyncDocumentStore` yet — F06 must add
> it first (hard dep). (2) The **KV** binding (`wasm/bindgen/cf/kv.rs`) passes only `prefix` —
> no `cursor`/`start` wired, caps at **1000 keys/list**, discards the cursor, and is **eventually
> consistent** (no read-after-write) → KV cannot honor the strict ordered `scan_from`/parity
> contract without binding work, and even then is best-effort. (3) **D1** has no `exec()`/batch — only
> `prepare().run()` single statements — so the multi-statement `020`/`021` migrations won't apply
> as-is. (4) Migration `020` isn't registered + `021` doesn't exist (F06's job). (5) a pre-existing
> `AsyncQueryStore` trait/impl signature mismatch + unconfirmed wasm-bindgen-storage compile. D1 is
> the viable ordered backend; KV is downgraded to best-effort. See OD-23-6..10.

> Implements Decision 13's Cloudflare backends **async-first** via the **`AsyncDocumentStore`** trait
> (D1/KV/R2 are Promise-based, async-only on wasm); the sync `DocumentStore` is a valtron wrapper over
> the async impl. **The D1 path is NOT hand-rolled here — D1 is SQLite, so it reuses
> `AsyncSqlDocumentStore<D1WasmStorage>` from F22b** (same generic that serves native Turso/Libsql);
> this feature adds only the CF-specific glue: **R2 for large document blobs** that don't fit SQLite,
> bindings, and deployment. As of F06 the only `AsyncDocumentStore` impl is the in-memory one; F22b adds
> the SQL one; both pass the shared scan/scan_from/promoted-column conformance. **D1 is the primary ordered
> backend; R2 stores large document blobs; KV is optional best-effort.** Reuses the existing CF D1
> bindings (`wasm/workers_rs/d1.rs`, `wasm/wasm_storage/d1_wasm.rs`) and adds an R2 binding/path where
> missing. Completes the DocumentStore backend set begun in F06 (SQL+Memory) and F22 (VFS/Fjall).

## WHY: Problem Statement

For serverless/CF Workers deployments, sessions persist to Cloudflare. The Workers runtime is
**async-only**, so the canonical impl is **`AsyncDocumentStore`** (`storage_provider.rs:514`); the sync
`DocumentStore` is a valtron wrapper over it. The trait + the `scan_from_async` signature come from F06;
F23 supplies the CF implementations.

**Tool selection (right API per job):** **D1** (SQLite, relational, ordered) is the **primary** backend
and satisfies the strict `scan_from`/ordering contract. **R2** (object storage) holds **large document
blobs** — Message-API records (F08) can exceed what fits comfortably in a SQLite row, so the big payload
lives in R2 keyed by doc_id while D1 keeps the row + promoted columns + the R2 key. **KV** is an
**optional best-effort** key-value path (eventually consistent), used only where quick KV lookups help —
never the ordered backbone. Shipping D1 (+ R2 for large blobs) alone is a complete deliverable; KV is
additive.

## WHAT: Solution

### D1 backend (SQL, async) — `D1DocumentStore` — **PRIMARY**

D1 is SQLite-compatible, so this mirrors `SqlDocumentStore` over the async D1 binding and is the
strictly-ordered CF backend:

- Reuse the **same `documents` schema** (F06's migration `020`+`021`, incl. promoted columns) — D1
  runs the same SQL.
- `append_async`/`scan_async`/`scan_all_async`/`scan_from_async`/`delete_async`/`count_async` via the
  existing D1 query binding (`wasm/workers_rs/d1.rs`).
- `scan_from_async`: `WHERE collection_key=? AND doc_id>=? ORDER BY doc_id ASC LIMIT ?` — same as SQL
  (F06 doc_id ordering), returning `Vec<V>` (async trait returns Vec, not a stream).
- D1 migrations: ensure F06's `020`/`021` are applied in the D1 schema-init path. D1 lacks batch DDL —
  **add a D1 `exec()` binding or split migrations into per-statement `run()`s** (OD-23-1/OD-23-6); we own
  the binding, so we add the capability rather than work around it.
- **Large-blob offload to R2:** when a document exceeds a size threshold, D1 stores the row + promoted
  columns + an `r2_key`, and the blob goes to R2 (below). Small documents stay inline in D1.

### R2 backend (large blobs, async) — `R2DocumentStore` / R2 offload

CF **R2** is S3-style object storage for payloads too large for a SQLite row (e.g. big Message-API
records, F08):

- **`append_async`:** `r2.put("doc/{collection}/{doc_id}", json_or_bytes)`; doc_id = scru128 (F06).
- **`get`/`scan_from_async`:** R2 has `list({ prefix, cursor, startAfter })` returning
  lexicographically-ordered keys — scru128 keys are chronological, so `list("doc/{c}/")` + `startAfter`
  gives ordered range reads; fetch each object by key. Ordering metadata (doc_id list, promoted columns)
  is best kept in **D1** so range/typed queries stay on the relational backend and R2 holds only the
  bytes (the recommended split — OD-23-12).
- **Standalone vs offload:** R2 can back a store on its own, but the **recommended** topology is **D1 as
  the index/metadata + R2 as the blob store** for large records, transparent behind one
  `AsyncDocumentStore`.

### KV — **NOT a DocumentStore backend** (resolved, user 2026-06-15)

CF KV is eventually consistent (no read-after-write), has no transactions, no native range queries,
and caps `list` at 1000 keys with no `startAfter`. It **cannot** honor the strict `scan_from`/ordering
contract that `DocumentStore` requires. **KV is NOT used as a `DocumentStore` backend.**

KV's legitimate role: a **fast cache for `MemoryStore` (F07)** — keyed by `memory:{session_id}`,
storing the latest `SessionMemory` snapshot for cheap retrieval. This is a simple get/put by session
key, no scanning, no ordering. This usage belongs to F07's `KvMemoryStore`, not F23.

### Platform gating

D1 and R2 are `#[cfg(target_family = "wasm")]` (CF Workers) and behind the relevant `foundation_db`
wasm feature. They implement `AsyncDocumentStore` (one unified `Send` async trait — F00e/§A1; the CF
binding's `!Send` future is wrapped in `SendWrapper` on single-threaded wasm).

## Architecture

```mermaid
graph TD
    SY[DocumentStore sync] -->|valtron wrap| AS[AsyncDocumentStore trait - F06]
    AS --> D1[D1DocumentStore: SQL over D1 binding - PRIMARY/ordered]
    AS --> R2[R2 offload: large blobs by doc_id]
    D1 --> SCH[(documents schema 020+021 + r2_key)]
    D1 -.large blob.-> R2OBJ[(CF R2: doc/{c}/{scru128} objects)]
    KV -.eventually consistent.-> CFKV[(CF KV: lexicographic list via scru128)]
```

## HOW: Implementation Steps

1. Ensure **`AsyncDocumentStore`** (F06) is the canonical trait; provide the **sync `DocumentStore` as a
   valtron wrapper** over the async impl (house rule).
2. `D1DocumentStore` impl `AsyncDocumentStore` over `wasm/workers_rs/d1.rs` (**primary**); reuse
   `documents` schema; add the D1 `exec()` binding (or per-statement migration runner) so `020`/`021`
   apply (OD-23-6).
3. `scan_from_async` (D1): range query, `Vec<V>`.
4. **R2 large-blob path:** add/confirm the R2 binding; offload documents over a size threshold to
   `doc/{c}/{doc_id}` and store the `r2_key` + promoted columns in D1; transparent read-through (OD-23-12).
5. Tests: against D1/R2 via **miniflare/wrangler** (we test it properly, we already do so — OD-23-5)
   — D1 append→scan_from ordering parity with SQL/Memory/Fjall; R2 large-blob round-trip +
   read-through; D1+R2 transparent offload verified end-to-end.

## Open Decisions

- **OD-23-1 — D1 migration application:** how/where F06's `020`/`021` run on D1 (schema-init path,
  one-time). Confirm the D1 binding exposes batch DDL.
        - Check existing code, i believe in the cloudflare app we run the migration always, see examples in /home/darkvoid/Boxxed/@dev/ewe_platform/examples/cf-login-app and /home/darkvoid/Boxxed/@dev/ewe_platform/examples/cf-valtron-counter

- **OD-23-2 through OD-23-4, OD-23-7 through OD-23-9 — KV as DocumentStore: REMOVED (user,
  2026-06-15).** KV is NOT a DocumentStore backend — it cannot honor `scan_from`/ordering, is eventually
  consistent, has no range queries, and caps list at 1000 keys. KV's role is **MemoryStore cache only**
  (F07 `KvMemoryStore` — simple get/put by session key). All KV-as-DocumentStore ODs are moot.

- **OD-23-5 — testing harness: RESOLVED (user, 2026-06-15).** Miniflare and Wrangler — we test it
  properly, we already do so. No mock bindings for unit parity; real miniflare for integration.

- **OD-23-6 (D1 DDL): RESOLVED (user, 2026-06-15).** We own the code — expand the D1 binding to
  support batch DDL (`exec()` / multi-statement). Add a D1 `exec()` binding wrapping CF's
  `D1Database.exec`. Register `020`+`021` and ensure `init_schema_async` runs the documents schema
  on D1 (today it only creates the KV table). Also: migrate existing wasm-bindgen bindings to use
  **worker-rs types** instead of extracting raw `js_sys::Object` handles — makes the binding layer
  cleaner and type-safe.

- **OD-23-10 — pre-existing mismatch:** `AsyncQueryStore::query_async` is declared
  `-> AsyncQueryStream` but D1 impls `-> Vec<SqlRow>` (`d1_wasm.rs:606`); confirm the
  wasm-bindgen-storage path actually compiles for wasm32 before building `D1DocumentStore` on it.
       Ok we should fix that and ensure it aligns properly.

- **OD-23-11 — `?Send`: RESOLVED (user, 2026-06-15; Item #1 / §A1, owned by [F00e](../00e-unified-send-async-traits/feature.md)).**
  `AsyncDocumentStore` becomes a **single `Send` async trait** (no `?Send`); on single-threaded wasm the
  CF binding's `!Send` future is wrapped in `SendWrapper` so it presents as `Send`. Native + emscripten
  use genuine `Send`. So a `Send` caller on Workers drives the future through the adapter — no `?Send`
  surface to reconcile.


- **OD-23-12 — R2 topology: RESOLVED (user, 2026-06-15).** D1 holds the row + promoted columns +
  `r2_key`; R2 holds the large blob. Transparent behind one `AsyncDocumentStore`. **Threshold: the
  size of a SQLite page** (4KB default) — anything beyond that goes to R2. This is deliberately small
  to ensure D1 rows stay lean and R2 handles the heavy lifting. Both D1 and R2 are always used
  together (not one or the other) — the split is the default topology, not optional.
  
  
- **OD-23-13 — sync wrapper:** **Resolved (user, 2026-06-15)** → the sync `DocumentStore` is a **valtron
  wrapper** over the async impl (house rule: async-first, sync-via-valtron). Confirm the valtron
  block-on/drive primitive used elsewhere for async→sync and reuse it; don't hand-roll a second bridge.
      Yes, but if we hit a wall that makes this hard, its always ok in rare cases to duplicate if its the cleanest option.

- **OD-23-14 — KvDocumentStore: REMOVED (user, 2026-06-15).** `KvDocumentStore` does not exist. KV is
  used **only** as a MemoryStore cache (F07 `KvMemoryStore`) — simple key-value get/put for session
  memories. D1 + R2 is the complete DocumentStore deliverable for CF.

## Target Files

- `backends/foundation_db/src/wasm/` — `d1_document_store.rs` (primary), R2 offload path
  (`r2_document_store.rs`/blob module). NO `kv_document_store.rs` (KV is for MemoryStore only, F07).
- reuse `wasm/workers_rs/d1.rs`; add a D1 `exec()` binding + an R2 binding where missing;
  `documents` schema (+`r2_key`) from F06; migrate existing bindings to worker-rs types
- the sync `DocumentStore` valtron wrapper over the async impl (house rule)
- coordinates with F06 (`AsyncDocumentStore` + `scan_from_async` + doc_id ordering + promoted columns)
  and F08 (large Message records → R2)

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
the edge, prepared statements, no batch DDL — and how we add `exec()`); **R2** (S3-style object storage,
`list`/`startAfter`, when to offload large blobs vs inline in D1); **KV** (eventual consistency, list
pagination/cursors, 1000-key caps, no transactions); read-after-write hazards; async `?Send` bindings on
wasm; **async-first design with valtron sync wrappers** (why we build async and bridge to sync); choosing
the right store per job (D1 ordered, R2 for big blobs, KV best-effort). (Task — see list.)

## Done When

- `D1DocumentStore` implements `AsyncDocumentStore` incl. `scan_from_async`, with ordering parity
  (doc_id/scru128) with the SQL/Memory/Fjall backends — D1 is the strictly-ordered CF backend.
- Large documents offload to **R2** (D1 row + promoted columns + `r2_key`; blob in R2), transparent
  behind the trait; round-trip + read-through verified.
- The sync `DocumentStore` is a **valtron wrapper** over the async impl (async-first house rule).
- D1 reuses the `documents` schema (020+021, +`r2_key`); the D1 `exec()`/migration-apply gap is closed
  (capability added, not worked around).
- NO `KvDocumentStore` — KV is for MemoryStore cache only (F07), not DocumentStore.
- Builds for `wasm32` under the CF features; native unaffected. Tests via miniflare/wrangler.
- OD-23-1..14 resolved.
