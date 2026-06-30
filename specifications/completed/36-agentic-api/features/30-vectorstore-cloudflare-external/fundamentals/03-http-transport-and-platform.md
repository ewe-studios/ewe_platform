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

`TurboPufferVectorStore` holds an **`Arc<dyn HttpClient>`** and never names a
concrete client. The deployment injects the right one. That single decision is what
makes the adapter **platform-agnostic** — it builds and runs for both
`x86_64` and `wasm32-unknown-unknown` (verified: the wasm build of the
`turbopuffer` feature is clean).

```rust
let store = TurboPufferVectorStore::new(http_client, api_key, config);
//                                       ^^^^^^^^^^^ Arc<dyn HttpClient>: native or fetch
```

## 3. `Send` on single-threaded wasm (the `?Send` resolution)

`AsyncVectorStore` is a single **`Send`** async trait — *not* `?Send`. But wasm
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

Because the adapter depends only on `Arc<dyn HttpClient>`, tests inject a **mock**
client that records requests and returns canned responses (`turbopuffer_tests.rs`).
That asserts the exact REST contract — URL, method, `Bearer` auth, upsert/query
body shapes, and the `dist → score` mapping — deterministically and offline. The
same seam that makes the adapter portable makes it trivially testable.

---

That's the platform story: **one trait (`AsyncVectorStore`), one HTTP seam
(`HttpClient`), `SendWrapper` to bridge wasm** — so a single TurboPuffer adapter
serves native servers and edge Workers alike, while CF-native D1/KV ride their
bindings for the serverless fetch-then-search path.
