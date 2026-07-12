---
feature: "DynNetClient + PreparedRequestBuilder alignment for code generator and deployment crates"
description: "Migrate foundation_openapi code generator and all deployment crates from deprecated SimpleHttpClient<R>/ClientRequestBuilder<R> to cross-platform DynNetClient/PreparedRequestBuilder; add missing PreparedRequestBuilder methods (structured query params, re-export, send); use open_exchange() directly for TaskIterator-based HTTP"
status: "in-progress"
priority: "high"
phase: 0
depends_on: []
estimated_effort: "medium"
created: 2026-07-12
---
# Feature 00: DynNetClient + PreparedRequestBuilder alignment

## Why this exists

F51 (unified network client) introduced the cross-platform HTTP surface:

- **`DynNetClient`** (`Arc<dyn NetClient>`) — platform-agnostic client handle that works on
  native (TCP/TLS) and wasm32 (browser `fetch` / CF Workers) without leaking
  platform-specific types. Covers both HTTP and WebSocket.
- **`PreparedRequestBuilder`** — cross-platform request builder that produces
  `PreparedRequest` values consumable by any `HttpClient` implementation.
- **`HttpClient` trait** — `send()`, `send_async()`, `send_sse()`, `send_sse_async()`,
  `open_exchange()`.
- **`HttpExchange` / `HttpExchangeClientTask`** — the valtron `TaskIterator`-based HTTP
  exchange type. `open_exchange()` IS the general-purpose HTTP request mechanism — it's
  not specific to gRPC/ConnectRPC.

At the same time, F51 deprecated `SimpleHttpClient` (now a type alias for
`NativeHttpClient<R>`) and left `ClientRequestBuilder<R>` as a native-only type
generic over `R: DnsResolver`.

However, the **code generator** (`foundation_openapi`) and **all deployment crates**
(cloudflare, stripe, supabase, neon, planetscale, prisma, flyio, and the in-progress
Docker crate) still generate/use the old surface:

```rust
use foundation_netio::http::{ClientRequestBuilder, SimpleHttpClient};

pub fn get_applications_request<R, F>(
    client: &SimpleHttpClient<R>,
    args: &GetApplicationsArgs,
    builder_mod: Option<F>,
) -> Result<impl TaskIterator<...>, ApiError>
where
    R: foundation_netio::shared::client::DnsResolver + Clone + Default + 'static,
    F: FnOnce(&mut ClientRequestBuilder<R>),
```

This leaks the `R: DnsResolver` generic through every generated function, ties callers
to the native-only `ClientRequestBuilder<R>`, and uses a deprecated type. The
generated code also depends on `RequestIntro` (a native-only response intro enum) and
`build_send_request()` (a native-only method that returns `SendRequestTask<R>`).

**Goal**: Align the code generator and all deployment crates with the F51 cross-platform
surface so that:

1. Generated functions accept `DynNetClient` (no resolver generic, supports WebSocket).
2. `PreparedRequestBuilder` is used instead of `ClientRequestBuilder<R>`.
3. Generated code calls `client.open_exchange()` directly and uses existing
   `body_reader` helpers to process the `HttpExchange` stream — no wrapper needed.
4. `PreparedRequestBuilder` gains missing methods: structured query params,
   re-export, `send()` equivalent.

## Current state (baseline)

### The old surface (still in use)

| Type | Location | Status |
|------|----------|--------|
| `SimpleHttpClient<R>` | `foundation_netio::http` | **Deprecated** alias for `NativeHttpClient<R>` |
| `ClientRequestBuilder<R>` | `foundation_netio::http::request.rs:18` | Native-only, generic over `R: DnsResolver` |
| `ClientRequestBuilder::build_send_request()` | `request.rs:79` | Returns `SendRequestTask<R>` (native TaskIterator) |
| `RequestIntro` | `foundation_netio::http::tasks::request_intro.rs:47` | Native-only response intro enum (`Success{stream,conn,intro,headers}` / `Failed`) |
| `NativeHttpClient::get(url)` / `.post(url)` etc. | `http_client_impl.rs:396-444` | Verb methods return `ClientRequestBuilder<R>` |

### The new surface (F51, under-used)

We use `DynNetClient` so APIs can support WebSocket usage as well as HTTP.

| Type | Location | Status |
|------|----------|--------|
| `DynNetClient` | `foundation_netio::network_client.rs:41` | `Arc<dyn NetClient>` — platform-agnostic, HTTP + WebSocket |
| `NetClient` trait | `network_client.rs:37` | `HttpClient + WebSocketConnector` |
| `HttpClient` trait | `shared/client/http_client.rs:50` | `send()`, `send_async()`, `send_sse()`, `send_sse_async()`, `open_exchange()` |
| `HttpExchangeClientTask` | `shared/client/request_task.rs:50` | `Box<dyn TaskIterator<Ready=HttpExchange, ...>>` |
| `HttpExchange` | `request_task.rs:22` | `Head{status,headers}` / `BodyChunk(Bytes)` / `Failed(Arc<dyn Error>)` |
| `PreparedRequestBuilder` | `shared/client/request_builder.rs:21` | Cross-platform builder, builds `PreparedRequest` |
| `PreparedRequest` | `shared/client/request.rs:15` | Data struct: method, url, headers, body, extensions |
| `HttpClientBuilder` | `network_client.rs:55` | Platform-agnostic builder |

### Who uses the old surface

