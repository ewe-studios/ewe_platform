---
feature: "Alt-Svc advertisement + H3 ConnectionContext population (F35 tail / D12 §9)"
description: "Advertise HTTP/3 from TCP servers via Alt-Svc header; populate ConnectionContext from QUIC handshake at H3 accept time"
status: "in-progress"
priority: "low"
phase: 5
depends_on: ["35-http3-transport-integration"]
estimated_effort: "small"
created: 2026-07-12
---
# Feature 53: Alt-Svc advertisement + ConnectionContext population for H3

## Why this exists

F35 delivered the H3 server half — clients can speak HTTP/3 to the server.
Two things that make the experience seamless are still missing:

1. **Alt-Svc advertisement**: A client connecting over TCP (HTTP/1.1 or HTTP/2)
   has no way to discover that the same origin speaks HTTP/3. `Alt-Svc: h3=":<port>"`
   is the standard mechanism (RFC 7838) — a response header the TCP server adds
   so the client can upgrade to QUIC on subsequent requests.

2. **ConnectionContext population**: `dispatch_h3` passes
   `Arc::new(ConnectionContext::default())` — empty. The peer's network address,
   the negotiated ALPN protocol (`h3`), and any QUIC-level metadata (connection
   id, 0-RTT flag) should be populated from the QUIC handshake at accept time,
   exactly as the H1 server does for TCP connections.

## Design

### Part A — Alt-Svc advertisement

A new optional field on `ServerConfig`:

```rust
pub struct ServerConfig {
    // ... existing fields ...
    /// When set, every HTTP/1.1 and HTTP/2 response carries
    /// `Alt-Svc: h3=":<port>"` telling clients that this origin
    /// also speaks HTTP/3 on `port`.
    pub alt_svc_h3_port: Option<u16>,
}
```

The header is injected in the HTTP response rendering path — close to where
status-line and other response headers are serialised. A `None` value (the
default) means no header is added; existing servers are completely unaffected.

### Part B — ConnectionContext from QUIC handshake

`QuinnConnection` gains a `connection_context()` method that reads the peer
address from its internal `ConnState` and builds a `ConnectionContext` with:

- `peer_addr` — the remote socket address
- `alpn` — `Some(b"h3".to_vec())` (the QUIC handshake always negotiates h3)
- `quic_connection_id` — `None` for now (quinn-proto exposes connection IDs
  through its public API, but we don't need them yet)

The `dispatch_h3` function (and its `H3Serve` callers) accept an
`Arc<ConnectionContext>` instead of constructing a default. The `QuicListener`
(or the accept loop around it) builds the context once per connection and
passes it through.

## Scope

- `foundation_http`:
  - `ServerConfig` gets `alt_svc_h3_port: Option<u16>` + `with_alt_svc_h3(port)`
  - Response rendering injects `Alt-Svc` header when port is set
- `foundation_netio`:
  - `QuinnConnection::connection_context()` — builds `ConnectionContext` from
    the QUIC handshake peer address
- `foundation_connectrpc`:
  - `dispatch_h3` accepts `Arc<ConnectionContext>` parameter instead of
    constructing a default

## Verification

- `cargo check -p foundation_http --features quic` — compiles
- `cargo check -p foundation_netio --features quic` — compiles
- `cargo check -p foundation_connectrpc --features h3,rpc_multi` — compiles
- Existing H3 conformance test still passes
- ServerConfig::default().alt_svc_h3_port is None — no header by default

## Acceptance criteria

- An HTTP/1.1 or HTTP/2 response from a server configured with
  `ServerConfig::defaults().with_alt_svc_h3(443)` carries the header
  `Alt-Svc: h3=":443"`.
- `dispatch_h3` receives a populated `ConnectionContext` (non-empty peer_addr,
  ALPN "h3") rather than an empty default.
- No behaviour change for servers that don't set `alt_svc_h3_port`.
- All existing tests pass.
