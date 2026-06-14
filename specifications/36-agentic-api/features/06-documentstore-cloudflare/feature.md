---
feature: "DocumentStore: Cloudflare D1 + KV (AsyncDocumentStore)"
description: "Async-first AsyncDocumentStore for Cloudflare: D1 (SQLite, the primary ordered backend) + R2 for large document blobs (e.g. big Message-API records that don't fit SQLite), with KV as an optional best-effort key-value path. scan_from_async; the sync DocumentStore is a valtron wrapper over async. Completes the DocumentStore backend set for serverless/wasm"
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

> **RESOLVED (user, 2026-06-15) — design is the target; build/fix/rebuild reality to match. No
> half-assed work.** The spec states the end desire; where the current code doesn't meet it we build it
> properly, rebuilding from scratch if that's what getting it right takes. If a CF capability is missing
> (D1 batch DDL, KV cursor pagination), **we add it** — we own these bindings; we don't declare "this
> won't work." But we **select tooling sensibly** to what each store is actually good at.
>
> **RESOLVED — async-first, sync wraps async via valtron (house rule).** Build for the **async world**:
> the canonical implementation is **`AsyncDocumentStore`** (`*_async` methods). The **sync `DocumentStore`
> is a thin valtron wrapper** around the async impl (the same pattern used for every other async trait in
> the platform). If `AsyncDocumentStore` is missing, F04 adds the trait; F06 supplies CF impls. So we
> have two traits — `DocumentStore` (sync) and `AsyncDocumentStore` (async) — and async-first means
> everything just works where needed, with valtron bridging to sync where sensible.
>
> **RESOLVED — use the right CF API per job; D1+KV are NOT both mandatory.** foundation_db's CF layer
> supporting both KV and D1 does not mean F06 must use both. Plan:
> - **D1 (SQLite, full relational) is the primary ordered backend** — it satisfies the strict
>   `scan_from`/ordering contract. Even if F06 ships **only D1**, that's a complete win.
> - **R2 for large documents** — Messages persisted by the Message API (F16) can be large and don't fit
>   SQLite well. Store the big blob in **R2** (object storage), keyed by doc_id, with D1 holding the
>   row + promoted columns + the R2 key. (New `R2DocumentStore` / R2-backed blob path.)
> - **KV only where quick key-value lookup genuinely helps** — best-effort (eventually consistent), not
>   the ordered backbone. Optional.

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

> Implements Decision 13's Cloudflare backends **async-first** via the **`AsyncDocumentStore`** trait
> (D1/KV/R2 are Promise-based, async-only on wasm); the sync `DocumentStore` is a valtron wrapper over
> the async impl. No `AsyncDocumentStore` impl exists yet (verified). **D1 is the primary ordered
> backend; R2 stores large document blobs; KV is optional best-effort.** Reuses the existing CF D1
> bindings (`wasm/workers_rs/d1.rs`, `wasm/wasm_storage/d1_wasm.rs`) and adds an R2 binding/path where
> missing. Completes the DocumentStore backend set begun in F04 (SQL+Memory) and F05 (VFS/Fjall).

## WHY: Problem Statement

For serverless/CF Workers deployments, sessions persist to Cloudflare. The Workers runtime is
**async-only**, so the canonical impl is **`AsyncDocumentStore`** (`storage_provider.rs:514`); the sync
`DocumentStore` is a valtron wrapper over it. The trait + the `scan_from_async` signature come from F04;
F06 supplies the CF implementations.

**Tool selection (right API per job):** **D1** (SQLite, relational, ordered) is the **primary** backend
and satisfies the strict `scan_from`/ordering contract. **R2** (object storage) holds **large document
blobs** — Message-API records (F16) can exceed what fits comfortably in a SQLite row, so the big payload
lives in R2 keyed by doc_id while D1 keeps the row + promoted columns + the R2 key. **KV** is an
**optional best-effort** key-value path (eventually consistent), used only where quick KV lookups help —
never the ordered backbone. Shipping D1 (+ R2 for large blobs) alone is a complete deliverable; KV is
additive.

## WHAT: Solution

### D1 backend (SQL, async) — `D1DocumentStore` — **PRIMARY**

D1 is SQLite-compatible, so this mirrors `SqlDocumentStore` over the async D1 binding and is the
strictly-ordered CF backend:

