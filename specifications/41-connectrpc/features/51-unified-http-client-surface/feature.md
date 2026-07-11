---
feature: "Unified network client — dissolve SimpleHttpClient, one client for HTTP + WebSocket across native + wasm (D07 §Client architecture)"
description: "One HttpClient trait (async + a valtron task surface via HttpExchangeClientTask); fold SimpleHttpClient into NativeHttpClient; relocate the client out of simple_http into a netio-level module; a uniform cross-platform builder; FetchHttpClient honours ClientConfig; then (later stage) fold WebSocketClient into the same concrete client behind a segregated WebSocketConnector trait. Delivered in gated stages."
status: "in-progress"
priority: "high"
phase: 1
depends_on: ["07-pushable-request-body", "44-connection-owner-walking-skeleton"]
estimated_effort: "x-large"
created: 2026-07-11
---
# Feature 51: Unified network client (HTTP + WebSocket)

> **Status: proposed, for review.** A design proposal, not a plan of record.
> Nothing is implemented. It is delivered in **gated stages** (see *Staged
> rollout*): Stages 1–4 unify the HTTP client and relocate it; Stages 5–6 fold
> the WebSocket client into the same concrete client. Later stages can land
> independently once the earlier ones ship.

## Why this exists

`foundation_netio` is meant to serve **both** native and wasm. Its HTTP *client*,
however, is split three ways and the native-only half leaks everywhere:

```
HttpClient trait  (shared: send / send_async / send_sse / send_sse_async)
├── NativeHttpClient<R>   ── wraps ──▶ SimpleHttpClient<R>   ← native only
│                                        pool, ClientConfig, MiddlewareChain (dead),
│                                        TLS connector, verb builders, timeouts,
│                                        proxy, redirects, retries
└── FetchHttpClient       (unit struct — no config, no resolver, no builder)

valtron task layer (separate, inconsistent):
  HttpExchangeTask<R>      native — Spawner = BoxedSendExecutionAction
  WasmHttpExchangeTask     wasm   — Spawner = NoSpawner   ← violates house law
```

Three concrete problems:

1. **`SimpleHttpClient` is native-only but pervades the codebase.** It is named
   directly in `foundation_connectrpc` (the `H1Transport` transport), `foundation_ai`,
   `foundation_auth`, `foundation_db`, `foundation_proxy`, every deployment client
   (`gcp`, `stripe`, `planetscale`, `cloudflare`, `flyio`, …), and more. A crate
   that is supposed to compile to wasm cannot, the moment it touches the client.

2. **The task surface leaks native-only types.** `H1Transport::open()`
   (`transport/h1.rs`) wants a valtron task it can `split` and `valtron::send()`.
   To get one it reaches into `SimpleHttpClient` for `client_pool()` +
   `client_config()` and builds `HttpExchangeTask<R>` by hand — re-baking the
   native-only `HttpConnectionPool` into connectrpc. The task layer that
   `HttpExchangeTask` was meant to make easy is instead ad-hoc and native-pinned.

3. **The two task impls disagree.** `WasmHttpExchangeTask` uses
   `type Spawner = NoSpawner`, which is against the house law (**always**
   `BoxedSendExecutionAction`, never `NoAction`/`NoSpawner`). So the native and
   wasm tasks cannot be unified behind one type today.

`FetchHttpClient`, meanwhile, ignores `ClientConfig` entirely — timeouts, redirect
policy, and headers a caller sets are silently dropped on wasm.

4. **A *second* connection-owning client duplicates all of this.**
   `WebSocketClient<R: DnsResolver + Clone + Send + 'static>`
   (`websocket/native/connection.rs`) is a parallel `R`-generic native client with
   its own `connect()`/`connect_with_reconnect()`/`connect_parts()`, carrying the
   same `DnsResolver + Clone` baggage. But a WebSocket handshake **is** an HTTP/1.1
   `Upgrade` (`websocket/shared/handshake.rs`) — it dials, does TLS, and could reuse
   the very pool the HTTP client owns. Two clients own "establish a connection"; they
   should be one.

Finally, the client lives under `simple_http/client/` — an HTTP-specific home. Once
it also opens WebSockets it is no longer HTTP-specific, so it is **relocated** to a
netio-level module.

## The target shape

