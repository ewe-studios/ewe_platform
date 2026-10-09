# 26 — HTTP/2 proxy integration

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_proxy` supports HTTP/2 on the front-end and back-end using
`foundation_netio`'s HTTP/2 implementation. This is a Stage 4 feature — the
proxy already works over HTTP/1.1; HTTP/2 is a transport upgrade, not a protocol
change.

## Why

kamal-proxy supports HTTP/2 over TLS on port 443. Our proxy currently forces
`Connection: close` on every response (stage 1 limitation). HTTP/2 enables:

1. **Multiplexing** — multiple requests over one TCP connection, no head-of-line blocking
2. **Server push** — for static assets behind the proxy
3. **gRPC** — requires HTTP/2 as transport
4. **Reduced latency** — binary framing is more efficient than text HTTP/1.1

`foundation_netio` recently landed HTTP/2 support. The proxy should consume it.

## Design

### Front-end (client ↔ proxy)

The `ProxyServer` currently uses `foundation_http::HttpServer`. When the
framework gains HTTP/2 listener support (or the proxy switches to an `h2`
listener directly), the handler (`ProxyHandler`) remains unchanged — it receives
`SimpleIncomingRequest` regardless of transport.

```rust
// In ProxyServer::start(), after TLS is up:
let server = if config.http2_enabled {
    HttpServer::with_config(app, &bind_addr, ServerConfig {
        protocols: Protocols::H1 | Protocols::H2,
        ..ServerConfig::defaults()
    })
} else {
    HttpServer::with_config(app, &bind_addr, ServerConfig::defaults())
};
```

### Back-end (proxy → upstream)

The proxy forwards to backends via `SimpleHttpClient`. When the backend
advertises HTTP/2 (via ALPN or prior knowledge), the client should use it.
This is transparent to the proxy's forwarding logic — `forward_http()` sends a
request and gets a response; the transport is the client's concern.

```rust
// ClientConfig gains:
pub struct ClientConfig {
    // ... existing fields ...
    /// Prefer HTTP/2 when the backend supports it (ALPN negotiation).
    pub http2_preferred: bool,
}
```

### Constraints

| Item | Stage 1 (current) | Stage 4 (with HTTP/2) |
|------|------------------|----------------------|
| Front-end protocol | HTTP/1.1 only | HTTP/1.1 + HTTP/2 (ALPN) |
| Connection header | `Connection: close` | Keep-alive, multiplexed streams |
| Backend protocol | HTTP/1.1 only | HTTP/1.1 + HTTP/2 |
| WebSocket upgrade | Yes (HTTP/1.1 Upgrade) | Not needed (HTTP/2 has native streams) |
| gRPC | No | Yes (requires HTTP/2) |

## Integration with existing code

- `handler.rs`: no change — `Serve::serve()` receives `SimpleIncomingRequest`
- `forward.rs`: no change — `forward_http()` uses `SimpleHttpClient`
- `server.rs`: add `http2_enabled` flag to `ProxyConfig`, pass to `HttpServer`

## Verification

1. Proxy with `http2_enabled: true`, curl `--http2 https://app.local` → HTTP/2 response.
2. Backend advertising h2 via ALPN → proxy connects via HTTP/2.
3. Backend that only speaks HTTP/1.1 → proxy falls back, no error.
4. gRPC client → proxy → gRPC backend: unary + streaming calls work.
