---
feature: "Native Proxy"
description: "Replace hyper/axum/tower proxy with foundation_core networking + raw socket I/O for HTTP/1 proxy and TCP tunnel"
status: "pending"
priority: "high"
depends_on: ["02-task-operators", "specifications/34-native-file-watchers/01-native-apis"]
estimated_effort: "large"
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
- Uses `tokio::io::{AsyncReadExt, AsyncWriteExt}` for streaming
- Uses `tokio::io::copy_bidirectional` for tunnel streaming
- `Http1Service` implements `hyper::service::Service<Request>` — tight hyper coupling
- Supports HTTP/1, HTTP/2, HTTP/3, and TCP tunnel — but only HTTP/1 and tunnel are implemented

## Solution

Replace with raw socket I/O using `foundation_nativeapis` poll-layer networking:

### TcpListener + Accept Loop

```rust
// Instead of: tokio::net::TcpListener::bind(addr).await
let listener = std::net::TcpListener::bind(addr)?;
// listener.set_nonblocking(true) for integration with poll-layer
```

For valtron integration, the listener fd is registered with the poll-layer Selector:
```rust
let poll = Poll::new()?;
let mut events = Events::with_capacity(64);
poll.registry().register(
    &mut SourceFd(&listener.as_raw_fd()),
    TOKEN_LISTENER,
    Interest::READABLE,
)?;

// In TaskIterator::next_status():
poll.poll(&mut events, Some(Duration::from_millis(50)))?;
for event in &events {
    if event.token() == TOKEN_LISTENER {
        let (stream, addr) = listener.accept()?;
        // handle connection
    }
}
```

### HTTP/1 Request Handling

Parse HTTP request from raw bytes, forward to upstream, relay response:

```rust
fn handle_http1_request(mut client_stream: std::net::TcpStream, config: Http1Config) -> Result<()> {
    // 1. Read HTTP request from client
    let mut buf = [0u8; 8192];
    let n = client_stream.read(&mut buf)?;
    let request = parse_http_request(&buf[..n])?;

    // 2. Check static routes (SSE, reloader.js)
    if let Some(handler) = config.routes.get(request.path) {
        let response = handler(&request);
        client_stream.write_all(&response)?;
        return Ok(());
    }

    // 3. Forward to upstream
    let mut upstream = std::net::TcpStream::connect(config.destination)?;
    upstream.write_all(&buf[..n])?;

    // 4. Relay upstream response to client
    // Use poll-layer for bidirectional streaming
    copy_bidirectional_poll(&mut client_stream, &mut upstream)?;
    Ok(())
}
```

### HTTP Request Parser (minimal)

```rust
struct HttpRequest {
    method: http::Method,
    uri: http::Uri,
    version: HttpVersion,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn parse_http_request(buf: &[u8]) -> Result<HttpRequest> {
    // Parse: METHOD PATH HTTP/1.1\r\nHeader: Value\r\n\r\nBody
    // Minimal parser sufficient for dev proxy needs
}
```

### Bidirectional Copy via poll-layer

```rust
fn copy_bidirectional_poll(
    client: &mut std::net::TcpStream,
    upstream: &mut std::net::TcpStream,
) -> io::Result<()> {
    // Register both fds with poll-layer
    // Loop: poll → read from readable → write to other → repeat
    // This replaces tokio::io::copy_bidirectional
}
```

### TCP Tunnel

Same approach but without HTTP parsing — raw TCP bidirectional copy:

```rust
fn handle_tunnel(
    client: std::net::TcpStream,
    client_addr: SocketAddr,
    tunnel: Tunnel,
) -> Result<()> {
    let mut upstream = std::net::TcpStream::connect(tunnel.destination.to_string())?;
    copy_bidirectional_poll(&mut client, &mut upstream)?;
    Ok(())
}
```

### HTTP/2 and HTTP/3

Mark as `todo!()` or stub — the current devserver doesn't implement HTTP/2 or HTTP/3 serving anyway (they return `TunnelNotSupported`). Remove the types but keep the enum variants for API compatibility.

### Task Breakdown

1. [ ] Define `HttpRequest` / `HttpResponse` minimal structs (using `http::Method`, `http::Uri`, `http::StatusCode` — data types only)
2. [ ] Implement minimal HTTP request parser
3. [ ] Implement minimal HTTP response writer
4. [ ] Implement `copy_bidirectional_poll` using poll-layer
5. [ ] Implement `ProxyTask` TaskIterator with TcpListener accept loop
6. [ ] Implement HTTP/1 proxy forwarding
7. [ ] Implement TCP tunnel
8. [ ] Implement static route handler dispatch
9. [ ] Remove hyper/axum/tower/h2/h3 from imports
10. [ ] Write tests: HTTP parsing, response writing, proxy forwarding

### Dependencies from Spec-34

Requires `foundation_nativeapis` with `poll` feature for:
- `Poll`, `Registry`, `Token`, `Interest`, `Events`
- `SourceFd` for registering raw fds
- Networking types (if not using std::net directly)

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_toolings/src/proxy/mod.rs` | Create — ProxyTask |
| `backends/foundation_toolings/src/proxy/http1.rs` | Create — HTTP/1 parser + handler |
| `backends/foundation_toolings/src/proxy/tunnel.rs` | Create — TCP tunnel |
| `backends/foundation_toolings/src/proxy/http_parser.rs` | Create — minimal HTTP parser |

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| HTTP parsing | Minimal hand-written parser | No need for full HTTP parser; dev proxy only needs path/method/headers |
| Socket I/O | `std::net` + poll-layer | Non-blocking without async runtime |
| HTTP/2, HTTP/3 | Stub/removed | Not implemented in current devserver anyway |
| `http` crate | Keep (data types) | `Method`, `Uri`, `StatusCode` are pure data, no runtime dependency |

---

_Created: 2026-06-01_
