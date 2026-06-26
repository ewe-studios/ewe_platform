# Fundamentals 00 — Native persistent vector backends

Zero-to-expert on persisting vectors on a real machine (vs the in-memory F28
store). Three backends, one trait — and the recurring decision that shapes all of
them: **let the database do the nearest-neighbour search, or fetch the vectors and
search them yourself.**

---

## 1. Why persist at all

The in-memory store (F28) is fast and exact but loses everything on restart, and
holds the whole corpus in RAM. Agentic sessions need vectors that **survive a
restart** and can exceed RAM. F29 adds three durable native backends, all behind
the same F28 `VectorStore` trait so the agent never knows which one it's using:

| backend | storage | search path |
|---|---|---|
| `LibsqlVectorStore` | libSQL `FLOAT32` column + DiskANN index | **native ANN** (`vector_top_k`) |
| `SqlVectorStore<Q>` | SQL BLOB column (any `QueryStore`) | **fallback** (`flat_top_k` over a namespace scan) |
| `FjallVectorStore` | fjall LSM + persisted F25 IVF index | **native ANN** (IVF), rebuildable from vectors |

`SqlVectorStore` doubles as the **SQLite backend's fallback** (Doc 03 §1): a
SQLite/libSQL file with vectors stored as BLOBs, searched by fetching the
namespace and running `flat_top_k`.

## 2. The one trait

Every backend implements F28's `VectorStore`:

```rust
trait VectorStore: Send + Sync {
    fn insert(&self, namespace: &str, entry: VectorEntry) -> Result<(), E>;
    fn delete(&self, namespace: &str, id: &str) -> Result<(), E>;
    fn search(&self, namespace: &str, query: &[f32], k: usize) -> Result<Vec<VectorMatch>, E>;
    fn get(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, E>;
    fn len(&self, namespace: &str) -> usize;
    fn config(&self) -> &VectorStoreConfig;
}
```

Differences are **purely in storage + the query path**. Dimension is enforced on
insert; namespaces are isolated in every query (Doc 01 §4, Doc 02 §3).

## 3. Native ANN vs fetch-then-search

This is the axis everything turns on:

- **Native ANN** — the store has an index (DiskANN, IVF) and answers top-k in
  sub-linear time on its own side. `LibsqlVectorStore` (DiskANN) and
  `FjallVectorStore` (IVF) do this.
- **Fetch-then-search (fallback)** — the store has no vector index, so search
  *fetches* the namespace's vectors and runs an exact `flat_top_k` (O(n) per
  namespace). `SqlVectorStore` does this; it's the universal, correct, portable
  path for any backend (and the SQLite-without-`sqlite-vec` story).

Native ANN is faster at scale but **approximate** and metric-bound to the index;
fetch-then-search is exact and metric-flexible but O(n). Two of our backends
bridge the gap by using native ANN for *candidate retrieval* and then re-ranking
exactly (Doc 01 §3).

## 4. Research before implementation (Decision 07)

Native vector features are vendor-specific and easy to assume wrong. Decision 07
mandates **verifying the actual API before coding** — and it mattered here:

- The default `turso` crate (0.5.x) has **no** `vector_top_k`/DiskANN — only
  scalar `vector_distance_*` (a full scan). DiskANN lives in the **`libsql`**
  crate. So the "Turso/libSQL native" backend is really **libSQL-only**; over
  `turso` you fall back to `SqlVectorStore`.
- `vector_top_k` is a table-valued function over the **whole index** — it
  **cannot** take a `WHERE namespace = ?`. That single fact reshapes the
  namespace design (Doc 01 §4).
- `sqlite-vec` is a **net-new external C extension** — not in the repo, loaded at
  runtime by path; the fallback path covers SQLite without it (Doc 03).

Each finding is recorded in this feature's `feature.md`; the lesson is to probe
the real database (a throwaway test) before building on an assumed API.

---

**Next:** Doc 01 — native DB vector search (libSQL DiskANN + sqlite-vec).
