---
feature: "DynNetClient + PreparedRequestBuilder cross-platform HTTP surface"
description: "Foundation primitives for the F51 cross-platform HTTP surface: Uri structured query params (foundation_core); PreparedRequestBuilder query delegates + send() + crate-root re-export; HttpClientBuilder::build() -> DynNetClient (+ H1Transport field update); body_reader split_exchange()/send_and_split()/collect_exchange(); deprecate RequestIntro re-export. No generator or deployment-crate changes — those live in Feature 03."
status: "completed"
priority: "high"
phase: 0
depends_on: ["00-sendsafebody-sync"]
estimated_effort: "medium"
created: 2026-07-12
---
# Feature 01: DynNetClient + PreparedRequestBuilder cross-platform HTTP surface

## Why this exists

F51 (unified network client) introduced the cross-platform HTTP surface:

- **`DynNetClient`** (`Arc<dyn NetClient>`) — platform-agnostic client handle that
  works on native (TCP/TLS/Unix sockets) and wasm32 (browser `fetch` / CF Workers)
  without leaking platform-specific types. Covers both HTTP and WebSocket.
- **`PreparedRequestBuilder`** — cross-platform request builder that produces
  `PreparedRequest` values consumable by any `HttpClient` implementation.
- **`HttpClient` trait** — `send()`, `send_async()`, `send_sse()`, `send_sse_async()`,
  `open_exchange()`.
- **`HttpExchange` / `HttpExchangeClientTask`** — the valtron `TaskIterator`-based
  HTTP exchange type. `open_exchange()` IS the general-purpose HTTP request
  mechanism — not specific to gRPC/ConnectRPC.

At the same time, F51 deprecated `SimpleHttpClient` (now a type alias for
`NativeHttpClient<R>`) and left `ClientRequestBuilder<R>` as a native-only type
generic over `R: DnsResolver`.

This feature fills the **primitive gaps** in that surface so downstream consumers
(the code generator in Feature 03, and the hand-written Docker client in Feature
04/05) have everything they need:

1. `Uri` owns **structured** query params, and `PreparedRequestBuilder` delegates
   to it (`.query()`, `.query_set()`, `.query_remove()`, `.query_params()`).
2. `PreparedRequestBuilder` gains `.send(client)` and is re-exported from the
   crate root.
3. `HttpClientBuilder::build()` returns the canonical `DynNetClient`.
4. `body_reader` gains `split_exchange()` / `send_and_split()` / `collect_exchange()`
   — the split-before-send combinators (per the `H1Transport` pattern) that turn
   an `open_exchange()` task into drainable head/body observers.
5. `RequestIntro` (native-only) is deprecated.

> **Scope boundary (changed 2026-07-12):** the code-generator emit changes and
> the regeneration/migration of all deployment crates were split out into
> **[Feature 03 — generator async fn codegen](../03-generator-async-fn-codegen/feature.md)**,
> which implements [Decision 05](../decisions/05-valtron-task-iterator.md). This
> feature is pure `foundation_core` + `foundation_netio` + `foundation_connectrpc`
> primitive work and touches **no** generated code.

## Current state (baseline)

### The old surface (still in use by generated code)

| Type | Location | Status |
|------|----------|--------|
| `SimpleHttpClient<R>` | `foundation_netio::http` | **Deprecated** alias for `NativeHttpClient<R>` |
| `ClientRequestBuilder<R>` | `foundation_netio::http::request.rs` | Native-only, generic over `R: DnsResolver` |
| `RequestIntro` | `foundation_netio::http::tasks::request_intro.rs` | Native-only response intro enum |

### The new surface (F51, under-used) — what this feature completes

| Type | Location | Status |
|------|----------|--------|
| `DynNetClient` | `foundation_netio::network_client.rs` | `Arc<dyn NetClient>` — HTTP + WebSocket |
| `HttpClient` trait | `shared/client/http_client.rs` | `send()/send_async()/send_sse()/send_sse_async()/open_exchange()` |
| `HttpExchange` / `HttpExchangeClientTask` | `shared/client/request_task.rs` | `Head`/`BodyChunk`/`Failed` task |
| `PreparedRequestBuilder` | `shared/client/request_builder.rs` | Cross-platform builder |
| `HttpClientBuilder` | `network_client.rs` | Platform-agnostic builder |

## Design

### Part A — `foundation_core::url::Uri`: structured query parameters

`foundation_core::url` already has a `Query` type (`query.rs`) with structured
key-value pairs, `append()`, `get()`, `get_all()`, `iter()`, and `Display`
(percent-encodes). But `PathAndQuery` stored `query: Option<String>` — a raw
string. Push structured query handling into `Uri` so it fully owns query semantics;
`PreparedRequestBuilder` then delegates to `Uri` rather than keeping its own store.