| Crate / File | Old types used |
|--------------|---------------|
| `foundation_openapi/src/unified/generator.rs:814` | Emits `use foundation_netio::http::{ClientRequestBuilder, SimpleHttpClient}` |
| `foundation_openapi/src/unified/generator.rs:1332` | Emits `client: &SimpleHttpClient<R>` parameter |
| `foundation_openapi/src/unified/generator.rs:1337-1338` | Emits `R: DnsResolver` bound + `F: FnOnce(&mut ClientRequestBuilder<R>)` |
| `foundation_openapi/src/unified/generator.rs:1378` | Emits `client.{method}(&endpoint_url)` |
| `foundation_openapi/src/unified/generator.rs:1403` | Emits `.build_send_request()` |
| `foundation_deployment_cloudflare/src/client.rs:4,9` | `SimpleHttpClient` field + `from_system()` |
| `foundation_deployment_cloudflare/src/generated/*/mod.rs` | ~40 generated modules using old pattern |
| `foundation_deployment_stripe/src/*/mod.rs` | ~100+ generated modules using old pattern |
| `foundation_deployment_supabase/src/*/mod.rs` | Same pattern |
| `foundation_deployment_neon/src/mod.rs` | Same pattern |
| `foundation_deployment_planetscale/src/mod.rs` | Same pattern |
| `foundation_deployment_prisma/src/mod.rs` | Same pattern |
| `foundation_deployment_flyio/src/mod.rs` | Same pattern |
| `foundation_deployment/src/providers/common/api_types.rs:15` | Re-exports `foundation_netio::http::RequestIntro` |

## Design

### Part A — `foundation_core::url::Uri`: structured query parameters

`foundation_core::url` already has a `Query` type (`query.rs`) with structured
key-value pairs, `append()`, `get()`, `get_all()`, `iter()`, and `Display`
(percent-encodes). But `PathAndQuery` stores `query: Option<String>` — a raw string.
The `Query` struct is only used in `UriBuilder::query(Query)`, not on `Uri` itself.

Push structured query handling all the way into `Uri` so it fully owns query
parameter semantics. `PreparedRequestBuilder` delegates to `Uri`'s methods rather
than storing its own `Vec<(String, String)>`.

#### A1. `PathAndQuery` — store `Query` instead of `Option<String>`

File: `backends/foundation_core/src/url/path.rs`

**Before:**
```rust
pub struct PathAndQuery {
    pub path: String,
    pub query: Option<String>,
}
```

**After:**
```rust
pub struct PathAndQuery {
    pub path: String,
    pub query: Query,  // was Option<String>
}
```

Parse by constructing `Query::parse()` from the query substring. An empty/missing
query is just `Query::new()` (`.is_empty() == true`).

`Display` impl changes from `write!(f, "?{query}")` to `write!(f, "{query}")` —
`Query`'s own `Display` impl already prepends `?` when non-empty.

#### A2. `Uri` — structured query methods

File: `backends/foundation_core/src/url/mod.rs`

Add to `impl Uri`:

```rust
/// Returns a reference to the structured query parameters.
#[must_use]
pub fn query_params(&self) -> &Query {
    &self.path_and_query.query
}

/// Returns a mutable reference to the structured query parameters.
#[must_use]
pub fn query_params_mut(&mut self) -> &mut Query {
    &mut self.path_and_query.query
}

/// Returns a new `Uri` with the given query params.
#[must_use]
pub fn with_query_params(&self, query: Query) -> Self {
    let path_and_query = PathAndQuery {
        path: self.path_and_query.path().to_string(),
        query,
    };
    Uri {
        scheme: self.scheme.clone(),
        authority: self.authority.clone(),
        path_and_query,
        fragment: self.fragment.clone(),
    }
}

// Convenience delegate methods — callers don't need to reach into PathAndQuery:

/// Append a query parameter.
pub fn append_query(&mut self, key: impl Into<String>, value: impl Into<String>) {
    self.path_and_query.query.append(key, value);
}

/// Set a query parameter, removing any prior values for the key.
pub fn set_query(&mut self, key: impl Into<String>, value: impl Into<String>) {
    let k = key.into();
    self.path_and_query.query.retain(|existing| existing != &k);
    self.path_and_query.query.append(k, value);
}

/// Remove all query params for the given key.
pub fn remove_query(&mut self, key: &str) {
    self.path_and_query.query.retain(|k, _| k != key);
}
```

Also add `retain` to `Query` (`query.rs`):
```rust
/// Retain only the key-value pairs satisfying the predicate.
pub fn retain(&mut self, f: impl Fn(&str) -> bool) {
    self.pairs.retain(|(k, _)| f(k));
}
```

The existing `self.query()` accessor (returns `Option<&str>`) stays for backward
compat — it serializes the `Query` to a string on the fly.

#### A3. Backward compatibility

`PathAndQuery::query()` currently returns `Option<&str>`. After the change the
internal field is `Query`, so this method needs updating:

```rust
pub fn query(&self) -> Option<&str> {
    // Return None if empty; callers expect Option<&str>
    if self.query.is_empty() {
        None
    } else {
        // Cached string representation? Or allocate on demand.
        // The Display impl for Query produces the query string.
        None  // TODO: design decision — see below
    }
}
```

**Decision**: Change `query()` to return `Option<String>` (owned) rather than
`Option<&str>`. It allocates when the query is non-empty. This is a minor
breaking change but every caller already handles `Option` semantics. Audit
existing callers — most pass the result to `Uri::with_query()` which takes
`impl Into<String>` anyway.

Alternatively, add a cached string field that regenerates when the `Query`
is mutated. But that adds complexity for a hot-path that isn't hot.

**Decision**: Keep it simple. `query()` returns `Option<String>` (allocates
for non-empty queries). The primary API for structured access is `query_params()`.
New code uses `query_params()`; old code that just passes query strings around
uses `query()`. This is fine — URIs aren't constructed in hot loops.

