# 27 — HTTP/3 (QUIC) proxy integration

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_proxy` supports HTTP/3 (QUIC) on the front-end using
`foundation_netio`'s QUIC driver (spec-41 F33). This is a Stage 4+ feature —
QUIC is a fundamentally different transport (UDP-based, connection migration,
0-RTT) and requires more integration work than HTTP/2.

## Why

kamal-proxy has opt-in HTTP/3 via `--http3`. QUIC provides:

1. **0-RTT handshakes** — instant connection resumption
2. **No head-of-line blocking** — independent streams, unlike TCP
3. **Connection migration** — survive IP changes (mobile, roaming)
4. **Built-in TLS 1.3** — no separate TLS layer

`foundation_netio` already has a QUIC driver (`quic/quinn_driver.rs`, F33)
backed by `quinn-proto 0.11`. It runs as a `TaskIterator` on valtron. The
proxy can consume it as a front-end transport.

## Architecture

```
Client (QUIC)
    │
    ▼
QuicListener (UDP socket, quinn/quinn-proto)
    │
    ▼
ProxyHandler::serve()  ← same handler, different transport
    │
    ▼
Backend (HTTP/1.1 or HTTP/2 or QUIC)
```

The proxy's data plane is transport-agnostic:
- `SimpleIncomingRequest` carries the parsed request regardless of transport
- `ProxyHandler` routes, load-balances, and forwards — same code
- Health probes, state, persistence — no change

## Design

### Front-end

```rust
// In ProxyServer::start():
if config.http3_enabled {
    let quic_listener = QuicListener::bind("0.0.0.0:443")?;
    let quic_shutdown = shutdown.clone();
    std::thread::spawn(move || {
        quic_accept_loop(quic_listener, app, &quic_shutdown);
    });
}
```

The QUIC listener:
1. Binds a UDP socket on port 443 (same as HTTPS — ALPN distinguishes)
2. Accepts QUIC connections via `quinn::Endpoint`
3. Each stream becomes an HTTP/3 request → maps to `SimpleIncomingRequest`
4. Submits to the same valtron pool as HTTP/1.1 and HTTP/2 requests

### TLS cert reuse

QUIC uses TLS 1.3. The same `CertPair` from `CertManager` (Decision 18) feeds
both the TCP-TLS listener and the QUIC endpoint. No separate cert provisioning.

### Back-end

The proxy does NOT forward via QUIC to backends in stage 4. Backend connections
stay HTTP/1.1 or HTTP/2 over TCP. QUIC backend support is stage 5 — until then,
the proxy terminates QUIC at the front end and re-encodes as HTTP/1.1 to the
backend (same pattern as TLS termination for HTTPS).

## Constraints

| Item | Stage 1 | Stage 4 (H2) | Stage 4+ (H3) |
|------|---------|-------------|---------------|
| Transport | TCP | TCP + TLS | TCP + TLS + UDP |
| Front-end protocol | HTTP/1.1 | + HTTP/2 | + HTTP/3 (QUIC) |
| Backend protocol | HTTP/1.1 | + HTTP/2 | HTTP/1.1 + HTTP/2 (QUIC deferred) |
| TLS | None | rustls | rustls (shared with QUIC) |
| UDP dependency | None | None | QUIC driver (UDP socket) |

## Dependencies

| Crate | Role |
|-------|------|
| `foundation_netio::quic` | QUIC driver (already implements TaskIterator) |
| `quinn` or `quinn-proto` | QUIC protocol (already in netio deps) |
| `rustls` | TLS for QUIC (shared with HTTPS) |

## Verification

1. Proxy with `http3_enabled: true`, curl `--http3 https://app.local` → HTTP/3 response.
2. QUIC connection migration: change client IP, request continues on same stream.
3. 0-RTT: reconnect within session window → no handshake latency.
4. Fallback: client without QUIC → HTTPS (HTTP/2) → still works.
