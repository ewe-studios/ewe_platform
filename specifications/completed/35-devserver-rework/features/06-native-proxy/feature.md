---
feature: "Native Proxy"
description: "Use foundation_http (HttpServer + HttpApp + Serve) for HTTP/1 proxy + SSE routing, with TunnelProxy for raw TCP tunneling on the same port"
status: "complete"
priority: "high"
depends_on: ["02-task-operators"]
estimated_effort: "small"
created: "2026-06-01"
last_updated: "2026-06-15"
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

Use **`foundation_http`** — which already provides a full valtron-integrated HTTP server with routing, middleware, keep-alive, and static asset serving. Plus a **`TunnelProxy`** for non-HTTP traffic on the same port.

### Proxy Architecture

```
Browser ──(TCP)──► HttpServer (foundation_http)
                      │
                      ├─ ConnectionHandler (valtron TaskIterator per connection)
                      │    ├─ HTTP detected → route dispatch
                      │    │    ├─ /static/sse/reloader.js → StaticAssetHandler (served locally)
                      │    │    ├─ /static/sse/reload → SseReloadHandler (ConnectionResult::Take)
                      │    │    └─ /* → ProxyForwarder (forward to upstream via HttpConnectionPool)
                      │    │
                      │    └─ Non-HTTP detected → TunnelProxy
                      │         └─ copy_bidirectional to upstream
                      │
                      └─ HttpConnectionPool (persistent upstream connections)
```

### HttpApp setup

```rust
use foundation_http::{
    shared::{app::HttpApp, context::ContextBag, serve::Serve},
    native::server::HttpServer,
    native::handlers::static_asset::StaticAssetHandler,
};

// 1. Create app with Serve handlers
let mut app = HttpApp::new_serve();

// 2. Store shared state in ContextBag
app.ctx.store(reload_channel.clone());
app.ctx.store(upstream_config.clone());

// 3. Register routes
// Static reloader.js — embedded asset, already exists in foundation_http
let reloader_js = StaticAssetHandler::new(RELOADER_JS_BYTES, "text/javascript");
app.router().add_route_any("/static/sse/reloader.js", Arc::new(reloader_js));

// SSE reload endpoint — custom Serve handler
app.route_any::<SseReloadHandler>("/static/sse/reload");

// Catch-all — forward to upstream backend
app.route_any::<ProxyForwarder>("/");

// 4. Register TunnelProxy for non-HTTP traffic
app = app.tunnel_proxy(TunnelProxy {
    dest: upstream_config.address.clone(),
});

// 5. Create and start server
let server = app.server("0.0.0.0:8080");
server.serve(&shutdown_signal);
```

### TunnelProxy (non-HTTP tunnel)

```rust
/// Registered once on HttpApp, cloned into each ConnectionHandler.
/// Runs on the same valtron thread that owns the connection.
#[derive(Copy, Clone)]
pub struct TunnelProxy {
    pub dest: String,
}

impl foundation_http::shared::tunnel_proxy::TunnelProxyTrait for TunnelProxy {
    fn handle(
        &self,
        mut conn: SharedByteBufferStream<RawStream>,
        _streams: HTTPStreams<RawStream>,
        _client_ip: &str,
    ) -> ConnectionResult {
        // Connect to upstream using foundation_netio raw TCP
        match foundation_netio::netcap::Connection::connect(&self.dest) {
            Ok(mut upstream) => {
                // Bidirectional copy — runs to completion on this valtron thread
                copy_bidirectional(&mut conn, &mut upstream);
                ConnectionResult::Take
            }
            Err(e) => {
                tracing::error!("Tunnel upstream connect failed: {e}");
                ConnectionResult::Close(None)
            }
        }
    }
}
```

### TunnelProxyTrait (to be added to foundation_http)

```rust
// foundation_http::shared::tunnel_proxy

/// User-provided handler for non-HTTP connections on the same port.
/// Invoked when `read_next_request()` fails with a non-transient error.
/// Runs on the same valtron TaskIterator thread that owns the connection.
pub trait TunnelProxyTrait: Send + Copy + 'static {
    fn handle(
        &self,
        conn: SharedByteBufferStream<RawStream>,
        streams: HTTPStreams<RawStream>,
        client_ip: &str,
    ) -> ConnectionResult;
}
```

**Integration point in `ConnectionHandler::handle_idle()`:**
```rust
// Current code (line 224-247 of connection.rs):
Some(Err(e)) => {
    if Self::is_transient_error(&e) {
        // delay + retry
    } else {
        // TODO: if self.tunnel_proxy.is_some() → invoke instead of 400
        let _ = respond::text(&mut self.conn.clone(), 400, "Bad Request");
        None
    }
}
```

### ProxyForwarder (Serve handler for catch-all)

