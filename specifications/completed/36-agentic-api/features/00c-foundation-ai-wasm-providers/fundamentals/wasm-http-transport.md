# HTTP Transport: Native vs Wasm

## Native: TCP sockets + TLS (SimpleHttpClient)

On native targets, `foundation_netio::simple_http::SimpleHttpClient` drives HTTP over raw
TCP sockets with `rustls` TLS. It owns a socket, reads/writes framed HTTP/1.1, and supports
both one-shot request/response and streaming (SSE via `ReconnectingEventSourceTask`).

Key properties:
- Blocking or async-via-valtron (spawned onto a thread pool).
- `Send + Sync` — safe across threads.
- Links `ring`, `rustls`, `webpki-roots` — none of which build on `wasm32-unknown-unknown`.

This is why `SimpleHttpClient` is **native-only** — its entire I/O stack assumes OS sockets.

## Wasm: Browser fetch API (FetchHttpClient, F00f)

On `wasm32-unknown-unknown`, networking goes through the browser's `fetch()` API (or
Cloudflare Workers' compatible `fetch()`). `foundation_netio` provides `FetchHttpClient`
(Feature F00f) behind the same `HttpClient` trait that `SimpleHttpClient` implements.

```
   Native:  Provider -> HttpClient trait -> SimpleHttpClient -> TCP/TLS
   Wasm:    Provider -> HttpClient trait -> FetchHttpClient  -> web_sys::fetch()
```

The `HttpClient` trait (`foundation_netio::http_client`) is the abstraction boundary:
- `send()` — one-shot request/response.
- `send_streaming()` — returns a `BoxedSseIterator` for SSE.

Provider code (OpenAI, Anthropic, etc.) calls `HttpClient` methods without knowing the
transport. On native, `default_http_client()` returns a `SimpleHttpClient`. On wasm,
the caller must inject a `FetchHttpClient` (there is no wasm default — the provider
builder's `with_http_client()` is the entry point).

## SSE Streaming on Wasm

Server-Sent Events on native use `ReconnectingEventSourceTask` (TCP keep-alive, chunked
transfer). On wasm, SSE is delivered via `ReadableStream` from the fetch `Response` body.
`FetchHttpClient` adapts the `ReadableStream` into the same `BoxedSseIterator` that native
SSE produces — the provider's streaming code is identical.

## Why foundation_auth Needs the `wasm` Feature

`foundation_auth` uses `uuid`, `chrono`, and `getrandom` — all of which need JS interop on
wasm (`uuid/js`, `chrono/wasmbind`, `getrandom/js`). The `wasm` feature enables these.
Without it, `uuid::Uuid::new_v4()` panics at runtime (no entropy source).

`foundation_ai` always imports `AuthCredential` from `foundation_auth`, so the dep must
build on wasm. The Cargo.toml splits it:

```toml
[dependencies]
foundation_auth = { ..., default-features = false }  # base: no turso

[target.'cfg(not(target_family = "wasm"))'.dependencies]
foundation_auth = { ..., features = ["turso"] }       # native: SQLite

[target.'cfg(target_family = "wasm")'.dependencies]
foundation_auth = { ..., features = ["wasm"] }         # wasm: JS interop
```

The `shared/` module's network functions (`fetch_discovery`, `fetch_jwks`, etc.) use the
`HttpClient` trait via `default_http_client()` — one implementation, no cfg. On native
this resolves to `SimpleHttpClient`; on wasm it resolves to `FetchHttpClient`. The
`wasm-bindgen-oauth` feature (which pulled in raw `web_sys::fetch`) is no longer needed
for basic HTTP; foundation_auth's Cargo.toml target-gates `foundation_netio/multi` on
native and `foundation_netio/wasm-fetch` on wasm to provide `default_http_client()`.
