---
feature: "Native Proxy"
description: "Replace hyper/axum/tower proxy with foundation_netio (Connection, Listener, HttpConnectionPool, simple_http) for HTTP/1 proxy and TCP tunnel"
status: "pending"
priority: "high"
depends_on: ["02-task-operators"]
estimated_effort: "medium"
created: 2026-06-01
last_updated: 2026-06-01
---

# Feature: Native Proxy

## Problem

Current proxy in `crates/devserver/src/proxy.rs` and `crates/devserver/src/streams.rs`:
- Uses `tokio::net::TcpListener` + `tokio::net::TcpStream`
- Uses `hyper::server::conn::http1::Builder` to serve HTTP
- Uses `hyper::client::conn::http1::Builder` to connect to upstream
- Uses `axum::body::Body` for response bodies
- `Http1Service` implements `hyper::service::Service<Request>` — tight hyper coupling

## Solution

Use **`foundation_netio`** — which already provides everything we need:

### Proxy Architecture

```
Browser ──(new conn)──► Proxy ──(pooled conn)──► Upstream Backend
                          │
                    HttpConnectionPool
                    ├── "localhost:3000" ──► [RawStream, RawStream, ...]
                    └── "api.backend:8080" ──► [RawStream]
```

- **Client → Proxy**: New `Connection` per request (via `Listener.accept()`)
- **Proxy → Upstream**: Reuse pooled connections (via `HttpConnectionPool`)
- Connections are keyed by `host:port` and checked in/out with stale expiration

### Networking: `netcap::connection`

```rust
use foundation_netio::netcap::connection::{Listener, Connection, ConfigListenAddr};

// Proxy listens for client connections
let listen_addr = ConfigListenAddr::from_socket_addrs("0.0.0.0:8080")?;
let listener: Listener = listen_addr.bind()?;
let (client_conn, _addr) = listener.accept()?;
```

### Upstream Connection Pool

```rust
use foundation_netio::simple_http::client::native::{HttpConnectionPool, HttpClientConnection};

// Pool manages persistent upstream connections — created once, reused forever
let pool: HttpConnectionPool<SystemDnsResolver> = HttpConnectionPool::default();

// For each client request, get or create upstream connection:
let upstream_uri = Uri::parse("http://localhost:3000")?;
let upstream_conn = pool.create_http_connection(&upstream_uri, None)?;
// If pooled connection available → reuse. Otherwise → new TCP connect.

// After request/response cycle, return to pool:
pool.return_to_pool(upstream_conn);
```

### HTTP: `simple_http` module — iterator-based read/write

`simple_http` uses **iterators** for both reading and writing:
- **Reading**: `HttpRequestReader` / `HttpResponseReader` are `Iterator` types that yield parts (`IncomingRequestParts` / `IncomingResponseParts`)
- **Writing**: `Http11RequestIterator` / `Http11ResponseIterator` are `Iterator` types that yield `Vec<u8>` chunks

### HTTP/1 Proxy Flow

```rust
fn handle_http1_request(
    client_stream: SharedByteBufferStream<RawStream>,
    config: Http1Config,
    route_map: &RouteMap,
    pool: &HttpConnectionPool<SystemDnsResolver>,
) -> Result<()> {
    // Step 1: Parse the incoming request from client
    let mut reader = HttpRequestReader::new(client_stream.clone(), SimpleHttpBody::default());

    // Step 2: Collect parts — intro/headers are read immediately, body is LAZY
    let mut method = None;
    let mut url = None;
    let mut proto = None;
    let mut headers = None;
    let mut lazy_body = None;

    for part in &mut reader {
        match part? {
            IncomingRequestParts::Intro(m, u, p) => {
                method = Some(m); url = Some(u); proto = Some(p);
            }
            IncomingRequestParts::Headers(h) => { headers = Some(h); }
            IncomingRequestParts::StreamedBody(body) | IncomingRequestParts::SizedBody(body) => {
                lazy_body = Some(body);
            }
            IncomingRequestParts::NoBody => { lazy_body = Some(SendSafeBody::None); }
            _ => {}
        }
    }

    // Step 3: Check local routes — only /static/sse/reload
    if let Some(handler) = route_map.get(&url.as_ref().unwrap().url) {
        handler(&method, &headers, &lazy_body, &mut reader, config)?;
        return Ok(());
    }

    // Step 4: Get pooled upstream connection (reuse if available, new if not)
    let upstream_conn = pool.create_http_connection(&config.destination_uri, None)?;

    // Step 5: Build request with LAZY body, render and write to upstream
    let request = SimpleIncomingRequest::builder()
        .with_url(url.unwrap())
        .with_method(method.unwrap())
        .with_proto(proto.unwrap())
        .with_headers(headers.unwrap())
        .with_some_body(lazy_body)  // LAZY — reads from client stream as rendered
        .build()?;

    let http11_iter = Http11RequestIterator::new(request);
    for chunk in http11_iter {
        upstream_conn.stream.write_all(&chunk?)?;
    }

    // Step 6: Read upstream response and stream back to client
    // The client connection is already positioned after the request body was streamed.
    // We read the response from upstream and write it back through the same client stream.
    let mut response_reader = HttpResponseReader::new(
        upstream_conn.stream.clone(),
        SimpleHttpBody::default(),
    );

    // Render response parts to client — headers + lazy body stream back to client
    for part in &mut response_reader {
        match part? {
            IncomingResponseParts::Intro(status, proto, reason) => {
                let response = SimpleOutgoingResponse::builder()
                    .with_status(status)
                    .with_proto(proto)
                    .build()?;
                for chunk in Http11ResponseIterator::new(response) {
                    reader.stream_mut().write_all(&chunk?)?;
                }
            }
            IncomingResponseParts::Headers(h) => {
                // Forward headers to client
                let response = SimpleOutgoingResponse::builder()
                    .with_status(Status::OK)  // placeholder, actual status from Intro
                    .with_headers(h)
                    .build()?;
                // Write headers-only response to client
            }
            IncomingResponseParts::StreamedBody(body) | IncomingResponseParts::SizedBody(body) => {
                // Stream body from upstream directly to client
                stream_body_to_client(reader.stream_mut(), body)?;
            }
            IncomingResponseParts::NoBody => { /* done */ }
            _ => {}
        }
    }

    // Step 7: Return upstream connection to pool for reuse
    pool.return_to_pool(HttpClientConnection {
        stream: upstream_conn.stream,
        host: config.destination_host.clone(),
        port: config.destination_port,
    });

    Ok(())
}
```