```rust
/// Forwards HTTP requests to upstream backend, streams response back.
pub struct ProxyForwarder {
    pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
    reload_tx: broadcast::Sender<FileChange>,
}

impl ServeFactory for ProxyForwarder {
    fn create(bag: &ContextBag) -> Self {
        Self {
            pool: bag.get::<HttpConnectionPool<_>>().expect("pool in context"),
            reload_tx: bag.get::<broadcast::Sender<FileChange>>().expect("reload tx"),
        }
    }
}

impl Serve for ProxyForwarder {
    fn serve(
        &self,
        _bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        // 1. Get pooled upstream connection
        let mut upstream = self.pool.create_http_connection(&dest_uri, None)?;

        // 2. Forward request via Http11RequestIterator
        let request = SimpleIncomingRequest::builder()
            .with_method(req.method)
            .with_url(req.request_url)
            .with_proto(req.proto)
            .with_headers(req.headers)
            .with_some_body(req.body)
            .build()?;

        let http11_iter = Http11RequestIterator::new(request);
        for chunk in http11_iter {
            upstream.stream.write_all(&chunk?)?;
        }

        // 3. Read upstream response, stream back to client
        let mut response_reader = HttpResponseReader::new(
            upstream.stream.clone(),
            SimpleHttpBody::default(),
        );
        for part in &mut response_reader {
            match part? {
                IncomingResponseParts::Intro(status, proto, _) => {
                    let response = SimpleOutgoingResponse::builder()
                        .with_status(status)
                        .with_proto(proto)
                        .build()?;
                    Http11::response(response).http_render_to_writer(&mut conn)?;
                }
                IncomingResponseParts::Headers(h) => {
                    // forward headers
                }
                IncomingResponseParts::StreamedBody(body) |
                IncomingResponseParts::SizedBody(body) => {
                    // stream body to client
                    stream_body_to_client(&mut conn, body)?;
                }
                IncomingResponseParts::NoBody => break,
                _ => {}
            }
        }

        // 4. Return connection to pool
        self.pool.return_to_pool(upstream);

        ConnectionResult::Keep
    }
}
```

### Bidirectional Copy (raw TCP tunnel)

```rust
fn copy_bidirectional<R: Read, W: Write, S: Read, T: Write>(
    a: &mut SharedByteBufferStream<R>,
    b: &mut Connection<S, T>,
) {
    // Use read timeouts for non-blocking polling under valtron
    let mut buf_a = [0u8; 8192];
    let mut buf_b = [0u8; 8192];

    loop {
        match a.read(&mut buf_a) {
            Ok(0) | Err(_) => break,
            Ok(n) => { _ = b.write_all(&buf_a[..n]); }
        }
        match b.read(&mut buf_b) {
            Ok(0) | Err(_) => break,
            Ok(n) => { _ = a.write_all(&buf_b[..n]); }
        }
    }
}
```

### HTTP/2 and HTTP/3

Mark as `todo!()` or stub — not implemented in current devserver.

### Task Breakdown

1. [ ] Add `TunnelProxyTrait` to `foundation_http::shared::tunnel_proxy`
2. [ ] Wire `TunnelProxyTrait` into `ConnectionHandler::handle_idle()` — invoke on non-transient parse error instead of 400
3. [ ] Add `tunnel_proxy()` method to `HttpApp<Arc<dyn Serve>>`
4. [ ] Create `TunnelProxy` in `foundation_toolings::proxy::tunnel_proxy`
5. [ ] Create `ProxyForwarder` Serve handler in `foundation_toolings::proxy::handlers`
6. [ ] Wire `StaticAssetHandler` for reloader.js (already exists in foundation_http)
7. [ ] Create `SseReloadHandler` Serve handler (see feature 07)
8. [ ] Build `HttpApp` + `HttpServer` in `ProxyTask`
9. [ ] Write tests: HTTP forwarding, tunnel proxy, static asset serving

### Dependencies

```toml
foundation_http = { workspace = true }
foundation_netio = { workspace = true }
```

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_http/src/shared/tunnel_proxy/mod.rs` | Create — `TunnelProxyTrait` (foundation_http extension) |
| `backends/foundation_http/src/native/server/connection.rs` | Edit — invoke tunnel proxy in `handle_idle()` on non-transient error |
| `backends/foundation_http/src/shared/app/mod.rs` | Edit — add `tunnel_proxy()` method to `HttpApp` |
| `backends/foundation_toolings/src/proxy/mod.rs` | Create — ProxyTask builds HttpApp + HttpServer |
| `backends/foundation_toolings/src/proxy/handlers.rs` | Create — ProxyForwarder + SseReloadHandler Serve impls |
| `backends/foundation_toolings/src/proxy/tunnel_proxy.rs` | Create — TunnelProxy TunnelProxyTrait impl |
| `backends/foundation_toolings/src/proxy/copy.rs` | Create — copy_bidirectional helper |

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| HTTP server | `foundation_http::HttpServer` | Already valtron-integrated, handles accept loop, keep-alive, routing |
| Non-HTTP tunnel | `TunnelProxy` on same port | Single port for HTTP + raw TCP, no separate listener |
| Upstream connections | `HttpConnectionPool` — persistent, reused | Real proxies don't reconnect per request |
| HTTP/2, HTTP/3 | Stub/removed | Not implemented in current devserver anyway |

---

_Created: 2026-06-01_