One client surface, platform-symmetric, with native-only machinery gated — not
forked:

```
HttpClient trait  (shared)
  async:  send_async / send_sse_async / send / send_sse           (unchanged)
  task:   open_exchange(req) -> HttpExchangeClientTask            ← NEW, shared
          open_exchange_stream(req) -> HttpExchangeClientTask     (streaming body)

  impl for NativeHttpClient<R = BoxedDnsResolver>   (owns pool + config)
  impl for FetchHttpClient                          (owns config; wasm)

HttpExchangeClientTask   (shared type; one Spawner = BoxedSendExecutionAction)
  wraps the native HttpExchangeTask<R> OR the wasm fetch task, erased to a single
  TaskIterator<Ready = HttpExchange, Pending = HttpExchangePending,
               Spawner = BoxedSendExecutionAction>

HttpClientBuilder        (shared, generic; produces an HttpClient)
  cross-platform:  base_url, headers, timeouts, max_redirects, retries,
                   max_body_size, verb helpers (get/post/…)
  #[cfg(native)]:  connection pool, TLS connector, DnsResolver<R>, proxy

WebSocketConnector trait (shared; Stages 5–6)          ← separate trait, shared types
  open_websocket(req) -> WebSocketClient              (shared type, cross-platform)
  impl for NativeHttpClient (HTTP Upgrade over pool) + FetchHttpClient (browser WS)

  WebSocketClient   (shared, no R generic)            ←  wraps
    Box<dyn StreamIterator<Item=Stream<WebSocketMessage, WebSocketProgress>> + Send>
    + MessageDelivery                                  (PipeSender, always cross-platform)
  
  NativeHttpClient::open_websocket:
    pool.create_http_connection() → WebSocketTask or ReconnectingWebSocketTask
    → WsTask(Single|Reconnecting) → execute() → Box → WebSocketClient::new(boxed_stream, delivery)
  
  FetchHttpClient::open_websocket:
    browser WebSocket → WasmWsTask → execute() → Box → WebSocketClient::new(boxed_stream, delivery)

  all of the above live in  top-level http / src::shared / src::wasm   (flattened out of simple_http/client)
```

### 1. `HttpClient` gains a task surface

The trait grows a method that returns a **valtron task**, so any caller that wants
the task-level power (`split`, `valtron::send()`, `Depends`-parking) gets it from
the trait — no reaching into a concrete native client:

```rust
pub trait HttpClient: Send + Sync {
    // … existing async + sync methods unchanged …

    /// Build a valtron task that drives one request/response exchange, yielding
    /// `HttpExchange` items (Head, BodyChunk, Failed). The caller spawns it with
    /// `valtron::send()` or composes it with split combinators.
    fn open_exchange(&self, req: PreparedRequest) -> HttpExchangeClientTask;
}
```

`H1Transport::open()` then becomes platform-agnostic and pool-free:

```rust
// before: reaches into SimpleHttpClient::client_pool()/client_config()
let pump = HttpExchangeTask::new(prepared, max_redirects, pool, config);
// after: the client owns all of that; H1Transport just asks for the task
let pump = self.client.open_exchange(prepared);
```

`H1Transport` holds `Arc<dyn HttpClient>` (or a generic `C: HttpClient`) instead of
`Arc<SimpleHttpClient>`, and the native-only `HttpConnectionPool` never appears in
`foundation_connectrpc` again.

### 2. `HttpExchangeClientTask` — one task type, `BoxedSendExecutionAction`

A single shared type both platforms produce, so the trait method has one return
type. Internally it is an enum (or an erased `Box<dyn TaskIterator<…>>`) over the
native and wasm tasks; its `TaskIterator` impl fixes:

```rust
type Ready   = HttpExchange;               // already shared
type Pending = HttpExchangePending;        // already shared
type Spawner = BoxedSendExecutionAction;   // house law — never NoSpawner
```

**This requires migrating `WasmHttpExchangeTask` off `NoSpawner`** to
`BoxedSendExecutionAction` (it simply never returns `TaskStatus::Spawn`). That
removes the last obstacle to unification and satisfies the law. `NoSpawner`/
`NoAction` usage in this area is deleted.

### 3. Dissolve `SimpleHttpClient` into `NativeHttpClient`

