# Fundamentals: the async SQL DocumentStore (F22b)

Zero-to-expert on `SqlDocumentStore` and the ideas behind it. Read this if
you've never touched `foundation_db`'s storage traits.

---

## 1. Two traits, on purpose: `QueryStore` vs `AsyncQueryStore`

`foundation_db` deliberately keeps **separate sync and async traits** for SQL
access instead of one trait with a runtime flag:

```rust
trait QueryStore {                     // synchronous
    fn query(&self, sql, params) -> StorageResult<StorageItemStream<'_, SqlRow>>;
    fn execute(&self, sql, params) -> StorageResult<u64>;
}
#[async_trait(?Send)]
trait AsyncQueryStore {                 // asynchronous
    async fn query_async(&self, sql, params) -> StorageResult<AsyncQueryStream>;
    async fn execute_async(&self, sql, params) -> StorageResult<u64>;
}
```

Why two? Because the *environment* dictates which is usable:

- **Native** (Turso/Libsql on a real OS) can block a thread, so a sync API is
  natural and ergonomic.
- **Serverless / wasm** (Cloudflare D1) is **Promise-based and single-threaded** —
  you physically cannot block; you must `.await`. A sync call there would deadlock
  the event loop.

A single "maybe-async" trait would force every caller to pick the wrong half on
some target. Separate traits with an `_async` suffix let each environment use the
shape that actually works (this is the project's standing rule — see F06 OD-06-7).

The same split exists one layer up for documents: **`DocumentStore`** (sync) and
**`AsyncDocumentStore`** (async). `SqlDocumentStore<Q: AsyncQueryStore>` is the
document store for any SQL backend.

Backends implement only the async traits. The sync traits are a **view** for sync
callers: `foundation_db::Blocking<S>` implements each one over the matching async
trait (native and non-JS wasm only; see `00-storage-traits.md`).

---

## 2. Streams, not `Vec`: pulling at your own pace

A naïve "read the collection" API returns `Vec<T>` — it loads **everything** into
memory before you see the first item. For a session log with 100k records that's a
latency spike and an OOM risk.

Instead, scans return a **lazily-pulled stream**:

```rust
let mut s = store.scan_from_async::<Record>("session:42", &cursor, 0).await?;
while let Some(item) = s.next().await {       // one row at a time
    let rec = item?;
    handle(rec);                              // process + drop before pulling the next
}
```

The DB yields rows as you ask for them; memory stays flat regardless of how many
rows match. This is **back-pressure**: the consumer's pace controls the producer.
Two stream types express this, one per world:

| World | Type | Pull with |
|-------|------|-----------|
| sync  | `StorageItemStream<'_, V>` (a valtron `Stream` iterator) | `for item in s { … }` |
| async | `AsyncStorageItemStream<'_, V>` (a `futures` stream)      | `while let Some(i) = s.next().await` |

> Bounded reads are the exception: `scan_documents*` return `Vec<Document>` because
> they're explicitly capped ("last N", promoted columns observable). Only the
> *unbounded* scans must stream. (F06 OD-06-6 / OD-06-8.)

---

## 3. The house rule: async is the implementation, sync is a boundary view

Backends implement **only** the async traits; the async method holds the real
logic. A sync caller wraps the store in `Blocking<S>`, which waits on each async
call through valtron at that one boundary: `Blocking::new(SqlDocumentStore::new(turso))`
is a sync `DocumentStore`. Streams stay lazy: `Blocking`'s `scan*` iterators pull
one row per `next()`.

Why this direction? The async impl is the one that *must* exist for serverless
(where `Blocking` doesn't exist, because blocking would freeze the isolate).
Making sync a single generic adapter means one source of truth and no per-backend
sync twins that block on their own futures. (Pure in-memory stores with no I/O —
`MemoryStorage`, `MemoryJsonStore`, `MemoryDocumentStore` — still implement the
sync traits directly too, until foundation_ai's `AgentSession` moves to the async
traits.)

---

## 4. Generic over the backend: write the SQL once

`SqlDocumentStore<Q: AsyncQueryStore>` is generic. It never names Turso, Libsql
or D1 — it only needs "something that can run async SQL." So **one** implementation
serves every SQL backend:

```rust
SqlDocumentStore::new(turso_storage)   // native
SqlDocumentStore::new(libsql_store)    // native
SqlDocumentStore::new(d1_wasm_storage) // Cloudflare Workers
```

The actual SQL strings, parameter lists, and the `SqlRow → Document` projection
live in **one shared `sql` module** used by both SQL document stores (`SqlDocumentStore` and `D1R2DocumentStore`).
That's the anti-drift guarantee: the two stores
can never disagree about the schema, ordering, or `scan_from` cursor because they
are literally building the same statements.

---

## 5. SQLite-family parity

Turso, Libsql, and Cloudflare D1 are all **SQLite dialects**. The promoted-column
schema (migrations `020`/`021`), the `ORDER BY doc_id` scru128 ordering, the
inclusive `doc_id >= ?` cursor, and `LIMIT ? (-1 = unlimited)` are identical across
them. So the conformance suite (in-memory + Turso + Libsql) is the *same* assertions
parameterized over the backend — and F23's Cloudflare D1 reuses
`SqlDocumentStore<D1>` verbatim rather than re-deriving the SQL.

One portability footgun worth knowing: read `COUNT(*)` by **column index 0**, not by
the `as cnt` alias — some async backends don't populate column *names* on the
returned row, only positions.

---

## 6. Mental model in one line

> One generic async impl over `AsyncQueryStore`, returning lazily-pulled streams so
> unbounded scans never OOM; the sync trait wraps it via valtron; the SQL is written
> once and shared, so every SQLite-family backend behaves identically.