### Part B — `PreparedRequestBuilder`: delegate query to `Uri`

With `Uri` owning structured query params, `PreparedRequestBuilder` doesn't need
its own storage. It delegates query manipulation to the internal `Uri`.

Remove the `query_params: Vec<(String, String)>` field idea. Instead:

**No new field on `PreparedRequestBuilder`.** The existing `url: Option<Uri>` field
already carries query params via `Uri::query_params()`.

New delegate methods on `PreparedRequestBuilder` (`shared/client/request_builder.rs`):

```rust
/// Append a query parameter. If `value` is `None`, this is a no-op.
#[must_use]
pub fn query(mut self, key: impl Into<String>, value: Option<impl Into<String>>) -> Self {
    if let Some(v) = value {
        if let Some(ref mut uri) = self.url {
            uri.append_query(key, v);
        }
    }
    self
}

/// Set a query parameter, removing any existing values for the same key.
#[must_use]
pub fn query_set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    if let Some(ref mut uri) = self.url {
        uri.set_query(key, value);
    }
    self
}

/// Remove all query params for the given key.
#[must_use]
pub fn query_remove(mut self, key: &str) -> Self {
    if let Some(ref mut uri) = self.url {
        uri.remove_query(key);
    }
    self
}

/// Replace all query params.
#[must_use]
pub fn query_params(mut self, query: Query) -> Self {
    if let Some(ref mut uri) = self.url {
        *uri = uri.with_query_params(query);
    }
    self
}
```

`.build()` renders the `Uri` as-is — the query is already structured inside it.
No separate rendering step needed.

Generator usage:
```rust
let mut builder = PreparedRequestBuilder::get(&endpoint_url)?
    .query("filter", args.filter.as_deref())
    .query("limit", args.limit.as_deref())
    .query("offset", args.offset.as_deref());
```

### Part C — `PreparedRequestBuilder`: re-export + send

#### C1. Re-export from `lib.rs`

`PreparedRequestBuilder` is not re-exported from the crate root. Add to
`foundation_netio/src/lib.rs`:
```rust
pub use shared::client::request_builder::PreparedRequestBuilder;
```

