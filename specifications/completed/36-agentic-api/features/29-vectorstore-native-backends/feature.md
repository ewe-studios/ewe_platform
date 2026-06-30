---
feature: "VectorStore native backends: Turso/libSQL, SQLite (sqlite-vec), fjall (IVF)"
description: "Persistent native VectorStore backends — Turso/libSQL native vectors (DiskANN), SQLite via sqlite-vec with foundation_vectors fallback, and fjall storing serialized vectors + a persisted IVF index — all behind the F28 trait"
status: "complete"
priority: "medium"
depends_on: ["28-vectorstore-trait-inmemory", "25-foundation-vectors-ivf-hnsw"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-26
author: "Main Agent"
tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
---

# Feature 29: VectorStore — native persistent backends

> **Review status (2026-06-14) — research folded + corrections:**
> 1. **The default `turso` crate (0.5.3) has NO `vector_top_k`/DiskANN** — only scalar
>    `vector_distance_cos/l2/dot` (full-scan). **DiskANN `vector_top_k` is in the optional `libsql`
>    crate only.** Split the "Turso/libSQL" backend: `libsql` feature → native DiskANN; default `turso`
>    → scalar full-scan or route to the `foundation_vectors` fallback (OD-29-7).
> 2. **`vector_top_k('idx', vec, k)` is a table-valued function over the whole index — it CANNOT
>    filter by namespace.** The WHAT "`WHERE namespace = ?` in the top-k query" is impossible; use
>    **over-fetch (k' ≫ k) + filter** (with under-return risk) or **index-per-namespace** (OD-29-4).
> 3. **No standalone SQLite client exists** (no `rusqlite`/`sqlite-vec` in the repo) — the SQLite
>    backend is **net-new deps**, not reuse; or implement it on the local-file `libsql`/`turso`
>    connection.
> 4. **Target files belong in `src/native/`** (next to `turso_backend.rs`/`libsql_store.rs`), reusing
>    the `SqlDocumentStore<Q: QueryStore>` pattern as `SqlVectorStore<Q>` — NOT `core/backends/`.
> 5. **Sync vs `AsyncVectorStore` (OD-29-6):** Turso/libsql are async-native (bridged to sync via
>    valtron `run_future_iter`). Decide: sync via blocking bridge vs implement `AsyncVectorStore`
>    (align with F28). **F28 must actually *define* `AsyncVectorStore` first** (see F30 review).
> 6. **fjall** would be added to `foundation_db` (F22 has it in `foundation_nativeapis`) — note the
>    second-crate placement (OD-29-8). **Dimension persistence + reopen-mismatch** per backend
>    undefined (OD-29-10).
> 7. **Likely split per-backend** (libsql-DiskANN / sqlite-vec / fjall-IVF) — "no partial delivery"
>    applies *per backend*, not one giant feature blocked on F24/F25/F28.

> Implements Decision 07's persistent native backends behind the F28 `VectorStore` trait. Each either
> uses the database's **native** vector search or stores vectors for `foundation_vectors` (F24/F25) to
> search. Research-heavy (Decision 07 mandates research before implementation).

## WHY: Problem Statement

The in-memory store (F28) loses data on restart. Sessions need persistence. Decision 07: store
pre-generated embeddings and query via native vector support where available, else
`foundation_vectors`. Three native backends: Turso/libSQL (DiskANN native), SQLite (`sqlite-vec`
extension, fallback to flat), fjall (serialized vectors + persisted IVF index).

## WHAT: Solution

All implement the F28 `VectorStore` trait (same `insert/query(namespace)/delete/flush/stats`,
dimension enforcement). Differences are purely storage + query path:

### Turso / libSQL — native DiskANN

- Store vectors via libSQL's `vector(...)` BLOB type; query with `vector_top_k()` (DiskANN index).
- **Research (Decision 07):** does the Rust libSQL/Turso client expose `vector_top_k()`? index config?
  D1 compatibility (D1 is libSQL-derived)? Fallback to `foundation_vectors` if native unavailable.
- **Namespace filtering (OD-29-4 — load-bearing):** `vector_top_k('idx', vec, k)` is a table-valued
  function over the **whole index** — it **cannot** accept a `WHERE namespace = ?` filter. Two
  strategies:
  1. **Over-fetch + post-filter (default):** call `vector_top_k('idx', vec, k')` with `k' = k * 4`
     (configurable multiplier), then filter results by `namespace` in application code, take top `k`.
     Risk: if the namespace is a small fraction of the index, under-return is possible. Mitigate by
     increasing the multiplier or falling back to strategy 2.
  2. **Index-per-namespace:** create a separate DiskANN index per namespace (`CREATE INDEX
     idx_{ns} ...`). Eliminates the filter problem but increases storage and DDL ops. Better for
     long-lived sessions with many vectors.
  The strategy is config-driven (`NamespaceStrategy::OverFetch { multiplier }` or `PerNamespace`).
  Default: `OverFetch { multiplier: 4 }`.
- **Crate split:** the default `turso` crate (0.5.3) has **NO `vector_top_k`/DiskANN** — only scalar
  `vector_distance_cos/l2/dot` (full-scan). DiskANN `vector_top_k` is in the optional **`libsql`**
  crate only. So: `libsql` feature → native DiskANN; default `turso` → scalar full-scan OR route to
  `foundation_vectors::flat_top_k` as fallback (OD-29-7).

### SQLite — `sqlite-vec` (optional) + fallback

- **With `sqlite-vec`:** load the extension at connection init, create `vec0` virtual tables per
  namespace (`vec0_{ns}`), use native `vec0` KNN queries. `sqlite-vec` provides
  `vec_distance_cosine`/`vec_distance_l2` + a `vec0` virtual table that wraps brute-force KNN with
  columnar vector storage. Schema per namespace:
  ```sql
  CREATE VIRTUAL TABLE vec0_{ns} USING vec0(id TEXT PRIMARY KEY, embedding float[{dim}]);
  ```
  Query: `SELECT id, distance FROM vec0_{ns} WHERE embedding MATCH ? ORDER BY distance LIMIT ?`.
  Namespace isolation is structural (one virtual table per namespace).
- **Without `sqlite-vec` (fallback):** store vectors as BLOBs in a regular table
  (`vectors(id TEXT, namespace TEXT, vector BLOB, metadata TEXT)`); fetch all rows matching
  `WHERE namespace = ?`, deserialize, pass to `foundation_vectors::flat_top_k`. This is O(n) per
  namespace but correct.
- **Extension loading:** via the `libsql`/`rusqlite` `load_extension` API (path to `.so`/`.dylib`).
  Feature-gated: `sqlite-vec` feature enables the extension path; absent = BLOB fallback.
- **No standalone SQLite client exists in the repo** — this backend runs on the local-file
  `libsql`/`turso` connection (reuse F29's Turso/libSQL connection layer, not a new `rusqlite` dep).

### fjall — serialized vectors + persisted IVF

- **Storage layout:** one fjall keyspace per store, partitioned by namespace:
  - **Vector partition** (`vec:{ns}:{id}`): key = namespaced id, value = serialized `VectorEntry`
    (vector bytes as `[f32]` → little-endian `&[u8]`, plus `VectorMetadata`).
  - **Index partition** (`idx:{ns}`): single key per namespace, value = the F25 `VectorIndex`
    serialized bytes (`to_bytes()` — self-describing: kind tag + metric + dimension + params +
    vectors).
- **Insert:** write to the vector partition; mark the namespace index as stale.
- **Query:** load the persisted index (`load_index(bytes)` → `Box<dyn VectorIndex>`), call
  `.search(query, k)`. If the index is stale (inserts since last build), either rebuild (if few
  inserts) or fall back to `flat_top_k` over the vector partition scan.
- **Index persistence cadence:** persist index bytes on `flush()`; rebuild the stale tail on open
  (same crash-recovery model as F22). A background valtron task can rebuild periodically if the
  stale count exceeds a threshold (e.g. 100 inserts since last build).
- **Namespace isolation:** structural — each namespace has its own partitions. `query` reads only the
  target namespace's partition.

### Shared concerns

- **Dimension** stored in store config; enforced on insert (F28).
- **Namespace** isolation in every query (the gaps fix).
- **`&self`** + interior locking (Decision 08).
- All **native-only** (target-gated off wasm; CF backends are F30).
- **No partial delivery within a backend** (Decision 07): each backend ships insert + query (native or
  fallback) + delete + flush + dimension validation + tests.

## Architecture

```mermaid
graph TD
    T[VectorStore trait F28] --> TUR[Turso/libSQL: vector() + vector_top_k DiskANN]
    T --> SQL[SQLite: sqlite-vec vec0 OR BLOB + flat fallback]
    T --> FJ[fjall: vector bytes + persisted IVF index F25]
    TUR & SQL & FJ -->|fallback| FV[foundation_vectors flat/IVF]
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: native DB vector search (libSQL **DiskANN**, `vector_top_k`);
**`sqlite-vec`** (vec0 virtual tables, extension loading in Rust); storing vectors as BLOBs &
serialization; fjall keyspace design for vector+metadata + persisting an IVF index; native-vs-fallback
query paths; how namespace filtering composes with native KNN; the research-before-implementation
discipline. (Task — see list.)

## HOW: Implementation Steps

1. **Research pass** (Decision 07): document Turso `vector_top_k` Rust API, sqlite-vec loading,
   fjall layout — in this feature.md before coding.
2. Turso/libSQL backend (native + fallback) + namespace.
3. SQLite backend (sqlite-vec + flat fallback) + namespace.
4. fjall backend (serialized vectors + persisted IVF via F25) + namespace.
5. Dimension enforcement + delete/flush/stats each.
6. Tests per backend: insert/query parity with in-memory ground truth; namespace isolation;
   persistence across reopen; dimension mismatch; native-path + fallback-path both.

## Open Decisions

- **OD-29-1 — Turso `vector_top_k` availability:** confirm Rust client support; fallback if not.
- **OD-29-2 — sqlite-vec optionality:** feature-gate; flat fallback when absent.
- **OD-29-3 — fjall index persistence cadence:** rebuild-on-open vs incremental persist. Rec:
  persist index bytes (F25) on flush; rebuild tail on open.
- **OD-29-4 — namespace + native KNN: PARTIALLY RESOLVED.** `vector_top_k` CANNOT filter by namespace
  (it's a whole-index TVF). Two strategies documented in WHAT: over-fetch+post-filter (default,
  `k' = k * multiplier`) or index-per-namespace. Config-driven `NamespaceStrategy` enum. The
  multiplier default (4) and the per-namespace DDL cost need benchmarking during implementation.
- **OD-29-5 — reuse existing libSQL/SQLite infra:** foundation_db already has SQL backends — reuse
  the connection/query layer rather than new clients. Confirm.
      Sure make sense

## Target Files

- `backends/foundation_db/src/core/backends/{turso_vector_store, sqlite_vector_store, fjall_vector_store}.rs` (new, native)
- `backends/foundation_db/Cargo.toml` — sqlite-vec (optional), fjall, libSQL vector features

## Tests

```bash
cargo test -p foundation_db -- vector_store::{turso,sqlite,fjall}
```

## Verification

```bash
cargo build -p foundation_db --features <vector-native>
cargo clippy -p foundation_db --features <vector-native> -- -D warnings
cargo test  -p foundation_db -- vector_store
```

## Done When

**DONE (2026-06-26):**

- [x] **LibsqlVectorStore** — libSQL DiskANN via `vector_top_k`, over-fetch namespace strategy,
  exact re-rank with `flat_top_k`. 5 tests.
- [x] **SqliteVecVectorStore** — sqlite-vec vec0 virtual tables, bundled `.so` loaded at init
  (`load_extension_enable`/`load_extension` + `SendWrapper`/`pollster` bridge). 4 tests.
- [x] **FjallVectorStore** — fjall LSM + persisted F25 IVF index, rebuild-from-vectors on open.
  7 tests.
- [x] **SqlVectorStore** (shipped earlier) — SQL BLOB + `flat_top_k` fallback for backends without
  native vector support. 11 tests.
- [x] All backends: dimension + namespace enforcement, persist across restart, pass parity tests
  vs in-memory ground truth.
- [x] Native-only/target-gated off wasm; fundamentals authored (`fundamentals/00-overview.md`).
- [x] OD-29-1..5 resolved (libsql vector_top_k confirmed, sqlite-vec optionality, fjall index
  persistence cadence, namespace+native KNN over-fetch, reuse libsql connection layer).