#### A1. `PathAndQuery` stores `Query`

`backends/foundation_core/src/url/path.rs`: field `query: Option<String>` →
`query: Query`. `parse()` builds `Query::parse()` from the query substring; an
empty/missing query is `Query::new()`. `query()` accessor returns `Option<String>`
(serializes on demand). `Display` prepends `?` when the query is non-empty
(`Query`'s own `Display` does **not** prepend `?`).

#### A2. `Uri` structured query methods

`backends/foundation_core/src/url/mod.rs`, `impl Uri`:
`query_params() -> &Query`, `query_params_mut() -> &mut Query`,
`with_query_params(Query) -> Uri`, and mutating delegates `append_query(k, v)`,
`set_query(k, v)` (retain-then-append), `remove_query(k)`. `Query` gains
`retain(|k, v| ...)`.

#### A3. Backward compatibility

`Uri::query()` / `PathAndQuery::query()` change return type from `Option<&str>` to
`Option<String>` (owned; allocates only for non-empty queries). Callers already
handle `Option` semantics and mostly pass into `impl Into<String>`. Audit the
handful of `.query()` call sites (netio websocket/request/connection, connectrpc
`H1Transport`) and adjust wrapper signatures as needed.

### Part B — `PreparedRequestBuilder`: delegate query to `Uri`

No new field. New `#[must_use]` delegate methods on `PreparedRequestBuilder`
(`shared/client/request_builder.rs`) mutate the internal `Uri`'s `Query`:

```rust
pub fn query(mut self, key: impl Into<String>, value: Option<impl Into<String>>) -> Self;  // None = no-op
pub fn query_set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self;
pub fn query_remove(mut self, key: &str) -> Self;
pub fn query_params(mut self, query: Query) -> Self;   // replace all
```

`.build()` renders the `Uri` as-is — the query is already structured inside it.

### Part C — `PreparedRequestBuilder`: `send()` + re-export

#### C1. Re-export from `lib.rs`

`foundation_netio/src/lib.rs`:
```rust
pub use shared::client::request_builder::PreparedRequestBuilder;
```
Confirm `PreparedRequest` is also reachable via the crate root.

#### C2. `send()` method

```rust
/// Build the PreparedRequest and open an HTTP exchange via `client`.
/// Cross-platform equivalent of `ClientRequestBuilder::build_send_request()`.
pub fn send(self, client: DynNetClient) -> HttpExchangeClientTask {
    client.open_exchange(self.build())
}
```

`DynNetClient` derefs to `dyn NetClient : HttpClient`, so `.open_exchange()` is
directly callable.

### Part D — `HttpClientBuilder::build()` returns `DynNetClient`

`foundation_netio/src/network_client.rs`: change `build()` return from
`Arc<dyn HttpClient>` to `DynNetClient` (`Arc<dyn NetClient>`). The concrete
clients (`NativeHttpClient`, `FetchHttpClient`) already implement both
`HttpClient` and `WebSocketConnector`, so they satisfy `NetClient` via the blanket
impl and `Arc::new(...)` coerces correctly.

Fallout: `foundation_connectrpc/src/shared/transport/h1.rs` — `H1Transport.client`
field and `H1Transport::new()` parameter change `Arc<dyn HttpClient>` → `DynNetClient`.
`.open_exchange()` still resolves via the `HttpClient` supertrait. Audit other
`build()` callers (netio F51 tests) — they use the client through `HttpClient`
methods, which remain available on `DynNetClient`.

### Part E — body reader: `HttpExchangeClientTask` combinators

`open_exchange()` returns an **unsent** `HttpExchangeClientTask`. The canonical
pattern (from `H1Transport`) splits it into observers + a continuation, then
`valtron::send()`s the continuation. Add three functions to
`foundation_netio/src/shared/client/body_reader.rs`:

#### E1. `split_exchange(client, builder) -> (head, body, task)`

Splits the unsent task via `split_collect_until_map` (head) + `split_collector_map`
(body). Returns the two observers plus the **continuation** (caller sends it).
`head` yields one `Ok((Status, SimpleHeaders))` (or a pre-head `Err`) then closes;
`body` yields `Ok(Bytes)` per `BodyChunk` (or a mid-body `Err`).

#### E2. `send_and_split(client, builder) -> Result<(head, body), HttpClientError>`

Convenience: `split_exchange()` + `valtron::send(task.map_ready(|_| ()))`, returns
just the two observers.

#### E3. `collect_exchange(client, builder) -> Result<SimpleResponse<SendSafeBody>, Arc<dyn Error + Send + Sync>>`

Non-streaming: `send_and_split()`, drain the head observer once (status+headers),
then accumulate body chunks into `BytesMut` and return
`SimpleResponse::new(status, headers, SendSafeBody::Bytes(collected.freeze()))`.

> These combinators are consumed by the **Docker streaming endpoints** (Feature
> 05) and are available to the generator's streaming emit (Feature 03). The
> generator's **non-streaming** path uses `HttpClient::send_async()` directly
> (Decision 05) and does not need `collect_exchange()`, but the function is part
> of this surface for hand-written streaming/collecting callers.

### Part F — deprecate `RequestIntro` re-export

`backends/foundation_deployment/src/providers/common/api_types.rs`:
```rust
#[deprecated(note = "RequestIntro is native-only; use open_exchange() + HttpExchange instead")]
pub use foundation_netio::http::RequestIntro;
```
`RequestIntro` cannot be removed yet (`foundation_ai/src/models/generator.rs`
matches on it directly). Deprecate the re-export; a follow-up migrates
`foundation_ai`.

### Part G — `PreparedRequestBuilder` API parity audit

| Method | `ClientRequestBuilder` | `PreparedRequestBuilder` | Status |
|--------|----------------------|------------------------|--------|
| verb ctors, `header`, `headers`, `body_*`, `bearer_token`, `basic_auth`, `api_key`, `authorization`, `x_api_key` | ✓ | ✓ | OK |
| `query` / `query_set` / `query_remove` / `query_params` | ✗ | ✗ → **Part B** | added |
| `build()` → `PreparedRequest` | ✓ | ✓ | OK |
| `build_send_request()` → `SendRequestTask<R>` | ✓ | — | replaced by `send()` (Part C2) |
| `send(client)` → `HttpExchangeClientTask` | ✗ | ✗ → **Part C2** | added |

## Scope

| Crate | Change |
|-------|--------|
| `foundation_core` | Part A: `PathAndQuery` stores `Query`; `Uri` structured query methods; `Query::retain()` |
| `foundation_netio` | Part B: query delegates; Part C: `send()` + re-export; Part D: `build()` → `DynNetClient`; Part E: `split_exchange()` / `send_and_split()` / `collect_exchange()` |
| `foundation_connectrpc` | Part D fallout: `H1Transport::client` field → `DynNetClient` |
| `foundation_deployment` | Part F: deprecate `RequestIntro` re-export |

### Not touched (moved to Feature 03)

- `foundation_openapi` generator emit.
- Regeneration / migration of cloudflare, stripe, supabase, neon, planetscale,
  prisma, flyio.

## Verification

- `cargo check -p foundation_core` — compiles; `PathAndQuery` stores `Query`;
  `.query("page", Some("1")).query("limit", Some("10")).build()` yields URL query
  `?page=1&limit=10`; `.query_set` replaces; `.query_remove` removes.
- `cargo check -p foundation_netio` (native + `--features wasm-fetch --target wasm32-unknown-unknown`).
- `cargo check -p foundation_connectrpc` — `H1Transport` compiles against `DynNetClient`.
- `use foundation_netio::PreparedRequestBuilder` resolves; `builder.send(client)`
  returns `HttpExchangeClientTask`.
- `HttpClientBuilder::new().build()` returns `DynNetClient`.
- `split_exchange`/`send_and_split`/`collect_exchange` behave per Part E; existing
  netio HTTP tests pass (no regression).

## Acceptance criteria

1. `Uri` owns structured query params (`PathAndQuery` stores `Query`);
   `query_params()`, `append_query`, `set_query`, `remove_query`, `Query::retain()`.
2. `PreparedRequestBuilder` delegates `.query()/.query_set()/.query_remove()/.query_params()` to the internal `Uri`.
3. `PreparedRequestBuilder::send(client)` returns `HttpExchangeClientTask`.
4. `PreparedRequestBuilder` re-exported from `foundation_netio` root.
5. `HttpClientBuilder::build()` returns `DynNetClient`; `H1Transport` updated.
6. `split_exchange()`, `send_and_split()`, `collect_exchange()` exist and follow the `H1Transport` split-before-send pattern.
7. `RequestIntro` re-export is deprecated with a migration note.
8. Existing netio/connectrpc tests pass — no regression; wasm target still compiles.

## Dependencies

- **Feature 00** (`SendSafeBody: Sync`) — body-bearing types are `Sync`, which
  simplifies sharing `SimpleResponse`/`HttpExchange` through split observers.

## Risks

- **`PathAndQuery` field type change** (`Option<String>` → `Query`): callers
  constructing it directly or relying on `query() -> Option<&str>` need updating.
  Blast radius is contained to the `url` module + a few netio/connectrpc call sites.
- **`HttpClientBuilder::build()` return type change**: breaking for callers that
  type-annotate `Arc<dyn HttpClient>`. Audit and update (`H1Transport`, tests).
- **`RequestIntro` cannot be removed**: `foundation_ai` still matches on it —
  deprecate only, follow up separately.