`NativeHttpClient<R>` already wraps `SimpleHttpClient<R>`. Invert that: fold the
pool, `ClientConfig`, TLS connector, verb builders, timeouts, proxy, redirect and
retry logic **into** `NativeHttpClient<R>`, which becomes the one native client and
the sole owner of `HttpConnectionPool<R>`. The dead `MiddlewareChain` is **dropped,
not folded** (see blast radius). `SimpleHttpClient` ceases to exist as a type.

Because it is named in hundreds of (mostly generated) sites, keep a **deprecated
type alias** during migration:

```rust
#[deprecated(note = "use NativeHttpClient; SimpleHttpClient was folded into it (F51)")]
pub type SimpleHttpClient<R = SystemDnsResolver> = NativeHttpClient<R>;
```

so the tree keeps compiling while call sites and codegen templates are migrated,
then the alias is removed.

### 4. A crate-root, generic `HttpClientBuilder` with a *uniform* surface

The builder ergonomics (`.get()/.post()/.config()/.max_redirects()/…`) are good and
should stay — but they should **not** live on a native-only struct, and they should
present the **same surface on both targets**, so a caller writes identical code and
never `#[cfg]`s their own call sites.

Two design keys make that possible:

**Key A — the resolver is erased to `BoxedDnsResolver` (§6), defaulted, never
threaded by the caller.** The builder synthesises its own internals from the
target-aliased `DefaultResolver` (`Arc::new(DefaultResolver::default())`) unless the
caller supplies one via `.resolver(r)`. That is what lets the same fluent code build
a client on both targets: on native the builder internally constructs a
`HttpConnectionPool<BoxedDnsResolver>`; on wasm that construction is simply dead code
behind a `cfg`, with the *same* method surface that does nothing. Only
`DefaultResolver` needs `Default` — not every resolver (see §6).

**Key B — the builder lives at the crate root, not `shared/`.** `shared/` is
strictly cross-platform-only (house rule: never put platform-specific things there),
and this builder must name native types (`HttpConnectionPool`, `SSLConnector`) inside
`cfg`-gated bodies. So it lives in a top-level `foundation_netio` module compiled on
**both** targets, with platform-specific *implementations* gated internally — the
*signatures* stay identical:

```rust
// foundation_netio::http_client (crate-root module, both targets)
pub struct HttpClientBuilder<R: DnsResolver + Default = DefaultResolver> { /* ClientConfig + R */ }

impl<R: DnsResolver + Default> HttpClientBuilder<R> {
    // Identical surface on both targets — no caller-visible cfg.
    pub fn max_redirects(self, n: u8) -> Self;
    pub fn connect_timeout(self, d: Duration) -> Self;
    pub fn header(self, k: &str, v: &str) -> Self;

    /// Configure the connection pool / TLS. On native this wires a real
    /// `HttpConnectionPool<R>` + `SSLConnector`; on wasm the browser owns
    /// connection reuse and TLS, so the body is inert dead code — but the call
    /// compiles, so the same code works on both sides.
    pub fn pool(self, opts: PoolOptions) -> Self;
    pub fn tls(self, opts: TlsOptions) -> Self;

    pub fn build(self) -> impl HttpClient;   // NativeHttpClient<R> | FetchHttpClient
}
```

Because the pool/TLS surface is uniform (its *body* is cfg'd, not its *signature*),
a caller configures "the pool they want with whatever TLS they want" once, and it is
honoured on native and harmlessly ignored on wasm — "same code, platform-specific
underneath." `PoolOptions`/`TlsOptions` are cross-platform description types (not the
native `HttpConnectionPool`/`SSLConnector` objects themselves), so they name nothing
native at the surface.

### 5. `FetchHttpClient` honours `ClientConfig`

`FetchHttpClient` stops being a stateless unit struct and carries a `ClientConfig`
(default, or supplied at construction). It maps the fields it *can* onto the fetch
API and documents the ones it cannot:

| `ClientConfig` field | wasm mapping |
|---|---|
| `connect_timeout` / `read_timeout` | `AbortController` + `setTimeout` |
| `max_redirects` (0 vs >0) | `RequestInit.redirect = "manual" | "follow"` |
| default headers | merged into `Headers` |
| `max_body_size` | enforced while draining the `ReadableStream` |
| proxy / TLS connector / pool / `DnsResolver` | **N/A** — the browser owns these; documented no-ops |

