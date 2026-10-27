# Fundamentals 03 — HTTP transport, platform, and credentials

How one TurboPuffer adapter runs on **both** native and wasm, why that needs the
`HttpClient` trait, how `Send` is reconciled on single-threaded wasm, and where the
API key comes from. This is the "make it actually work everywhere" doc.

---

## 1. The problem: HTTP differs by platform

A REST adapter is "just HTTP" — but *how* you make an HTTP call differs:

- **Native** — a real client (sockets, TLS) on a thread pool.
- **Wasm / CF Workers** — there are no sockets; you call the runtime's **`fetch`**
  via `web_sys`, which returns a JS `Promise` you `.await`.

If the adapter hard-coded a native client, it couldn't run on the edge — exactly
where serverless vector search is most wanted. So the adapter must not know which
transport it's using.

## 2. The `HttpClient` trait (F00f)

F00f defines one trait with the same surface on every platform:

```rust
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send_async(&self, req: PreparedRequest) -> Result<SimpleResponse<…>, …>;
    fn send(&self, req: PreparedRequest) -> Result<SimpleResponse<…>, …>;  // sync convenience
    // + SSE variants
}
```

- **Native** impl wraps the platform HTTP client.
- **Wasm** impl is `FetchHttpClient` — `web_sys` `fetch` under the same trait.

`TurboPufferVectorStore` holds a **`DynNetClient`** (`foundation_netio`) and never
names a concrete client. `DynNetClient` is the platform's network client layer:
`HttpClientBuilder::new().build()` gives the native client on native targets and
the fetch client (`FetchHttpClient`) on wasm32. That single decision is what makes
the adapter **platform-agnostic** in its code. On wasm32 the crate takes
`foundation_netio` with only its `wasm-fetch` feature, and CI builds the
`turbopuffer` feature for `wasm32-unknown-unknown`.

```rust
// The default client: native or fetch, from HttpClientBuilder.
let store = TurboPufferVectorStore::new(api_key, config)?;

// A client that the caller built (a shared client, or a test client).
let store = TurboPufferVectorStore::with_client(net_client, api_key, config)?;
//                                              ^^^^^^^^^^ DynNetClient
// Both refuse DistanceMetric::Dot: TurboPuffer has only cosine and L2.
```

Every request is built with `PreparedRequestBuilder`. Every method awaits `HttpClient::send_async`, so no call blocks a worker. The
store checks the status before it reads the body. It reads a 2xx body up to
`TurboPufferLimits::max_response_body` (default 16 MiB) and an error body up to
`TurboPufferLimits::max_error_body` (default 64 KiB).

The adapter uses the TurboPuffer v2 API:

| Method | Request |
|---|---|
| `insert` | `POST /v2/namespaces/{ns}` with `upsert_rows` and `distance_metric` |
| `delete` | `POST /v2/namespaces/{ns}` with `deletes` and `return_affected_ids` (an id that is not deleted is `NotFound`) |
| `search` | `POST /v2/namespaces/{ns}/query` with `rank_by: ["vector", "ANN", q]` |
| `get` | `POST /v2/namespaces/{ns}/query` with `filters: ["id", "Eq", id]` |
| `len` | `POST /v2/namespaces/{ns}/query` with `aggregate_by: {"count": ["Count"]}` |

A query on a namespace that does not exist (HTTP 404) reads as an empty
namespace.

## 3. `Send` on single-threaded wasm (the `?Send` resolution)

`VectorStore` is a single **`Send`** async trait — *not* `?Send`. But wasm
`fetch` futures are `!Send` (they touch JS objects bound to the single thread). How
do both hold?

- On native, the futures are genuinely `Send`.
- On wasm, the `!Send` fetch future is wrapped in **`SendWrapper`** (F00e), which
  asserts `Send` safety on a runtime that has exactly one thread — there's no other
  thread to violate the contract. The future then satisfies the `Send` bound the
  trait requires.

This is the platform-wide rule (F00e / Item #1 / §A1): **one `Send` async trait
everywhere; wasm bridges its `!Send` futures with `SendWrapper`.** It means a
`Send` caller drives the future on Workers with no separate `?Send` trait surface to
reconcile.

## 4. CF bindings are *not* HTTP

One distinction worth keeping straight: the **external** providers (TurboPuffer,
Pinecone, Chroma) speak **HTTP**, so they go through `HttpClient` and work on both
platforms. The **Cloudflare-native** backends (D1, KV, and a hypothetical
Vectorize) use **CF bindings** — `env.DB`, `env.BUCKET` — which are wasm-only JS
objects, not HTTP. So:

- TurboPuffer adapter → `HttpClient` → native **and** wasm.
- CF D1 fetch-then-search → CF D1 binding (or the native D1 REST client for tests)
  → the wasm CF runtime.

Same trait on top; different transport underneath.

## 5. Credentials (never in source)

External providers need an API key. The rules:

- The key is **injected**, never committed — via config or the agent's
  `SessionAccessProvider` (F18) / the model layer's credential management (reuse
  what `foundation_ai` already has rather than inventing a second mechanism —
  OD-30-5).
- The adapter takes the key as a constructor argument (`api_key: String`) and sends
  it as `Authorization: Bearer {key}`. It has no opinion on where the string came
  from — which keeps secrets out of the vector layer entirely.

## 6. Testing without a network

`turbopuffer_tests.rs` starts an in-process mock of the TurboPuffer v2 API on
a local port. The mock keeps rows per namespace and answers writes, vector
queries, id lookups and counts. The store talks to it through the real native
HTTP client. The tests check the request shapes (URL, method, `Bearer` auth,
bodies), the score mapping, the error paths and the body limits. The shared
`VectorStore` contract also runs against the mock. No network leaves the
machine.

---

That's the platform story: **one trait (`VectorStore`), one HTTP seam
(`HttpClient`), `SendWrapper` to bridge wasm** — so a single TurboPuffer adapter
serves native servers and edge Workers alike, while CF-native D1/KV ride their
bindings for the serverless fetch-then-search path.
