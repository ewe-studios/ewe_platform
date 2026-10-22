# Fundamentals: the async SQL DocumentStore (F22b)

Zero-to-expert on `SqlDocumentStore` and the ideas behind it. Read this if
you've never touched `foundation_db`'s storage traits.

---

## 1. One trait, async: `AsyncQueryStore`

SQL access is a single async trait:

```rust
#[async_trait]
trait AsyncQueryStore {
    async fn query_async(&self, sql, params) -> StorageResult<AsyncQueryStream>;
    async fn execute_async(&self, sql, params) -> StorageResult<u64>;
    async fn execute_batch_async(&self, sql) -> StorageResult<()>;
}
```

Why async only? Because serverless/wasm (Cloudflare D1) is **Promise-based and
single-threaded**: you physically cannot block; you must `.await`. A blocking
call there would deadlock the event loop. Native backends (Turso/libsql) run
the same async code; a native program that must be sync waits once, at its
entry point. (Until spec 50 F04 there was also a sync `QueryStore` twin; it is
gone. The `_async` suffixes go in a follow-up rename.)

One layer up, **`AsyncDocumentStore`** is the document store trait;
`SqlDocumentStore<Q: AsyncQueryStore>` implements it for any SQL backend.

Backends implement only the async traits, and there are no sync twins (the
old sync traits and the `Blocking<S>` wrapper were removed in spec 50 F04).
Sync code awaits at its entry point with `block_on_future` / `.block()`
behind `blocking_boundary!` (see `00-storage-traits.md`).

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

## 3. The house rule: async all the way, sync only at the entry point

Backends implement **only** the async traits, and so do their callers: code
that uses a store is itself `async` and `.await`s it. The one place that
waits is a true sync entry point (a `main`, an OS thread), with valtron's
`block_on_future` / `.block()` behind `blocking_boundary!`; on JS-hosted wasm
there is no such place, and everything `.await`s up to the Worker's entry.
Streams stay lazy end to end: a scan is pulled one row per `.next().await`.

The pure in-memory stores (`MemoryStorage`, `MemoryJsonStore`,
`MemoryDocumentStore`) follow the same rule: async impls only, whose futures
are ready at once.

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
