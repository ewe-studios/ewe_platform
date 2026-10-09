# Fundamentals: the Cloudflare DocumentStore (F23)

Zero-to-expert on persisting documents from a Cloudflare Worker — what D1, R2,
and KV actually are, why we pick D1 + R2 (and **not** KV) for the ordered
`DocumentStore` contract, and how `D1R2DocumentStore` ties them together behind
one trait. Read this if you've never deployed to Workers or touched
`foundation_db`'s storage traits.

---

## 1. The Cloudflare Workers runtime in one minute

A Worker is a small program that runs at Cloudflare's edge, close to the user.
The runtime is **`workerd`** (the same engine miniflare/wrangler run locally), and
it has two properties that dominate every storage decision:

1. **Single-threaded, event-loop, Promise-based.** There is no thread to block.
   Every binding (D1, R2, KV) is **async** — you `.await` a JS `Promise`. A
   synchronous "block until done" call would freeze the only thread and deadlock
   the isolate.
2. **No local disk, no long-lived process.** A Worker invocation is short-lived
   and stateless; all durable state lives in *bindings* to managed services.

So the canonical storage trait on Workers is the **async** one
(`AsyncDocumentStore`), and the synchronous `DocumentStore` is a thin valtron
wrapper over it (§7). This is the house rule across the platform: **async holds
the real logic; sync wraps async.**

The three storage bindings you get are **D1**, **R2**, and **KV**. They are not
interchangeable.

---

## 2. D1 — SQLite at the edge (the ordered backbone)

**D1 is SQLite**, exposed as a binding. You send SQL; you get rows back. Because
it's SQLite, it gives you exactly what an ordered `DocumentStore` needs:

- **Strict ordering.** `ORDER BY doc_id ASC` over scru128 ids is chronological
  (scru128 is time-sortable — see F06). That makes `scan` and `scan_from`
  deterministic.
- **Range reads.** `WHERE collection_key = ? AND doc_id >= ? ORDER BY doc_id ASC
  LIMIT ?` is the whole `scan_from` contract in one statement.
- **Read-after-write.** A row you just inserted is visible to the next query —
  no eventual-consistency surprises.
- **Promoted columns.** F06's `documents` schema promotes `title`/`summary`/
  `record_type` to real columns so you can filter/sort without parsing the JSON
  blob.

**The one sharp edge: D1 has no batch DDL.** The binding exposes
`prepare(sql).run()` for a *single* statement. A migration file with several
`CREATE TABLE` / `ALTER TABLE` / `CREATE INDEX` statements will **not** apply as
one call. We own the binding, so F23 adds a **`D1Database.exec()`** wrapper
(`bindgen/cf/d1.rs`) that runs multi-statement DDL, and `init_schema_async` uses
it to apply migrations 020 (create), 021 (promoted columns), and 022 (`r2_key`).
We *add the capability* rather than work around it.

D1 is the **primary, strictly-ordered** Cloudflare backend. If F23 shipped only
D1, it would already be a complete `DocumentStore`.

---

## 3. R2 — S3-style object storage (for the heavy bytes)

**R2** is object storage: `put(key, bytes)`, `get(key)`, `delete(key)`, and
`list({ prefix, cursor, startAfter })`. It has no SQL, no rows, no transactions —
just keyed blobs, but with effectively unbounded object size.

Why bring it in? **Big documents don't fit a SQLite row well.** Message-API
records (F08) can be large; stuffing megabytes of JSON into a D1 `content` column
bloats the database and slows every scan. The fix is the classic **index +
blob-store split**:

> **D1 holds the row** (promoted columns + the blob key); **R2 holds the bytes.**

R2's `list` returns keys in **lexicographic** order, and because scru128 keys are
chronological, `list("doc/{collection}/") + startAfter` gives ordered range reads
directly from R2 if you ever need them. But the **recommended topology** keeps
all ordering/metadata in D1 (so range/typed queries stay relational) and uses R2
purely as the byte store. That's what `D1R2DocumentStore` does.

---

## 4. KV — and why it is **not** a DocumentStore backend

