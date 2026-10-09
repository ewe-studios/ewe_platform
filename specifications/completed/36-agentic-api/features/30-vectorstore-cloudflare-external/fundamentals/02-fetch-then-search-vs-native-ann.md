# Fundamentals 02 — Fetch-then-search vs native ANN

The central trade-off for edge/serverless vector storage. When the backend has a
real ANN index (Doc 01), you let it do the search. When it doesn't — Cloudflare D1
or KV — you **fetch the candidate vectors and search client-side**. This doc
explains both, and the cost that makes fetch-then-search a bounded, best-effort
tool, not a scaling strategy.

---

## 1. Native ANN (the good path)

With TurboPuffer/Pinecone/Vectorize you send a query vector and get back the top-k
ids. The index does sub-linear work; you transfer only the query and a handful of
results. This scales to millions of vectors and is the default whenever the backend
supports it.

## 2. Fetch-then-search (the fallback)

Cloudflare **D1** (SQLite) and **KV** have **no ANN index**. To do a nearest-
neighbor query you must:

1. **Fetch** the candidate vectors for the namespace (a SQL `SELECT` over D1, or a
   `list`+`get` over KV).
2. **Search client-side**: run `foundation_vectors`' flat top-k
   (`flat_top_k`) — compute the distance from the query to every fetched vector and
   keep the best k.

This is exactly what F29's **`SqlVectorStore<Q: QueryStore>`** does, and it's why
the CF D1 vector path needs *no new backend*: `SqlVectorStore` over a D1 query store
stores vectors in a SQL table and re-ranks with flat search. (D1 is SQLite, so the
same generic that serves Turso/libSQL serves D1.)

```
query ──▶ D1: SELECT id, vector FROM vectors WHERE namespace = ?
       ──▶ flat_top_k(query, fetched, k)  ──▶ top-k matches
```

## 3. The cost (why it's bounded, best-effort)

Fetch-then-search is **O(N)** in the namespace size — every query reads and scores
*every* vector in the namespace. That's fine for hundreds or low-thousands of
vectors; it falls over for large sets:

- **Transfer + compute** grow linearly with N. A namespace of 100k vectors means
  fetching 100k vectors per query.
- It's a **hard cap + truncation** situation (OD-30-8): set a maximum candidate
  count; beyond it, recall is **truncated** (you searched only the first N you
  fetched), not merely "slower". The caller must know the answer may be incomplete.

So D1 fetch-then-search is a legitimate edge option for **small namespaces**, and a
**best-effort** one beyond that. For real scale at the edge you want native ANN
(TurboPuffer over HTTP — which works on wasm too, Doc 03).

## 4. KV is worse — and only best-effort

CF **KV** as a vector store inherits every F23 caveat:

- **`list` caps at 1000 keys** with no `startAfter` — you literally cannot enumerate
  a namespace beyond 1000 vectors without a binding fix. Recall is **truncated**,
  not just degraded.
- **Eventually consistent** — a vector you just wrote may not appear in the next
  `list`. No read-after-write.
- No range queries, no transactions.

Hence KV is **not** a primary vector backend in F30 — at most a best-effort cache,
and documented as such. D1 (read-after-write, real `SELECT`) is the viable CF path.

## 5. Choosing per deployment

| You have…                       | Use                                   |
|---------------------------------|----------------------------------------|
| Local/native, durable           | F29 `SqlVectorStore` (Turso/libSQL/fjall) |
| Tests / small RAM workload      | F28 `InMemoryVectorStore`             |
| Serverless, **small** namespace | CF D1 via `SqlVectorStore` (fetch-then-search) |
| Serverless/managed, **scale**   | **TurboPuffer** over HTTP (native ANN, native+wasm) |
| Pinecone/Chroma                 | trait-ready, deferred adapters         |

The trait (Doc 00 §2) is what lets a deployment pick a row of this table without
changing agent code.

---

**Next:** Doc 03 — HTTP transport & platform (one adapter, native + wasm).
