# Fundamentals 01 — Managed vector DB data models

What the managed vector databases actually are, how they partition data, and how
F30 maps the trait's `namespace: String` onto each. Read this before Doc 02
(fetch-then-search) — it explains *why* the trait abstracts what it does.

---

## 1. The shared shape

Every vector DB stores **(id, vector, metadata)** triples and answers **top-k
nearest-neighbor** queries under a distance metric (cosine, L2, dot). The
differences are in *partitioning*, *index type*, and *who enforces dimension*.

## 2. Namespaces vs collections (the impedance mismatch)

- **Pinecone / TurboPuffer — namespaces.** One index has a fixed dimension +
  metric; **namespaces** are partitions *inside* it. You don't create a namespace
  explicitly — writing to `ns1` makes it exist. All namespaces share the index's
  dimension/metric.
- **Chroma — collections.** A **collection** is a first-class object with its
  *own* dimension, metric, and lifecycle (create/delete). Closer to "a table".

F28's trait exposes a single `namespace: String`. Each adapter maps it:

| trait `namespace` | TurboPuffer | Pinecone  | Chroma            |
|-------------------|-------------|-----------|-------------------|
| `"sessions"`      | namespace   | namespace | collection name   |

Provisioning (who creates/deletes the index or collection) is provider-specific
(OD-30-7): TurboPuffer/Pinecone namespaces are implicit; Chroma collections need
explicit creation. The adapter owns that detail so the agent never sees it.

## 3. Dimension enforcement: local vs server-side

F28's local stores check vector dimension **at insert time** (a fast, synchronous
guard). Managed providers configure dimension on the **index/collection** and
reject mismatches **server-side, asynchronously** — you find out when the HTTP
call fails, not before. So external adapters deviate from F28's insert-time check:
the adapter still validates what it cheaply can (our `TurboPufferVectorStore`
checks dimension and zero-vectors before sending — see its tests), but the
authoritative dimension contract lives on the server.

## 4. ANN indexes (why these DBs are fast)

Managed DBs build an **approximate nearest neighbor (ANN)** index (HNSW, IVF,
DiskANN, …) so top-k doesn't scan every vector. You send a query vector; the server
walks its index and returns the k closest ids + distances. This is **native ANN** —
the contrast in Doc 02 is doing the search yourself ("fetch-then-search") when the
backend has no ANN.

- **TurboPuffer** — serverless, object-storage-backed ANN with a simple REST API
  (`upsert` / `query` / `delete` per namespace). Cheap at rest, pay-per-query;
  great fit for the agent's intermittent workload. **This is the one F30 ships.**
- **Pinecone** — managed ANN, namespace partitions, gRPC/REST. *Deferred* (trait-
  ready).
- **Chroma** — open-source, collection-per-dataset, REST. *Deferred* (trait-ready).
- **CF Vectorize** — Cloudflare's native ANN. Would be the preferred edge path, but
  has **no runtime Worker binding** today (Doc 00 §3), so it's not used.

## 5. The TurboPuffer adapter, concretely

`TurboPufferVectorStore` holds an `Arc<dyn HttpClient>` (Doc 03), an API key, a
base URL, and a `VectorStoreConfig` (dimension + metric). Each trait method becomes
one REST call:

- `insert` → `POST /v1/vectors/{ns}` with an **upsert** body (`ids`, `vectors`,
  `attributes.metadata`).
- `search` → `POST /v1/vectors/{ns}/query` with `{vector, top_k, distance_metric,
  include_attributes}`; the response gives `(id, dist)` pairs.
- `delete` → `POST /v1/vectors/{ns}` with a delete body.

**Distance → score:** TurboPuffer returns *distances*; the agent wants
*similarity*. The adapter maps `score = 1.0 - dist` so larger = more similar,
consistent with the other backends. The metric name is translated
(`Cosine → "cosine_distance"`, `L2 → "euclidean_squared"`).

Auth is a `Bearer {api_key}` header (Doc 03 §4 on where the key comes from). All of
this is asserted by `turbopuffer_tests.rs` against a mock `HttpClient` — no network.

---

**Next:** Doc 02 — fetch-then-search vs native ANN (the CF D1/KV path and its cost).