- Reuse the **same `documents` schema** (F04's migration `020`+`021`, incl. promoted columns) — D1
  runs the same SQL.
- `append_async`/`scan_async`/`scan_all_async`/`scan_from_async`/`delete_async`/`count_async` via the
  existing D1 query binding (`wasm/workers_rs/d1.rs`).
- `scan_from_async`: `WHERE collection_key=? AND doc_id>=? ORDER BY doc_id ASC LIMIT ?` — same as SQL
  (F04 doc_id ordering), returning `Vec<V>` (async trait returns Vec, not a stream).
- D1 migrations: ensure F04's `020`/`021` are applied in the D1 schema-init path. D1 lacks batch DDL —
  **add a D1 `exec()` binding or split migrations into per-statement `run()`s** (OD-06-1/OD-06-6); we own
  the binding, so we add the capability rather than work around it.
- **Large-blob offload to R2:** when a document exceeds a size threshold, D1 stores the row + promoted
  columns + an `r2_key`, and the blob goes to R2 (below). Small documents stay inline in D1.

### R2 backend (large blobs, async) — `R2DocumentStore` / R2 offload

CF **R2** is S3-style object storage for payloads too large for a SQLite row (e.g. big Message-API
records, F16):

- **`append_async`:** `r2.put("doc/{collection}/{doc_id}", json_or_bytes)`; doc_id = scru128 (F04).
- **`get`/`scan_from_async`:** R2 has `list({ prefix, cursor, startAfter })` returning
  lexicographically-ordered keys — scru128 keys are chronological, so `list("doc/{c}/")` + `startAfter`
  gives ordered range reads; fetch each object by key. Ordering metadata (doc_id list, promoted columns)
  is best kept in **D1** so range/typed queries stay on the relational backend and R2 holds only the
  bytes (the recommended split — OD-06-12).
- **Standalone vs offload:** R2 can back a store on its own, but the **recommended** topology is **D1 as
  the index/metadata + R2 as the blob store** for large records, transparent behind one
  `AsyncDocumentStore`.

### KV backend (key-list, async) — `KvDocumentStore` — **OPTIONAL, best-effort**

CF KV is pure key-value and **eventually consistent** (no read-after-write), so it is **optional** and
**not** the ordered backbone — use only where quick KV lookups help. Documents use the Decision 13
layout:

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
    SY[DocumentStore sync] -->|valtron wrap| AS[AsyncDocumentStore trait - F04]
    AS --> D1[D1DocumentStore: SQL over D1 binding - PRIMARY/ordered]
    AS --> R2[R2 offload: large blobs by doc_id]
    AS -.optional.-> KV[KvDocumentStore: doc:{c}:{scru128} - best-effort]
    D1 --> SCH[(documents schema 020+021 + r2_key)]
    D1 -.large blob.-> R2OBJ[(CF R2: doc/{c}/{scru128} objects)]
    KV -.eventually consistent.-> CFKV[(CF KV: lexicographic list via scru128)]
```

## HOW: Implementation Steps

1. Ensure **`AsyncDocumentStore`** (F04) is the canonical trait; provide the **sync `DocumentStore` as a
   valtron wrapper** over the async impl (house rule).
2. `D1DocumentStore` impl `AsyncDocumentStore` over `wasm/workers_rs/d1.rs` (**primary**); reuse
   `documents` schema; add the D1 `exec()` binding (or per-statement migration runner) so `020`/`021`
   apply (OD-06-6).
3. `scan_from_async` (D1): range query, `Vec<V>`.
4. **R2 large-blob path:** add/confirm the R2 binding; offload documents over a size threshold to
   `doc/{c}/{doc_id}` and store the `r2_key` + promoted columns in D1; transparent read-through (OD-06-12).
5. *(Optional)* `KvDocumentStore` over the CF KV binding (`doc:{c}:{scru128}` keys), extending the KV
   `list` wrapper to honor `cursor`/`limit` (OD-06-7); explicitly best-effort (OD-06-8).
6. Resolve "last N" on KV (OD-06-3) and promoted-field handling (OD-06-4) if KV is built.
7. Tests: against D1/R2/KV bindings (gated; mock bindings or miniflare) — D1 append→scan_from ordering
   parity with SQL/Memory/Fjall; R2 large-blob round-trip + read-through; KV list-cursor correctness
   (best-effort).

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
- **OD-06-12 — R2 topology:** **Resolved (user, 2026-06-15)** → recommended split is **D1 holds the row
  + promoted columns + `r2_key`; R2 holds the large blob**, transparent behind one `AsyncDocumentStore`.
  A standalone R2-only store is possible but loses cheap range/typed queries. Decide the **size
  threshold** for offload (e.g. inline in D1 below N KB, R2 above). The Message API's large records (F16)
  are the motivating case.
- **OD-06-13 — sync wrapper:** **Resolved (user, 2026-06-15)** → the sync `DocumentStore` is a **valtron
  wrapper** over the async impl (house rule: async-first, sync-via-valtron). Confirm the valtron
  block-on/drive primitive used elsewhere for async→sync and reuse it; don't hand-roll a second bridge.
- **OD-06-14 — KV is optional:** **Resolved (user, 2026-06-15)** → D1 (+ R2 for blobs) is a complete
  deliverable; `KvDocumentStore` is additive/best-effort and may be deferred. Don't gate the feature's
  Done-When on KV parity.

## Target Files

- `backends/foundation_db/src/wasm/` — `d1_document_store.rs` (primary), R2 offload path
  (`r2_document_store.rs`/blob module), `kv_document_store.rs` (optional)
- reuse `wasm/workers_rs/d1.rs`; add a D1 `exec()` binding + an R2 binding where missing; the CF KV
  binding (extend `list` cursor) if KV is built; `documents` schema (+`r2_key`) from F04
- the sync `DocumentStore` valtron wrapper over the async impl (house rule)
- coordinates with F04 (`AsyncDocumentStore` + `scan_from_async` + doc_id ordering + promoted columns)
  and F16 (large Message records → R2)

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
- `KvDocumentStore` is **optional/best-effort** (eventually consistent) — its absence does not block the
  feature; when built it uses the `doc:{c}:{scru128}` layout with proper cursor pagination.
- Builds for `wasm32` under the CF features; native unaffected.
- OD-06-1..14 resolved.