CF **KV** is a globally-replicated key-value cache. It is tempting (it's simple),
but it **cannot** honor the `DocumentStore` contract:

- **Eventually consistent** — no read-after-write. A key you just wrote may not be
  visible yet at the edge you read from. Ordered append→scan parity is impossible.
- **No transactions, no range queries.**
- **`list` caps at 1000 keys**, paginates by an opaque cursor, and has no
  `startAfter` — so you cannot do an inclusive `scan_from`.

So **KV is NOT used as a DocumentStore backend** (resolved, user 2026-06-15). Its
legitimate role is a **fast cache for `MemoryStore` (F07)** — a simple
`memory:{session_id} → latest SessionMemory` get/put, no scanning, no ordering.
That belongs to F07's `KvMemoryStore`, not here. There is deliberately **no
`KvDocumentStore`**.

The rule of thumb: **D1 for ordered/relational, R2 for big blobs, KV for
best-effort cache.** Pick the store to the job.

---

## 5. `D1R2DocumentStore` — transparent blob offload

Putting D1 and R2 together behind one `AsyncDocumentStore`:

```rust
pub struct D1R2DocumentStore<Q, B> {   // Q: AsyncQueryStore, B: AsyncBlobStore
    query_store: Q,   // D1 (the ordered index)
    blob_store: B,    // R2 (large payloads)
    table: String,
}
```

On **append**:

1. Serialize the document to JSON once.
2. If the JSON is larger than the **4 KB threshold** (the SQLite default page
   size — keep D1 rows lean), `put` it to R2 at `doc/{collection}/{doc_id}` and
   remember the key; the D1 `content` column stores a placeholder.
3. Otherwise keep the JSON inline in D1 and leave `r2_key` NULL.
4. Insert the D1 row: `collection_key, doc_id, content, metadata, promoted
   columns, r2_key`.

On **read** (`scan` / `scan_from` / `scan_documents`):

1. D1 returns the ordered rows (including `r2_key`).
2. For each row, if `r2_key` is set, fetch the bytes from R2 and substitute them
   for the placeholder — **transparent read-through**. Otherwise use the inline
   `content`.

On **delete**: look up the row's `r2_key`; if present, delete the R2 object too,
then delete the D1 row. No dangling blobs.

The 4 KB split is the **default topology, not an option** — D1 and R2 are always
used together. The threshold is deliberately small so D1 stays an index and R2
does the heavy lifting.

---

## 6. Why the store is generic (and how we test it)

`D1R2DocumentStore<Q, B>` is **target-agnostic**: it depends only on the
`AsyncQueryStore` + `AsyncBlobStore` traits, not on any wasm type. That buys two
things:

- **On Cloudflare**, instantiate it as
  `D1R2DocumentStore<D1WasmStorage, R2WasmStorage>` (aliased `CfD1R2DocumentStore`)
  over the real CF bindings.
- **Everywhere else**, instantiate it over *any* SQL + blob backends. The F23
  conformance suite runs the exact same store over `TursoStorage` (real in-memory
  SQLite) + `MemoryStorage` (in-memory blobs) and asserts the 4 KB offload, the
  `r2_key` bookkeeping, transparent read-through, scru128 ordering, inclusive
  `scan_from`, and delete cleanup — with **no external infra**, so it runs in CI.

The dedicated **miniflare/wrangler integration test** then drives the same store
over the *native* `D1Store` + `R2Store` REST clients pointed at a local
`wrangler dev` worker (`mise run cf:start` / `test:cf`), validating the actual
Cloudflare D1/R2 wiring end-to-end. Same logic, two backends — the generic store
is what makes both possible.

> Note: the wasm32 build of `foundation_db` under `wasm-bindgen-storage` currently
> has a pre-existing valtron/`Send`-bound gap (tracked with F00e), independent of
> F23's store logic; the generic store itself adds no wasm errors.

---

## 7. Async-first, sync-via-valtron

Workers force async, but native callers often want a synchronous API. The
platform rule: **write the async impl once; wrap it for sync with valtron.**
`valtron`'s block-on/drive primitive runs an async future to completion on a
worker pool, so the sync `DocumentStore` is a few lines delegating to the async
`AsyncDocumentStore`. We never hand-roll a second async→sync bridge (and never
`std::thread::block_on` on wasm, where it would deadlock the isolate). This keeps
one source of truth for the logic and one well-tested bridge.

---

## 8. Read-after-write hazards (the thing that bites you)

The single most common Cloudflare storage bug is assuming consistency you don't
have:

- **D1**: read-after-write **within a Worker invocation** is fine (it's SQLite).
- **R2**: a `put` is durable and immediately readable by key; but **`list` is
  eventually consistent** — a freshly written object may not appear in a `list`
  for a moment. That's why we keep the authoritative ordering in **D1** and use R2
  only for keyed `get`/`put`, never for the ordered scan itself.
- **KV**: eventually consistent for *everything* — never assume a write is visible
  on the next read. (Which is exactly why it isn't a DocumentStore backend.)

Design so that the **ordered, must-be-consistent** path is always D1, and R2/KV
only ever serve keyed lookups where eventual consistency is acceptable.

---

## TL;DR

- Workers are async, single-threaded, diskless → the canonical store is
  `AsyncDocumentStore`; sync wraps it via valtron.
- **D1 = SQLite** → ordered, range-queryable, read-after-write: the backbone.
  Add an `exec()` binding because D1 has no batch DDL.
- **R2 = object storage** → holds documents over 4 KB; D1 keeps the row + `r2_key`.
- **KV = eventually-consistent cache** → never a DocumentStore backend; it's the
  F07 MemoryStore cache only.
- `D1R2DocumentStore<Q, B>` is generic over `AsyncQueryStore` + `AsyncBlobStore`,
  so the same code runs on CF bindings, on native REST clients (miniflare tests),
  and on in-memory backends (CI conformance).