### 6. `BoxedDnsResolver` — erase the resolver, keep the stack generic

`DnsResolver` lives in `client/shared/dns.rs` (compiled on wasm). Rather than
thread a resolver *type* through every public signature, **erase it to a boxed
trait object at construction**, so the client is effectively non-generic:

- **Make `DnsResolver` object-safe.** Its `resolve(&self, host, port) -> Result<…>`
  method already is; the only blocker is the `Clone` supertrait (`Clone: Sized`).
  Drop it — `trait DnsResolver: Send + Sync` — and let the `Arc` provide cloning.
- **`type BoxedDnsResolver = Arc<dyn DnsResolver>`**, and widen the existing
  `impl<T: DnsResolver> DnsResolver for Arc<T>` to `impl<T: DnsResolver + ?Sized>`
  so `Arc<dyn DnsResolver>` itself is a `DnsResolver`.
- **`build<R: DnsResolver + 'static>(r: R)`** boxes it: `Arc::new(r) as
  Arc<dyn DnsResolver>`. The whole generic stack (`SendRequestTask<R>`,
  `HttpConnectionPool<R>`, `HttpExchangeTask<R>`, `NativeHttpClient<R>`) then
  monomorphises at the single concrete type `R = BoxedDnsResolver` — the resolver
  type parameter never appears in a public signature, so `dyn HttpClient` is
  trivially object-safe (**Open Question 1 → (b)**), and one `dyn` dispatch per DNS
  resolution (once per connection, cached) is the only cost.
- **A target-aliased default** covers the no-arg path without requiring every
  resolver to be `Default` — only the default one:

  ```rust
  #[cfg(not(target_family = "wasm"))] pub type DefaultResolver = SystemDnsResolver;
  #[cfg(target_family = "wasm")]       pub type DefaultResolver = NoopDnsResolver;   // new; browser resolves
  ```

  `NativeHttpClient::default()` / the builder's no-arg path constructs
  `Arc::new(DefaultResolver::default())`. `SystemDnsResolver` is already `Default`;
  `NoopDnsResolver` is added as a `Default` no-op.

This supersedes an earlier "keep `<R = DefaultResolver>` generic on every public
type" sketch: boxing is less surface, fewer bounds (`Default` needed only on the
default resolver, not the trait), and the same uniform builder from §4.

### 7. Flatten `simple_http/client` → a top-level `http` module (Stage 4)

Once the client owns HTTP *and* WebSocket it is no longer HTTP-specific, and it is
buried three levels deep. Apply netio's own Phase 0 shared/native/wasm pattern to
it: **flatten `simple_http/client` into a top-level `http` module, and promote its
`shared`/`wasm` submodules into netio's top-level `src/shared` and `src/wasm`**:

```
simple_http/client/shared/ ──▶ src/shared/     (HttpClient trait, DnsResolver /
                                                 BoxedDnsResolver, ClientConfig,
                                                 request_task, body_reader, the
                                                 cross-platform builder surface)
simple_http/client/wasm/   ──▶ src/wasm/        (FetchHttpClient, wasm exchange task)
simple_http/client/native/ ──▶ src/http/        (NativeHttpClient, pool, native
                                                 exchange task, native builder bits;
                                                 gated native in lib.rs)
```

`simple_http` keeps only the HTTP *wire* types it genuinely owns (request/response
framing, `PreparedRequest`, `SendSafeBody`, the SSE parser). Re-exports stay at the
old paths during the deprecation window. Because F51 already rewrites every client
import path (dissolving `SimpleHttpClient`), the flatten rides the *same* migration.

**Precondition — `src/shared` must first be genuinely shareable.** It is not today:
`src/shared/context.rs` pulls the native `netcap::SocketAddr` (native-only for its
`Unix` variant) into `ConnectionContext.peer_addr` behind a `cfg`. Before promoting
client types into `src/shared`, fix that leak — `peer_addr` uses the cross-platform
`core::net::SocketAddr` (present on wasm), with any Unix-address need handled as a
native extension. (`errors.rs::SocketAddrError(AddrParseError)` is already fine —
`AddrParseError` is cross-platform.) This keeps `src/shared` honest, so promoting
the client's shared types in does not import the leak wholesale.

