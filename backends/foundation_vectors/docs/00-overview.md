# Fundamentals 00 — Cloudflare & external vector backends

Zero-to-expert on putting vectors *somewhere other than local memory* — serverless
edges (Cloudflare) and managed vector databases (TurboPuffer, and trait-ready
Pinecone/Chroma). This folder explains the data models, the fetch-then-search vs
native-ANN trade-off, and the platform-agnostic HTTP transport that makes one
adapter run native **and** wasm.

---

## 1. Where this sits

The VectorStore backend set is built in three features:

- **F28 — in-memory** (`InMemoryVectorStore`): the trait + a RAM backend. Covers
  tests and small workloads.
- **F29 — native persistent** (`SqlVectorStore<Q: QueryStore>`, Turso/libSQL/
  SQLite, fjall): durable local stores.
- **F30 — this feature: Cloudflare + external managed.** Serverless/edge and
  large managed deployments.

All three implement the same trait, so the agentic layer (F16/F32 search) is
backend-agnostic — swap storage without touching call sites.

## 2. The trait seam

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn insert_async(&self, namespace: &str, entry: VectorEntry) -> Result<(), E>;
    async fn delete_async(&self, namespace: &str, id: &str) -> Result<(), E>;
    async fn search_async(&self, namespace: &str, query: &[f32], k: usize)
        -> Result<Vec<VectorMatch>, E>;
    async fn get_async(&self, namespace: &str, id: &str) -> Result<Option<VectorEntry>, E>;
    async fn len_async(&self, namespace: &str) -> Result<usize, E>;
    async fn is_empty_async(&self, namespace: &str) -> Result<bool, E>; // has a default
    fn config(&self) -> &VectorStoreConfig;
}
```

Every backend — local, CF, or managed — is just an implementation. A single
**`Send`** async trait (no `?Send`); on single-threaded wasm the `!Send`
fetch/Promise futures are wrapped in `SendWrapper` so they present as `Send`
(F00e / Doc 03).

The trait is async only (spec 50 feature 05 task T1). There is no sync vector
store trait. The method names keep the `_async` suffix until task T3 removes
the suffix from all storage traits.

### 2.1 The contract

All stores obey the same rules. The crate's `tests/contract` module checks
them for each store:

| Rule | Detail |
|---|---|
| Validation | A wrong dimension is `DimensionMismatch`. A zero vector is `ZeroVector`. |
| Replace | An insert with an existing id replaces the entry. |
| Delete | Deleting an id that is not in the namespace is `NotFound`. |
| Search | At most `k` matches, best first. A higher score is closer. |
| Empty | An unknown namespace gives no matches, `get` gives `None`, `len` gives 0. |
| Errors | A backend failure is an error. `len_async` never reads a failure as 0. |
| Isolation | One namespace never sees the entries of another. |

### 2.2 Where each store waits

| Store | Input/output (I/O) | How it waits |
|---|---|---|
| `InMemoryVectorStore` | none | Ready on the first poll. |
| `SqlVectorStore`, `LibsqlVectorStore` | SQL | Awaits the async `QueryStore`. |
| `SqliteVecVectorStore` | local SQLite file | Awaits libSQL's async API. `open` writes and loads the `.so` with sync calls, once. |
| `FjallVectorStore` | local disk | fjall has only a sync API. Calls run inline (page cache, no `fsync`), like `FjallDocumentStore` in `foundation_nativeapis`. `flush` (`fsync`) is a sync method. |
| `TurboPufferVectorStore` | network | Awaits `HttpClient::send_async`. Bodies are read with limits (`TurboPufferLimits`). |

## 3. What F30 actually ships (scope, post-decisions)

The original feature listed many providers; the resolved scope (user, 2026-06-15)
is deliberately tight:

- **TurboPuffer — REQUIRED, shipped.** A thin `VectorStore` adapter over our
  own `HttpClient` (F00f) against TurboPuffer's v2 REST API. Platform-agnostic: builds
  and runs on **native and wasm**.
- **Cloudflare D1 fetch-then-search — covered by F29's `SqlVectorStore`.** D1 is
  SQLite; `SqlVectorStore<D1 query store>` stores vectors in SQL and re-ranks
  client-side with `foundation_vectors` flat search (Doc 02). No new backend type
  needed — the generic seam already serves it.
- **CF Vectorize — absent.** Research (OD-30-1) found **no runtime Vectorize
  binding** in the Workers stack (only deployment DTOs). We don't force it; D1
  fetch-then-search is the CF vector path until/unless a binding appears.
- **CF KV vectors — best-effort, documented-only.** KV's eventual consistency +
  1000-key `list` cap (inherited from F23) make it unsuitable as a primary vector
  store; it's a best-effort cache at most (Doc 02 §4).
- **Pinecone + Chroma — deferred.** The trait makes them droppable adapters later;
  none is written now.

## 4. Why "behind one trait" is the whole point

Managed vector DBs differ wildly — Chroma has *collections* with their own
dimension/metric, Pinecone/TurboPuffer have *namespaces* (partitions of one index),
dimension is checked server-side and fails async. If those differences leaked into
the agent, swapping providers would be a rewrite. The trait absorbs them: the agent
says `search_async(namespace, query, k)`; each adapter maps `namespace` to the
provider's native concept and handles the provider's quirks (Doc 01).

---

**Next:** Doc 01 — managed vector DB data models (namespaces, collections, ANN).