Also ensure `PreparedRequest` is re-exported from the crate root (it's already
re-exported in `shared/client/mod.rs:26`, verify it's accessible via `lib.rs`).

#### C2. `send()` method on `PreparedRequestBuilder`

`ClientRequestBuilder` has `build_send_request()` which returns `SendRequestTask<R>`
(a `TaskIterator`). `PreparedRequestBuilder` needs the equivalent: build the
`PreparedRequest` and call `open_exchange()` on a client.

Add to `PreparedRequestBuilder`:
```rust
/// Consume the builder, build a `PreparedRequest`, and open an HTTP exchange
/// via the given client.
///
/// Returns an `HttpExchangeClientTask` — a `TaskIterator` that yields
/// `HttpExchange` variants (`Head`, `BodyChunk`, `Failed`).
///
/// This is the cross-platform equivalent of the old
/// `ClientRequestBuilder::build_send_request()`.
pub fn send(self, client: DynNetClient) -> HttpExchangeClientTask {
    let req = self.build();
    client.open_exchange(req)
}
```

`DynNetClient` derefs to `dyn NetClient` which implements `HttpClient`, so
`.open_exchange()` is directly callable.

### Part D — `HttpClientBuilder::build()` returns `DynNetClient`

`HttpClientBuilder::build()` currently returns `Arc<dyn HttpClient>`. Change it to
return `DynNetClient` (`Arc<dyn NetClient>`).

File: `foundation_netio/src/network_client.rs`

**Before:**
```rust
pub fn build(self) -> Arc<dyn HttpClient> {
```

**After:**
```rust
pub fn build(self) -> DynNetClient {
```

The concrete types (`NativeHttpClient`, `FetchHttpClient`) already implement both
`HttpClient` and `WebSocketConnector`, so they satisfy `NetClient` via the blanket
impl. The `Arc::new(...)` calls inside `build()` coerce correctly.

This also requires updating `H1Transport` (`foundation_connectrpc/src/shared/transport/h1.rs`):

**Before (line 38-39):**
```rust
pub struct H1Transport {
    client: Arc<dyn HttpClient>,
```

**After:**
```rust
pub struct H1Transport {
    client: DynNetClient,
```

Trait upcasting (`DynNetClient` → `Arc<dyn HttpClient>`) is stable since Rust 1.76,
so existing code that stores `Arc<dyn HttpClient>` from `build()` continues to work.
But the canonical type is now `DynNetClient` everywhere.

### Part E — Code generator changes

File: `backends/foundation_openapi/src/unified/generator.rs`

#### E1. Import line (line 814)

**Before:**
```rust
writeln!(out, "use foundation_netio::http::{{ClientRequestBuilder, SimpleHttpClient}};")?;
```

**After:**
```rust
writeln!(out, "use foundation_netio::{{DynNetClient, PreparedRequestBuilder}};")?;
```

#### E2. Function signature (lines 1330-1338)

**Before:**
```rust
writeln!(out, "pub fn {}_request<R, F>(", fn_prefix)?;
writeln!(out, "    client: &SimpleHttpClient<R>,")?;
writeln!(out, "    args: &{},", args_name)?;
writeln!(out, "    builder_mod: Option<F>,")?;
writeln!(out, ") -> Result<impl TaskIterator<...>, super::shared::ApiError>")?;
writeln!(out, "where")?;
writeln!(out, "    R: foundation_netio::shared::client::DnsResolver + Clone + Default + 'static,")?;
writeln!(out, "    F: FnOnce(&mut ClientRequestBuilder<R>),")?;
```

**After:**
```rust
writeln!(out, "pub fn {}_request<F>(", fn_prefix)?;
writeln!(out, "    client: DynNetClient,")?;
writeln!(out, "    args: &{},", args_name)?;
writeln!(out, "    builder_mod: Option<F>,")?;
writeln!(out, ") -> Result<impl TaskIterator<...>, super::shared::ApiError>")?;
writeln!(out, "where")?;
writeln!(out, "    F: FnOnce(&mut PreparedRequestBuilder),")?;
```

No `R` generic. No `DnsResolver` bound.

#### E3. URL + query building (lines 1341-1372)

The generator currently mutates a `String` to append query params. With structured
query params on `PreparedRequestBuilder`, this collapses to a builder chain:

```rust
// Build the URL (base + path with format! substitutions for path params)
let endpoint_url = format!("{}{}", escaped_base, escaped_path, /* path params */);

let mut builder = PreparedRequestBuilder::{method}(&endpoint_url)?
    {for each query_param:
        .query("{qp}", args.{safe_qp}.as_ref())
    };
```

The ~15-line manual query-param block is eliminated. If query param support in
netio needs enhancement to handle this ergonomically, enhance it.

#### E4. Request building (lines 1374-1398)

**Before:**
```rust
let mut builder = client.{method}(&endpoint_url)
    .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;

// body
builder = builder.body_json(&args.body)
    .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;

// builder_mod
if let Some(f) = builder_mod {
    f(&mut builder);
}
```

**After:**
```rust
let mut builder = PreparedRequestBuilder::{method}(&endpoint_url)
    .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;

// body
builder = builder.body_json(&args.body)
    .map_err(|e| super::shared::ApiError::RequestBuildFailed(e.to_string()))?;

// builder_mod
if let Some(f) = builder_mod {
    f(&mut builder);
}
```

`builder_mod` closure type: `FnOnce(&mut PreparedRequestBuilder)` — same methods
(`.header()`, `.bearer_token()`, etc.), no `R` generic.

#### E5. Send + process response via `open_exchange()` (lines 1400-1436)

**Before:**
```rust
Ok(
    builder
        .build_send_request()
        .map_err(|e: HttpClientError| ...)?
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, intro, headers, .. } => {
                let status = intro.0.into();
                if status < 200 || status >= 300 { ... }
                let body = foundation_netio::shared::client::body_reader::collect_string(stream);
                let parsed: ReturnType = serde_json::from_str(&body)?;
                Ok(ApiResponse { status, headers, body: parsed })
            }
            RequestIntro::Failed(e) => Err(...),
        })
        .map_pending(|_| ApiPending::Sending)
)
```

**After** — use `body_reader::collect_exchange()` (see Part I3 for the full
implementation). It handles split + send + collect, returns
`Result<SimpleResponse<SendSafeBody>, Error>` directly:

```rust
// collect_exchange() handles split → send → drain head → collect body.
// Returns a plain Result — wrap in a single-Ready TaskIterator.
let result = body_reader::collect_exchange(client, builder);

Ok(SingleReadyTask::new(result.map(|response| {
    let status: u16 = response.status().into();
    let headers = response.headers().clone();
    if status < 200 || status >= 300 {
        return Err(ApiError::HttpStatus { code: status, headers: headers.clone(), body: None });
    }
    let body_bytes = response.body().as_bytes();
    let parsed: ReturnType = serde_json::from_slice(body_bytes)
        .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
    Ok(ApiResponse { status, headers: headers.clone(), body: parsed })
}).map_err(|e| ApiError::RequestSendFailed(e.to_string()))))
```

For streaming endpoints, use `body_reader::split_exchange()` instead — see Part I6.

**Streaming responses**: For endpoints that return streaming responses (logs, events,
SSE), the generated code should NOT collect the body. Instead, it should return the
`HttpExchangeClientTask` (or a mapped version) directly, letting the caller
stream chunks. The generator already distinguishes streaming vs. non-streaming
endpoints — this distinction is preserved.

Note: `RequestIntro` is no longer referenced in generated code.

### Part F — `foundation_deployment` shared types

File: `backends/foundation_deployment/src/providers/common/api_types.rs`

#### F1. Deprecate `RequestIntro` re-export (line 15)

**Before:**
```rust
pub use foundation_netio::http::RequestIntro;
```

**After:**
```rust
#[deprecated(note = "RequestIntro is native-only; use open_exchange() + HttpExchange instead")]
pub use foundation_netio::http::RequestIntro;
```

`RequestIntro` cannot be removed yet — `foundation_ai/src/models/generator.rs:8`
imports and matches on it directly. Deprecate the re-export so existing code compiles
with a warning, and generated code stops referencing it. A follow-up feature should
migrate `foundation_ai` off `RequestIntro`.

### Part G — `foundation_deployment_cloudflare` client migration

File: `backends/foundation_deployment_cloudflare/src/client.rs`

#### G1. Replace `SimpleHttpClient` with `DynNetClient`

**Before:**
```rust
use foundation_netio::http::SimpleHttpClient;

pub struct CloudflareClient {
    http: SimpleHttpClient,
    token: String,
    zone_id: String,
    domain: String,
}

impl CloudflareClient {
    pub fn from_env() -> Result<Self, CloudflareError> {
        Ok(Self { http: SimpleHttpClient::from_system(), token, zone_id, domain })
    }

    pub fn new(token: String, zone_id: String) -> Self {
        Self { http: SimpleHttpClient::from_system(), token, zone_id, domain: String::new() }
    }

    #[must_use] pub fn http(&self) -> &SimpleHttpClient { &self.http }
}
```

**After:**
```rust
use foundation_netio::{DynNetClient, HttpClientBuilder};

pub struct CloudflareClient {
    http: DynNetClient,
    token: String,
    zone_id: String,
    domain: String,
}

impl CloudflareClient {
    pub fn from_env() -> Result<Self, CloudflareError> {
        Ok(Self { http: HttpClientBuilder::new().build(), token, zone_id, domain })
    }

    pub fn new(token: String, zone_id: String) -> Self {
        Self { http: HttpClientBuilder::new().build(), token, zone_id, domain: String::new() }
    }

    /// Create with an externally-provided client (for testing or custom config).
    pub fn with_client(http: DynNetClient, token: String, zone_id: String) -> Self {
        Self { http, token, zone_id, domain: String::new() }
    }

    #[must_use] pub fn http(&self) -> DynNetClient { self.http.clone() }
}
```

`HttpClientBuilder::new().build()` now returns `DynNetClient` (Part B).
All callers (generated code, `dns_ops.rs`, etc.) use `DynNetClient` consistently.

#### G2. `dns_ops.rs` — update `auth_mod` signature

**Before:**
```rust
fn auth_mod(token: &str) -> impl FnOnce(&mut ClientRequestBuilder<SystemDnsResolver>) + '_ {
    move |b: &mut foundation_netio::http::ClientRequestBuilder<_>| {
        b.header(SimpleHeader::AUTHORIZATION, format!("Bearer {t}"));
    }
}
```

**After:**
```rust
fn auth_mod(token: &str) -> impl FnOnce(&mut PreparedRequestBuilder) + '_ {
    move |b: &mut PreparedRequestBuilder| {
        b.header(SimpleHeader::AUTHORIZATION, format!("Bearer {t}"));
    }
}
```

No more `SystemDnsResolver` generic. Same `.header()` method on both builders.

#### G3. Regenerate all `generated/` modules

After the generator changes (Part C), re-run:
```
cargo run --bin ewe_platform gen_api generate cloudflare
```
This regenerates all ~40 modules under `generated/` with the new signatures.

### Part H — Other deployment crates

| Crate | Migration |
|-------|-----------|
| `foundation_deployment_stripe` | Regenerate; update hand-written client if any |
| `foundation_deployment_supabase` | Regenerate; update hand-written client if any |
| `foundation_deployment_neon` | Regenerate; update hand-written client if any |
| `foundation_deployment_planetscale` | Regenerate; update hand-written client if any |
| `foundation_deployment_prisma` | Regenerate; update hand-written client if any |
| `foundation_deployment_flyio` | Regenerate; update hand-written client if any |
| `foundation_deployment_docker` (spec 54) | Uses the new surface from the start |

### Part I — body reader: `HttpExchangeClientTask` combinators

`open_exchange()` returns `HttpExchangeClientTask` — a `Box<dyn TaskIterator>`
that has NOT been sent to valtron:

```rust
pub enum HttpExchange {
    Head { status: Status, headers: SimpleHeaders },
    BodyChunk(Bytes),
    Failed(Arc<dyn Error + Send + Sync + 'static>),
}
```

The canonical pattern (from `H1Transport`, lines 95-138): get the task,
split it into observers + continuation, then `valtron::send()` the continuation.
The observers drain from internal `ConcurrentQueue`s as the continuation runs.

#### I1. `split_exchange()` — split, don't send

Splits the unsent `HttpExchangeClientTask` via `split_collect_until_map` +
`split_collector_map`. Returns the **continuation** `TaskIterator` (for the
caller to send) plus the two observer `StreamIterator`s.

```rust
use bytes::{Bytes, BytesMut};
use foundation_core::valtron::{CollectionState, TaskIteratorExt};
use foundation_netio::shared::client::request_task::HttpExchange;

/// Split an unsent `HttpExchangeClientTask` into a continuation + two
/// observers. **Does not send to valtron** — the caller controls when
/// and how the continuation is executed.
///
/// Returns `(head, body, task)`:
/// - `head` — yields one `Ok((status, headers))` then closes
/// - `body` — yields `Ok(Bytes)` per chunk, unwrapping `BodyChunk`
/// - `task` — the continuation; caller sends this to valtron
///
/// Use this when the caller wants to chain further `.map_ready()` /
/// `.map_pending()` on the continuation before sending.
pub fn split_exchange(
    client: DynNetClient,
    builder: PreparedRequestBuilder,
) -> (
    CollectorStreamIterator<
        Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>,
        (),
    >,
    CollectorStreamIterator<
        Result<Bytes, Arc<dyn Error + Send + Sync>>,
        (),
    >,
    Box<dyn TaskIterator<
        Ready = HttpExchange,
        Pending = HttpExchangePending,
        Spawner = BoxedSendExecutionAction,
    > + Send>,
) {
    let pump = builder.send(client);

    let (head, head_cont) = pump.split_collect_until_map(
        |item: &HttpExchange| match item {
            HttpExchange::Head { status, headers } => (
                CollectionState::Close(true),
                Some(Ok((status.clone(), headers.clone()))),
            ),
            HttpExchange::Failed(err) => (
                CollectionState::Close(true),
                Some(Err(Arc::clone(err))),
            ),
            HttpExchange::BodyChunk(_) => (CollectionState::Skip, None),
        },
        1,
    );

    let (body, body_cont) = head_cont.split_collector_map(
        |item: &HttpExchange| match item {
            HttpExchange::BodyChunk(bytes) => (true, Some(Ok(bytes.clone()))),
            HttpExchange::Failed(err) => (true, Some(Err(Arc::clone(err)))),
            HttpExchange::Head { .. } => (false, None),
        },
        DEFAULT_PUSHABLE_DEPTH,
    );

    (head, body, Box::new(body_cont))
}
```

Caller usage (same as `H1Transport`):
```rust
let (head, body, task) = split_exchange(client, builder);
valtron::send(task.map_ready(|_| ()))?;
// Drain head first (check status), then body (stream chunks).
let head_item = head.next();
let body_chunk = body.next();
```

#### I2. `send_and_split()` — split + send, return observers

Convenience wrapper: calls `split_exchange()`, sends the continuation to
valtron, returns just the two observers.

```rust
/// Split an exchange AND send the continuation to valtron.
///
/// Returns the head + body observers ready to drain. This is the common
/// case — most callers don't need to inspect the continuation.
///
/// # Errors
///
/// Returns `HttpClientError` if `valtron::send()` fails.
pub fn send_and_split(
    client: DynNetClient,
    builder: PreparedRequestBuilder,
) -> Result<
    (
        CollectorStreamIterator<
            Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>,
            (),
        >,
        CollectorStreamIterator<
            Result<Bytes, Arc<dyn Error + Send + Sync>>,
            (),
        >,
    ),
    HttpClientError,
> {
    let (head, body, task) = split_exchange(client, builder);
    valtron::send(task.map_ready(|_| ())).map_err(|e| {
        HttpClientError::Reason(format!("valtron::send failed: {e}"))
    })?;
    Ok((head, body))
}
```

#### I3. `collect_exchange()` — collect full response

Non-streaming convenience: calls `send_and_split()`, drains the head observer
once (checking status), then collects body chunks into a `BytesMut` for
efficient accumulation.

```rust
/// Split, send, and collect a full response.
///
/// For non-streaming endpoints. Drains the head observer (status+headers),
/// then collects all body chunks into `SendSafeBody::Bytes`.
///
/// Uses `BytesMut` internally — more efficient than `Vec<u8>` for byte
/// accumulation (no reallocation on extend).
///
/// # Errors
///
/// Returns the wrapped error if the exchange fails (pre-head or mid-body),
/// or `HttpClientError` if `valtron::send()` fails.
pub fn collect_exchange(
    client: DynNetClient,
    builder: PreparedRequestBuilder,
) -> Result<SimpleResponse<SendSafeBody>, Arc<dyn Error + Send + Sync>> {
    use bytes::BytesMut;

    let (head, mut body) = send_and_split(client, builder)
        .map_err(|e| Arc::new(e) as Arc<dyn Error + Send + Sync>)?;

    // Drain head — one item then the queue closes.
    let (status, headers) = match head.next() {
        Some(Stream::Next(Ok(pair))) => pair,
        Some(Stream::Next(Err(e))) => return Err(e),
        _ => return Err(Arc::new(HttpClientError::Reason(
            "no response head".into(),
        ))),
    };

    // Drain body into BytesMut — no reallocation on each chunk.
    let mut collected = BytesMut::new();
    while let Some(item) = body.next() {
        match item {
            Stream::Next(Ok(chunk)) => collected.extend_from_slice(&chunk),
            Stream::Next(Err(e)) => return Err(e),
            // Non-data signals — the queue is channel-backed, so
            // Wait/Pending/Ignore pass through; we keep draining.
            _ => {}
        }
    }

    Ok(SimpleResponse::new(
        status,
        headers,
        SendSafeBody::Bytes(collected.freeze()),
    ))
}
```

#### I4. Summary: which function to use

| Scenario | Function | Returns |
|----------|----------|---------|
| Caller wants to chain `.map_ready()` on the continuation before sending | `split_exchange()` | `SplitExchange { task, head, body }` — caller sends `task` |
| Simple: just want the observers, continuation auto-sent | `send_and_split()` | `(head, body)` — both `CollectorStreamIterator` |
| Non-streaming: collect full response body | `collect_exchange()` | `SimpleResponse<SendSafeBody>` |
| Streaming: caller drains body chunks | `split_exchange()` or `send_and_split()` | Caller iterates body observer directly |

#### I5. Generated code: non-streaming endpoints

```rust
pub fn get_applications_request<F>(
    client: DynNetClient,
    args: &GetApplicationsArgs,
    builder_mod: Option<F>,
) -> Result<
    impl TaskIterator<
        Ready = Result<ApiResponse<ReturnType>, super::shared::ApiError>,
        Pending = super::shared::ApiPending,
        Spawner = super::shared::BoxedSendExecutionAction,
    > + Send + 'static,
    super::shared::ApiError,
>
where
    F: FnOnce(&mut PreparedRequestBuilder),
{
    let endpoint_url = format!("{}{}", base, path, /* path params */);
    let mut builder = PreparedRequestBuilder::get(&endpoint_url)?
        .query("filter", args.filter.as_deref())
        // ... more query params ...
        ;

    if let Some(body) = &args.body {
        builder = builder.body_json(body)
            .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?;
    }

    if let Some(f) = builder_mod {
        f(&mut builder);
    }

    // collect_exchange() handles split + send + collect — returns a
    // plain Result, not a TaskIterator. Wrap in a single-Ready task
    // for the generated function's return type.
    let result = body_reader::collect_exchange(client, builder);

    Ok(SingleReadyTask::new(result.map(|response| {
        let status = response.status();
        let headers = response.headers().clone();
        let body_bytes = response.body().as_bytes();
        let parsed: ReturnType = serde_json::from_slice(body_bytes)
            .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
        ApiResponse { status: status.into(), headers, body: parsed }
    }).map_err(|e| ApiError::RequestSendFailed(e.to_string()))))
}
```

#### I6. Generated code: streaming endpoints

For streaming endpoints, just return the tuple from `split_exchange` — no wrappers.

```rust
pub fn container_logs_request<F>(
    client: DynNetClient,
    args: &ContainerLogsArgs,
    builder_mod: Option<F>,
) -> (
    CollectorStreamIterator<Result<(Status, SimpleHeaders), Arc<dyn Error + Send + Sync>>, ()>,
    CollectorStreamIterator<Result<Bytes, Arc<dyn Error + Send + Sync>>, ()>,
    Box<dyn TaskIterator<Ready = HttpExchange, Pending = HttpExchangePending,
                         Spawner = BoxedSendExecutionAction> + Send>,
)
where
    F: FnOnce(&mut PreparedRequestBuilder),
{
    let endpoint_url = format!(
        "http://localhost/v1.43/containers/{}/logs",
        args.container_id,
    );
    let mut builder = PreparedRequestBuilder::get(&endpoint_url)?
        .query("stdout", Some(&args.stdout.to_string()))
        .query("stderr", Some(&args.stderr.to_string()))
        .query("follow", Some(&args.follow.to_string()));

    if let Some(f) = builder_mod {
        f(&mut builder);
    }

    body_reader::split_exchange(client, builder)
}
```

Caller usage:
```rust
let (head, body, task) = container_logs_request(&client, &args);

// Caller can chain on the task before sending, or just send it.
task.map_ready(|_| ());   // optional mapping
valtron::send(task)?;

// Drain head — one item.
if let Some(Stream::Next(Ok((status, headers)))) = head.next() {
    if !status.is_success() { return Err(...); }
}

// Stream body chunks.
while let Some(Stream::Next(Ok(chunk))) = body.next() {
    handle(chunk);
}
```

Non-streaming → `collect_exchange()`. Streaming → `split_exchange()`.

### Part J — `PreparedRequestBuilder` API parity audit

| Method | `ClientRequestBuilder` | `PreparedRequestBuilder` | Status |
|--------|----------------------|------------------------|--------|
| `get(url)` / `post(url)` / etc. | ✓ | ✓ | **OK** |
| `header(key, value)` | ✓ | ✓ | **OK** |
| `headers(headers)` | ✓ | ✓ | **OK** |
| `body_json(value)` | ✓ | ✓ | **OK** |
| `body_text(text)` | ✓ | ✓ | **OK** |
| `body_bytes(bytes)` | ✓ | ✓ | **OK** |
| `body_form(params)` | ✓ | ✓ | **OK** |
| `body(body)` | ✓ | ✓ | **OK** |
| `bearer_token(token)` | ✓ | ✓ | **OK** |
| `basic_auth(u, p)` | ✓ | ✓ | **OK** |
| `api_key(name, key)` | ✓ | ✓ | **OK** |
| `authorization(scheme, creds)` | ✗ | ✓ | **OK** (extra) |
| `x_api_key(key)` | ✗ | ✓ | **OK** (extra) |
| `query(key, value)` | ✗ | ✗ | **MISSING** → Part A1 |
| `query_set(key, value)` | ✗ | ✗ | **MISSING** → Part A1 |
| `query_remove(key)` | ✗ | ✗ | **MISSING** → Part A1 |
| `query_params(pairs)` | ✗ | ✗ | **MISSING** → Part A1 |
| `build()` → `PreparedRequest` | ✓ | ✓ | **OK** |
| `build_send_request()` → `SendRequestTask<R>` | ✓ | ✗ | **REPLACED** → Part A3 (`send()`) |
| `send(client)` → `HttpExchangeClientTask` | ✗ | ✗ | **NEW** → Part A3 |

## Scope

### Crates touched

| Crate | Change |
|-------|--------|
| `foundation_core` | Part A: `PathAndQuery` stores `Query` struct; `Uri` gets structured query methods; add `Query::retain()` |
| `foundation_netio` | Part B: query delegate; Part C: re-export + `send()`; Part D: `HttpClientBuilder::build()` → `DynNetClient`; Part I: `split_exchange()`, `send_and_split()`, `collect_exchange()` (split before send, per `H1Transport` pattern) |
| `foundation_openapi` | Part E1-E5: generator emits new imports, signatures; non-streaming uses `collect_exchange()`, streaming uses `split_exchange()` |
| `foundation_deployment` | Part F1: deprecate `RequestIntro` re-export |
| `foundation_deployment_cloudflare` | Part G1-G2: `CloudflareClient` → `DynNetClient`; `dns_ops.rs` update; Part G3: regenerate |
| `foundation_connectrpc` | Part D fallout: update `H1Transport::client` field from `Arc<dyn HttpClient>` to `DynNetClient` |
| `foundation_deployment_stripe` | Part H: regenerate |
| `foundation_deployment_supabase` | Part H: regenerate |
| `foundation_deployment_neon` | Part H: regenerate |
| `foundation_deployment_planetscale` | Part H: regenerate |
| `foundation_deployment_prisma` | Part H: regenerate |
| `foundation_deployment_flyio` | Part H: regenerate |

### Crates NOT touched

- `foundation_ai` — uses `HttpClient` trait directly. Note: `models/generator.rs`
  uses `RequestIntro` — follow-up migration needed.
- `foundation_codegen` / `foundation_codegentools` — no HTTP usage
- `foundation_deployment_docker` — not yet implemented; uses new surface from the start

## Verification

### Parts A+B+C (Uri + PreparedRequestBuilder)
- `cargo check -p foundation_core` — compiles; `PathAndQuery` stores `Query`
- `.query("page", Some("1"))?.query("limit", Some("10"))?.build()` produces a
  `PreparedRequest` with URL query `?page=1&limit=10`
- `.query_set("key", "new")` replaces existing `key` param
- `.query_remove("filter")` removes all `filter` params
- `PreparedRequestBuilder` is importable as `foundation_netio::PreparedRequestBuilder`
- `builder.send(&client)` returns `HttpExchangeClientTask`

### Part D (HttpClientBuilder → DynNetClient)
- `cargo check -p foundation_netio` — compiles
- `cargo check -p foundation_connectrpc` — compiles (H1Transport updated)
- `HttpClientBuilder::new().build()` returns `DynNetClient`

### Part E (generator)
- `cargo check -p foundation_openapi` — compiles
- Generated code compiles with `cargo check -p foundation_deployment_cloudflare --features cloudflare_apps`
- Generated signatures: no `R` generic, `DynNetClient` param (by value), `&mut PreparedRequestBuilder` in closure

### Part F (foundation_deployment)
- `cargo check -p foundation_deployment` — compiles
- `RequestIntro` is deprecated with a clear migration note

### Part G (cloudflare)
- `cargo check -p foundation_deployment_cloudflare` — compiles
- `CloudflareClient` stores `DynNetClient` constructed via `HttpClientBuilder`

### Part H (other crates)
- All 6 deployment crates compile after regeneration

### Part I (body_reader)
- `split_exchange(client, builder)` splits the unsent task via `split_collect_until_map` + `split_collector_map`, sends the continuation to valtron, returns `SplitExchange { head, body }` as `StreamIterator` values
- `head` yields `Stream::Next(Ok((status, headers)))` once then ends
- `body` yields `Stream::Next(Ok(Bytes))` per chunk (unwrapping `BodyChunk`); `Stream::Next(Err(...))` on `Failed`
- `collect_exchange(client, builder)` wraps this in a `TaskIterator` yielding one `Ready(Ok(SimpleResponse<SendSafeBody>))`
- Follows the `H1Transport` pattern: split before send, observers are `StreamIterator`, only continuation goes to valtron

### No regressions
- `cargo check -p foundation_netio --features multi,ssl` — all tests pass
- `cargo check -p foundation_netio --features wasm-fetch --target wasm32-unknown-unknown` — wasm compiles

## Acceptance criteria

1. **`Uri` owns structured query params** — `PathAndQuery` stores `Query` (not `Option<String>`).
   `uri.query_params()` returns `&Query` with `append`, `get`, `get_all`, `iter`.
   `Query::retain()` exists for filtering.

2. **`PreparedRequestBuilder` delegates query to `Uri`** — `.query()`, `.query_set()`,
   `.query_remove()`, `.query_params()` all delegate to the internal `Uri`'s `Query`.

3. **`PreparedRequestBuilder` has `.send(client)`** — calls `client.open_exchange()`,
   returns `HttpExchangeClientTask`. Replaces the old `build_send_request()`.

4. **`PreparedRequestBuilder` is re-exported** — `use foundation_netio::PreparedRequestBuilder` works.

5. **`HttpClientBuilder::build()` returns `DynNetClient`** — `DynNetClient` is the
   canonical client type everywhere. `H1Transport` updated accordingly.

6. **Generator emits `DynNetClient` + `PreparedRequestBuilder`** — no `R` generic,
   no `ClientRequestBuilder`, no `RequestIntro`. Uses `.send()` + `collect_exchange()`.

7. **All deployment crates compile** after regeneration.

8. **`CloudflareClient` uses `DynNetClient`** — no more `SimpleHttpClient::from_system()`.

9. **`RequestIntro` is deprecated** in `foundation_deployment`'s public API.
   Generated code no longer references it.

10. **`split_exchange()`, `send_and_split()`, and `collect_exchange()` body reader
    combinators exist** — `split_exchange()` splits the unsent `HttpExchangeClientTask`
    into a continuation + two observers (caller sends the continuation). `send_and_split()`
    is the convenience wrapper that also calls `valtron::send()`. `collect_exchange()`
    collects the full response into `SimpleResponse<SendSafeBody>` using `BytesMut`.
    Non-streaming endpoints use `collect_exchange()`; streaming endpoints use
    `split_exchange()`.

11. **Existing netio tests pass** — no regression in the HTTP client layer.

## Dependencies

- None — F51 already shipped the new types. This feature aligns everything with them.

## Risks

- **`PathAndQuery` field type change**: `query: Option<String>` → `query: Query`.
  Callers that construct `PathAndQuery` directly need updating. `Uri::query()` return
  type changes from `Option<&str>` to `Option<String>` (allocates). Audit callers.

- **Large regeneration surface**: ~6 deployment crates need regeneration. ~100K+ lines
  of generated code change. Mitigation: regenerate cloudflare first, verify, then
  batch the rest.

- **`HttpClientBuilder::build()` return type change**: from `Arc<dyn HttpClient>` to
  `DynNetClient`. This is a breaking change for callers that explicitly type-annotate.
  Mitigation: audit all callers (`H1Transport`, tests, etc.) and update them.

- **`RequestIntro` cannot be removed**: `foundation_ai/src/models/generator.rs`
  still uses it. Deprecate the re-export; follow up with a separate migration.

- **Streaming vs. non-streaming distinction**: the generator must preserve the
  distinction between endpoints that return a single JSON body (collect it) vs.
  endpoints that stream (pass through `HttpExchangeClientTask`).