### 8. WebSocket over the same client — a segregated `WebSocketConnector` trait (Stages 5–6)

The concrete clients become the universal means to open a socket too — but through
a **separate** trait, not by swelling `HttpClient`. A request→response exchange and
a persistent full-duplex channel are different lifecycles; interface segregation
keeps each cohesive and lets HTTP-only consumers depend on `HttpClient` alone:

```rust
pub trait WebSocketConnector: Send + Sync {
    /// Open a WebSocket. Native performs the HTTP/1.1 Upgrade over the client's
    /// own dial/TLS/pool; wasm hands off to the browser `WebSocket` API. Returns
    /// the existing `WebSocketConnection`.
    fn open_websocket(&self, req: WebSocketRequest) -> Result<WebSocketConnection, WebSocketError>;
}

impl WebSocketConnector for NativeHttpClient { /* Upgrade over the pool */ }
impl WebSocketConnector for FetchHttpClient  { /* browser WebSocket */ }
```

The existing WebSocket machinery is **reused, not reinvented**: `open_websocket`
drives `websocket/shared/handshake.rs` over the client's connection and returns the
existing `WebSocketConnection`; the parallel `WebSocketClient<R>` /
`ReconnectingWebSocketTask<R>` are **folded into** the unified client (their
`connect`/`reconnect` become methods/return types on the connector path) and their
own `R: DnsResolver + Clone` generic is dropped — it inherits F51's
`BoxedDnsResolver` for free. wasm asymmetry is on-pattern: native reuses the HTTP
dial/TLS/pool; wasm uses the browser `WebSocket` API (nothing shared with `fetch`
but the method surface).

## What this affects (blast radius)

Two populations, migrated differently:

- **Generated code — migrate by regenerating.** The bulk of `SimpleHttpClient`
  references are in codegen output: `foundation_deployment_gcp` (~300 files),
  `stripe` (~107), `planetscale` (~77), `cloudflare` (~43), `prisma`, `supabase`,
  `flyio`, `neon`, … These change by updating the **codegen templates**
  (`foundation_codegentools` / the deployment generators) to emit `NativeHttpClient`
  (or the trait), then regenerating. The deprecated alias keeps them building in
  the interim.
- **Hand-written core — migrate by hand.** The real edits:
  - `foundation_netio` — the client refactor itself (fold `SimpleHttpClient` →
    `NativeHttpClient`; `HttpClientBuilder`; `open_exchange` on the trait;
    `HttpExchangeClientTask`; `FetchHttpClient` config; `NoopDnsResolver`; migrate
    `WasmHttpExchangeTask` off `NoSpawner`); **flatten** `simple_http/client` into a
    top-level `http` module (`shared`→`src/shared`, `wasm`→`src/wasm`) after
    de-leaking `src/shared`; **fold `WebSocketClient<R>` /
    `ReconnectingWebSocketTask<R>`** into the unified client behind
    `WebSocketConnector`, reusing `websocket/shared/handshake.rs` and
    `WebSocketConnection`.
  - `foundation_connectrpc` — `H1Transport` holds `Arc<dyn HttpClient>` and calls
    `open_exchange`; drop the `SimpleHttpClient`/pool import (`transport/h1.rs`,
    examples, `server_socket_tests.rs`, `conformance_tests.rs`).
  - `foundation_ai` (`models/generator.rs`), `foundation_auth` (`oauth.rs`,
    `password_auth.rs`), `foundation_db` (`d1_kvstore.rs`, `r2_blobstore.rs`),
    `foundation_proxy`, the deployment **core** clients (`*/src/client.rs`,
    `flyio/src/clients`), `foundation_openapi`, `foundation_testing`,
    `foundation_testbed`.
- **MiddlewareChain — deleted as dead code.** Audited: **no caller populates it.**
  Every `.middleware(...)` in the tree is the unrelated *server-side*
  `app.middleware(...)` on `HttpApp`; the client chain is only ever
  `MiddlewareChain::new()` (the empty default) constructed inside `foundation_netio`
  itself and never fed. So the whole client middleware apparatus
  (`client/shared/middleware.rs`, the `middleware_chain` fields, the `.middleware()`
  builder method, and the process-request/response plumbing) is removed outright —
  not migrated. If a request-interception hook is ever wanted, it returns as a
  cross-platform concept on the builder, designed for a real use case.

