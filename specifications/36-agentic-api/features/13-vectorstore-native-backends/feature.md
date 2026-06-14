---
feature: "VectorStore native backends: Turso/libSQL, SQLite (sqlite-vec), fjall (IVF)"
description: "Persistent native VectorStore backends — Turso/libSQL native vectors (DiskANN), SQLite via sqlite-vec with foundation_vectors fallback, and fjall storing serialized vectors + a persisted IVF index — all behind the F12 trait"
status: "pending"
priority: "medium"
depends_on: ["12-vectorstore-trait-inmemory", "09-foundation-vectors-ivf-hnsw"]
estimated_effort: "large"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 12
  total: 12
  completion_percentage: 0%
---

# Feature 13: VectorStore — native persistent backends

> **Review status (2026-06-14) — research folded + corrections:**
> 1. **The default `turso` crate (0.5.3) has NO `vector_top_k`/DiskANN** — only scalar
>    `vector_distance_cos/l2/dot` (full-scan). **DiskANN `vector_top_k` is in the optional `libsql`
>    crate only.** Split the "Turso/libSQL" backend: `libsql` feature → native DiskANN; default `turso`
>    → scalar full-scan or route to the `foundation_vectors` fallback (OD-13-7).
> 2. **`vector_top_k('idx', vec, k)` is a table-valued function over the whole index — it CANNOT
>    filter by namespace.** The WHAT "`WHERE namespace = ?` in the top-k query" is impossible; use
>    **over-fetch (k' ≫ k) + filter** (with under-return risk) or **index-per-namespace** (OD-13-4).
> 3. **No standalone SQLite client exists** (no `rusqlite`/`sqlite-vec` in the repo) — the SQLite
>    backend is **net-new deps**, not reuse; or implement it on the local-file `libsql`/`turso`
>    connection.
> 4. **Target files belong in `src/native/`** (next to `turso_backend.rs`/`libsql_store.rs`), reusing
>    the `SqlDocumentStore<Q: QueryStore>` pattern as `SqlVectorStore<Q>` — NOT `core/backends/`.
> 5. **Sync vs `AsyncVectorStore` (OD-13-6):** Turso/libsql are async-native (bridged to sync via
>    valtron `run_future_iter`). Decide: sync via blocking bridge vs implement `AsyncVectorStore`
>    (align with F12). **F12 must actually *define* `AsyncVectorStore` first** (see F14 review).
> 6. **fjall** would be added to `foundation_db` (F05 has it in `foundation_nativeapis`) — note the
>    second-crate placement (OD-13-8). **Dimension persistence + reopen-mismatch** per backend
>    undefined (OD-13-10).
> 7. **Likely split per-backend** (libsql-DiskANN / sqlite-vec / fjall-IVF) — "no partial delivery"
>    applies *per backend*, not one giant feature blocked on F08/F09/F12.

> Implements Decision 07's persistent native backends behind the F12 `VectorStore` trait. Each either
> uses the database's **native** vector search or stores vectors for `foundation_vectors` (F08/F09) to
> search. Research-heavy (Decision 07 mandates research before implementation).

## WHY: Problem Statement

The in-memory store (F12) loses data on restart. Sessions need persistence. Decision 07: store
pre-generated embeddings and query via native vector support where available, else
`foundation_vectors`. Three native backends: Turso/libSQL (DiskANN native), SQLite (`sqlite-vec`
extension, fallback to flat), fjall (serialized vectors + persisted IVF index).

## WHAT: Solution

All implement the F12 `VectorStore` trait (same `insert/query(namespace)/delete/flush/stats`,
dimension enforcement). Differences are purely storage + query path:

### Turso / libSQL — native DiskANN

- Store vectors via libSQL's `vector(...)` BLOB type; query with `vector_top_k()` (DiskANN index).
- **Research (Decision 07):** does the Rust libSQL/Turso client expose `vector_top_k()`? index config?
  D1 compatibility (D1 is libSQL-derived)? Fallback to `foundation_vectors` if native unavailable.
- Namespace = a `namespace` column + `WHERE namespace = ?` in the top-k query.

### SQLite — `sqlite-vec` (optional) + fallback

- If `sqlite-vec` extension loads: `vec0` virtual table, native KNN.
- Else: store vectors as BLOBs, fetch (namespace-filtered) + `foundation_vectors::flat_top_k`.
- **Research:** `sqlite-vec` extension loading in Rust (rusqlite/libsql), availability, vec0 schema.

### fjall — serialized vectors + persisted IVF

- Store `id → (vector bytes, metadata)` in a fjall partition per namespace.
- Build/load a `foundation_vectors` IVF index (F09); persist via the F09 self-describing
  `to_bytes`/`load_index`. Query = load index → `search` within namespace.
- **Research:** fjall keyspace design for vector+metadata co-location; index persistence cadence.

### Shared concerns

- **Dimension** stored in store config; enforced on insert (F12).
- **Namespace** isolation in every query (the gaps fix).
- **`&self`** + interior locking (Decision 08).
- All **native-only** (target-gated off wasm; CF backends are F14).
- **No partial delivery within a backend** (Decision 07): each backend ships insert + query (native or
  fallback) + delete + flush + dimension validation + tests.

## Architecture

```mermaid
graph TD
    T[VectorStore trait F12] --> TUR[Turso/libSQL: vector() + vector_top_k DiskANN]
    T --> SQL[SQLite: sqlite-vec vec0 OR BLOB + flat fallback]
    T --> FJ[fjall: vector bytes + persisted IVF index F09]
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
4. fjall backend (serialized vectors + persisted IVF via F09) + namespace.
5. Dimension enforcement + delete/flush/stats each.
6. Tests per backend: insert/query parity with in-memory ground truth; namespace isolation;
   persistence across reopen; dimension mismatch; native-path + fallback-path both.

## Open Decisions

- **OD-13-1 — Turso `vector_top_k` availability:** confirm Rust client support; fallback if not.
- **OD-13-2 — sqlite-vec optionality:** feature-gate; flat fallback when absent.
- **OD-13-3 — fjall index persistence cadence:** rebuild-on-open vs incremental persist. Rec:
  persist index bytes (F09) on flush; rebuild tail on open.
- **OD-13-4 — namespace + native KNN:** column filter pre/post native top-k (DiskANN may not filter).
  Research; may over-fetch + filter.
- **OD-13-5 — reuse existing libSQL/SQLite infra:** foundation_db already has SQL backends — reuse
  the connection/query layer rather than new clients. Confirm.

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

- Turso/libSQL, SQLite, fjall backends implement `VectorStore` (native or fallback), persist across
  restart, enforce dimension + namespace, pass parity tests vs in-memory.
- Research findings documented in this feature before implementation.
- Native-only/target-gated; fundamentals authored. OD-13-1..5 resolved.