**Key points:**
- Upstream connections are **persistent and pooled** — `HttpConnectionPool` manages lifecycle
- Client body streams **lazily** from client → upstream via `Http11RequestIterator`
- Response body streams **lazily** from upstream → client
- After the response is fully relayed, the upstream connection is **returned to the pool**

### TCP Tunnel (protocol-agnostic bridge)

For non-HTTP connections or CONNECT tunnels:

```rust
fn handle_tunnel(
    mut client: Connection,
    tunnel: Tunnel,
    pool: &HttpConnectionPool<SystemDnsResolver>,
) -> Result<()> {
    // Tunnels can also use the pool if they target the same backend
    let mut upstream_conn = pool.create_http_connection(&tunnel.destination_uri, None)?;
    copy_bidirectional(&mut client, &mut upstream_conn.stream.inner())?;
    pool.return_to_pool(upstream_conn);
    Ok(())
}
```

### Bidirectional Copy

```rust
fn copy_bidirectional<R: Read + Write, S: Read + Write>(
    a: &mut R,
    b: &mut S,
) -> io::Result<()> {
    // Set read timeouts for non-blocking polling
    a.set_read_timeout(Some(Duration::from_millis(100)))?;
    b.set_read_timeout(Some(Duration::from_millis(100)))?;

    let mut buf_a = [0u8; 8192];
    let mut buf_b = [0u8; 8192];

    loop {
        match a.read(&mut buf_a) {
            Ok(0) | Err(_) => break,
            Ok(n) => { b.write_all(&buf_a[..n])?; }
        }
        match b.read(&mut buf_b) {
            Ok(0) | Err(_) => break,
            Ok(n) => { a.write_all(&buf_b[..n])?; }
        }
    }
    Ok(())
}
```

### HTTP/2 and HTTP/3

Mark as `todo!()` or stub — not implemented in current devserver.

### Task Breakdown

1. [ ] Wire up `foundation_netio::netcap::connection::Listener` for proxy accept loop
2. [ ] Create `HttpConnectionPool` for upstream connections (shared across all proxy ticks)
3. [ ] Implement request parsing via `HttpRequestReader` (lazy body)
4. [ ] Implement route dispatch: `/static/sse/reload` → SSE handler, all others → forward to upstream
5. [ ] Implement request forwarding: build `SimpleIncomingRequest` with lazy body, render via `Http11RequestIterator`, write to pooled upstream
6. [ ] Implement response relaying: read from upstream via `HttpResponseReader`, stream back to client
7. [ ] Implement upstream connection checkin/checkout lifecycle
8. [ ] Implement TCP tunnel for non-HTTP connections
9. [ ] Remove hyper/axum/tower/h2/h3 from imports
10. [ ] Write tests: proxy forwarding, tunnel streaming, connection pool reuse

### Dependencies

```toml
foundation_netio = { workspace = true, features = ["multi"] }
```

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/proxy/mod.rs` | Create — ProxyTask with HttpConnectionPool |
| `backends/foundation_toolings/src/proxy/http1.rs` | Create — HTTP/1 handler using simple_http |
| `backends/foundation_toolings/src/proxy/tunnel.rs` | Create — TCP tunnel using Connection |

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Upstream connections | `HttpConnectionPool` — persistent, reused | Real proxies don't reconnect per request |
| HTTP parsing | `HttpRequestReader` Iterator | Already built in `simple_http::shared::impls.rs` |
| Body forwarding | Lazy `SendSafeBody::Stream` | No buffering — streams client → upstream directly |
| HTTP/2, HTTP/3 | Stub/removed | Not implemented in current devserver anyway |

---

_Created: 2026-06-01_