## Staged rollout (one feature, gated stages, tree stays green)

Stages 1–4 are the HTTP unification + relocation; Stages 5–6 fold WebSocket in.
Each stage compiles and keeps F44 green, so later stages can land independently.

- **Stage 1 — additive core.** In `foundation_netio`: make `DnsResolver` object-safe
  (drop the `Clone` supertrait — update the `R: DnsResolver + Clone` bounds across the
  client stack to drop `+ Clone`, cloning now via `Arc`), add `BoxedDnsResolver` +
  the widened `Arc<T: ?Sized>` impl + `NoopDnsResolver`/`DefaultResolver`; migrate
  `WasmHttpExchangeTask` to `BoxedSendExecutionAction`; add `HttpExchangeClientTask`
  and `HttpClient::open_exchange`. Delete the dead client `MiddlewareChain`. No call
  site changes yet.
- **Stage 2 — fold + builder.** Fold `SimpleHttpClient` into `NativeHttpClient`
  (public surface erased at `R = BoxedDnsResolver`); add the deprecated
  `SimpleHttpClient` alias; lift the builder into the crate-root `HttpClientBuilder`
  with the uniform (cfg-bodied, not cfg-signatured) pool/TLS surface; give
  `FetchHttpClient` its `ClientConfig`.
- **Stage 3 — de-leak connectrpc.** `H1Transport` uses `Arc<dyn HttpClient>` +
  `open_exchange`; remove the pool/`SimpleHttpClient` import. The headline win (a
  wasm-capable transport client) and the F44/F23 beneficiary.
- **Stage 4 — flatten + relocate + migrate sites.** First de-leak `src/shared`
  (fix `ConnectionContext.peer_addr` to `core::net::SocketAddr`). Flatten
  `simple_http/client` → top-level `http`, promoting `shared`→`src/shared` and
  `wasm`→`src/wasm` (§7); re-exports at old paths. Migrate the hand-written sites
  (ai/auth/db/proxy/openapi/testing + deployment core clients) off the alias; update
  the codegen templates and regenerate gcp/stripe/planetscale/cloudflare/…; then
  remove the `SimpleHttpClient` alias. `grep` proves zero non-alias references and no
  `NoSpawner`.
- **Stage 5 — `WebSocketConnector` trait.** Add the segregated trait (§8); implement
  it for `NativeHttpClient` (HTTP/1.1 `Upgrade` over the pool/TLS, driving
  `websocket/shared/handshake.rs`, returning `WebSocketConnection`) and for
  `FetchHttpClient` (browser `WebSocket`). Additive — `WebSocketClient<R>` still
  exists.

  **Implementation plan (2026-07-11):**

  The blocker: `WebSocketClient` is native-only (parameterised over
  `R: DnsResolver`), and `WebSocketConnection` wraps `SharedByteBufferStream<RawStream>`
  which doesn't exist on wasm. Two work items unlock cross-platform:

  **A. Make `WebSocketClient` cross-platform by decoupling it from `R`:**
  - Move `WebSocketClient` + `MessageDelivery` out of `native/connection.rs` into
    `websocket/client.rs` (beside `native/` — shared).
  - Change `WebSocketClient` from `WebSocketClient<R: DnsResolver>` to
    `WebSocketClient` (no generic) — it stores a `Box<dyn StreamIterator<Item = Stream<WebSocketMessage, WebSocketProgress>> + Send>`
    plus a `MessageDelivery`.
  - `WebSocketClient::connect()` becomes `WebSocketClient::new(stream, delivery)` —
    the caller provides an already-spawned stream + delivery handle.
  - Platform-specific `connect` helpers stay in their respective modules:
    `native::connect(resolver, url, …)` returns `(WebSocketClient, MessageDelivery)`
    by creating a `WebSocketTask`/`ReconnectingWebSocketTask`, wrapping in `WsTask`
    enum, spawning, and boxing.
    `wasm::connect(url, …)` does the same with a wasm `WebSocket` task.
  - `WebSocketClient<R>` stays as a deprecated type alias during migration.

  **B. `WebSocketConnector` trait returns `WebSocketClient`, not `WebSocketConnection`:**
  - Change `open_websocket` return type to `Result<WebSocketClient, WebSocketError>`.
  - Native impl: reuses the existing `WsTask` enum (Single | Reconnecting) —
    creates `WebSocketTask` (or `ReconnectingWebSocketTask`) via the client's pool,
    wraps in `WsTask`, `execute()`s, boxes the `DrivenStreamIterator`, wraps in
    `WebSocketClient`.
  - Wasm impl: `FetchHttpClient` creates a wasm `WebSocket` task (browser `WebSocket`
    API bridged through `web_sys`), spawns, wraps in `WebSocketClient`.
  - Both return the same shared `WebSocketClient` type — zero platform leakage.

- **Stage 6 — fold + retire `WebSocketClient<R>`.** Route `WebSocketClient<R>` /
  `ReconnectingWebSocketTask<R>` through the unified client, drop their `R: DnsResolver
  + Clone` generic (inherit `BoxedDnsResolver`), migrate WS call sites to
  `open_websocket`, and remove the standalone `WebSocketClient` (alias during the
  window). `grep` proves one connection-owning client remains.

## Implementation status — Stages 5–6 landed (2026-07-11)

The cross-platform WebSocket client is implemented and unifies native + wasm:

- **`WebSocketClient` is now non-generic and shared** (`websocket/shared/client.rs`).
  It stores `Box<dyn Iterator<Item = WebSocketStreamItem> + Send>` + `MessageDelivery`,
  where `WebSocketStreamItem = Stream<Result<WebSocketMessage, WebSocketError>, WsProgress>`.
  The native `connect`/`with_options`/`connect_with_reconnect`/`with_pool*`/
  `connect_parts` helpers are generic over `R` but return the non-generic client
  (resolver erased at construction), so existing call sites compile unchanged.
- **Shared `WsProgress`** (Connecting/Handshaking/Reading/Reconnecting) replaces the
  earlier `()` pending erasure — both platforms map their progress into it, so the
  connecting/handshaking/reconnecting signal survives to consumers. Native maps
  `WsPending`; wasm maps the browser socket's readiness.
- **Shared `WebSocketConnectConfig`** (subprotocols, `extra_headers: SimpleHeaders`,
  `reconnect`, timeouts) — one config both platforms accept. `Reconnect` moved to the
  shared client (native honours `Yes`; wasm documents it best-effort). `open_websocket`
  now takes this config and returns `WebSocketClient` (was `WebSocketConnection`).
- **`extra_headers` are now actually sent.** Pre-existing bug: the native task threaded
  `extra_headers` through its state structs but `build_upgrade_request` never received
  them — they were silently dropped. Fixed: `extra_headers` is `SimpleHeaders`
  end-to-end (in `WebSocketTask` and `ReconnectingWebSocketTask`), applied to the
  handshake request multi-value-aware. Covered by a new handshake test.
- **Wasm browser bridge** (`websocket/wasm/browser.rs`): a `web_sys::WebSocket` wired
  into the shared client — JS `open`/`message`/`error`/`close` callbacks feed a pipe,
  outbound `MessageDelivery` drains to `WebSocket.send`. `FetchHttpClient` implements
  `WebSocketConnector` (`websocket/wasm/http_client_connector.rs`). `Send` is asserted
  only on single-threaded wasm (mirroring `SendWrapper`).
- **`open_websocket` is the single auto-switching surface**: build a `NativeHttpClient`
  or `FetchHttpClient` via the uniform builder, call the identical `open_websocket` →
  the right platform impl runs, returning the same `WebSocketClient`.

Verification: native — 175 websocket + F51 tests pass (`--profile uat`). Wasm —
`cargo check --target wasm32-unknown-unknown --no-default-features --features wasm-fetch`
compiles clean (0 errors). The browser socket's *runtime* behaviour is not exercised
here (the embedded V8 testbed has no `WebSocket` global; that needs the Playwright
browser runner + a WS server).

**Incidental pre-existing wasm-client fixes (were blocking any wasm build):** the
`src/shared/context.rs` `PeerIdentity` leak (cfg-gated out on wasm yet used
unconditionally — a Stage-4 de-leak miss), `FetchHttpClient` constructed as a unit
struct in the wasm exchange task, the stale `RequestInit::set_redirect` web-sys API
(now set via `Reflect`), and a `&self`-borrow escaping `run_future` in
`FetchHttpClient::send` (extracted to an owned `fetch_and_read` free fn).

**Remaining (Stage 6 tail, deferred):** the native `connect*` helpers still take
`R: DnsResolver + Clone` (erased at construction) rather than being dropped entirely;
no deployment-crate WS call sites needed migration (grep found none outside netio).

## Open questions for review

1. **Object safety vs the resolver generic — RESOLVED to (b), via `BoxedDnsResolver`
   (§6).** `open_exchange` on `dyn HttpClient` cannot be generic over `R`, so the
   resolver is chosen when the client value is **constructed** (the `build()`/`new()`
   call at runtime — *not* compile time) and erased into `Arc<dyn DnsResolver>`; the
   public client monomorphises at `R = BoxedDnsResolver`. Every request that client
   makes uses that resolver; the trait methods take only `req`, so `dyn HttpClient`
   stays object-safe. Most callers pass nothing and get `DefaultResolver`. The
   rejected alternative (a) — erase `R` *per request* inside `HttpExchangeClientTask`
   — buys per-request resolvers nobody needs. Two clients with two resolvers in one
   program is still fine; only *per-request* swapping is disallowed.
2. **`HttpExchangeClientTask`: enum or boxed?** An enum over native/wasm tasks keeps
   the concrete `TaskIterator` (composes with split combinators without a `dyn`
   indirection); a `Box<dyn TaskIterator<…>>` is simpler but must still satisfy what
   `split_collect_until_map` needs. Which composes with `transport/h1.rs`?
3. **Builder return type.** `-> impl HttpClient` vs a concrete `NativeHttpClient` /
   `FetchHttpClient` per target. `impl Trait` keeps callers symmetric but hides the
   concrete verb helpers unless they are also on the trait.
4. **Streaming upload on the task surface.** `open_exchange_stream` needs the
   incremental `WriteBody` still deferred to F23 — does F51 depend on F23, or ship
   the non-streaming `open_exchange` first?

## Acceptance criteria

- `foundation_connectrpc` compiles for `wasm32-unknown-unknown` with `H1Transport`
  (or its wasm analogue) — no `SimpleHttpClient`, no `HttpConnectionPool` reference.
- One `HttpClient` trait carries both the async surface and `open_exchange`;
  `NativeHttpClient` and `FetchHttpClient` both implement it.
- `HttpExchangeClientTask` has `type Spawner = BoxedSendExecutionAction`; no
  `NoSpawner`/`NoAction` remains in the client or task layer.
- `SimpleHttpClient` exists only as a deprecated alias, then is removed; `grep -r`
  finds zero non-alias references.
- `FetchHttpClient` observes `ClientConfig` (timeouts, redirect policy, headers,
  max body) and documents the fields it cannot honour.
- The shared `HttpClientBuilder` builds a client on both targets; native-only knobs
  have identical signatures with cfg'd bodies, not a separate type.
- The client is flattened into a top-level `http` module with its `shared`/`wasm`
  promoted into `src/shared`/`src/wasm`; `src/shared` is leak-free (no native
  `SocketAddr` in `ConnectionContext`); `simple_http/` no longer owns the client
  (re-exports at old paths removed after the deprecation window).
- **WebSocket (Stages 5–6):** a segregated `WebSocketConnector` trait is implemented
  by `NativeHttpClient` (HTTP `Upgrade` over the client's own pool/TLS) and
  `FetchHttpClient` (browser `WebSocket`), reusing `WebSocketConnection` and the
  existing handshake. The standalone `WebSocketClient<R>` /
  `ReconnectingWebSocketTask<R>` are folded in and removed; `grep` proves one
  connection-owning client remains, with no `DnsResolver + Clone` bound anywhere.
- F44 `server_socket_tests` and the existing WebSocket tests still pass; the
  deployment crates build after regeneration.

## Out of scope

- Incremental streaming request upload (`WriteBody`) — stays F23; `open_exchange`
  ships non-streaming first if F51 does not take an F23 dependency.
- HTTP/2+/3 client transports (F29+).
- WebSocket *protocol* changes — Stages 5–6 only change *who opens* the socket
  (the unified client) and reuse the existing frame/handshake/`WebSocketConnection`
  machinery unchanged.
- Changing the wire behaviour of any existing request path — this is a surface and
  ownership refactor, byte-for-byte identical on the wire.
