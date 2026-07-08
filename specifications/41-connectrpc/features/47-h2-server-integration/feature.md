---
feature: "HTTP/2 server integration: H2StreamHandle, ServeH2, H2ConnectionHandler, ConnectRpcServeH2 (D12 §5)"
description: "H2StreamHandle (per-stream write surface), ServeH2 trait, H2ConnectionHandler (valtron TaskIterator), h2c detect in HttpServer accept loop, ConnectRpcServeH2 — h2 serves the same Router as HTTP/1.1"
status: "pending"
priority: "high"
phase: 2
depends_on: ["29-http2-substrate", "30-http2-multiplexers", "22-router-dispatch"]
estimated_effort: "medium"
created: 2026-07-09
---
# Feature 47-h2-server-integration

## Description

Wire HTTP/2 into `foundation_http` so the `HttpServer` accept loop detects h2c,
spawns an `H2ConnectionHandler` on valtron, and dispatches each h2 stream to the
same `HttpApp.router()`. Same `Router`, same handlers — the only difference is
the wire format.

## Design

### Type mapping: HTTP/1.1 vs HTTP/2

| Concept | HTTP/1.1 | HTTP/2 |
|---|---|---|
| Connection type | `SharedByteBufferStream<RawStream>` | `H2Conn` |
| How handlers write | `Http11::response(resp).http_render_to_writer(&mut conn)` | `conn.stream(id).send_response(200, &[], Some(body))` |
| Handler trait | `Serve::serve(bag, req, conn)` | `ServeH2::serve_h2(bag, req, conn, stream_id)` |
| Connection owner | `ConnectionHandler` (valtron TaskIterator) | `H2ConnectionHandler` (valtron TaskIterator) |

### H2Conn — the h2 connection (foundation_netio)

A concrete type (not generic) wrapping `H2Connection<SharedByteBufferStream<RawStream>>`.
Represents one multiplexed h2 connection — owns frame I/O, HPACK state,
SETTINGS, flow control, and per-stream bookkeeping.

```rust
/// One multiplexed HTTP/2 connection — the h2 equivalent of
/// [`SharedByteBufferStream<RawStream>`] for HTTP/1.1.
pub struct H2Conn {
    inner: H2Connection<SharedByteBufferStream<RawStream>>,
}

impl H2Conn {
    /// Create for a server-side connection (even stream IDs for push).
    pub fn new_server(stream: SharedByteBufferStream<RawStream>) -> Self;

    /// Create for a client-side connection (odd stream IDs).
    pub fn new_client(stream: SharedByteBufferStream<RawStream>) -> Self;

    /// Run the server-side h2 handshake (read preface + SETTINGS exchange).
    pub fn server_handshake(&mut self) -> io::Result<()>;

    /// Run the client-side h2 handshake (write preface + SETTINGS exchange).
    pub fn client_handshake(&mut self) -> io::Result<()>;

    /// Read the next frame from the socket.
    pub fn read_frame(&mut self) -> io::Result<(Head, Bytes)>;

    /// Flush the write buffer to the socket.
    pub fn flush(&mut self) -> io::Result<()>;

    /// Get a stream handle for sending on a specific stream.
    pub fn stream(&mut self, stream_id: u32) -> H2StreamHandle<'_>;

    /// The underlying byte stream, for direct I/O.
    pub fn raw(&self) -> &SharedByteBufferStream<RawStream>;
}
```

### H2StreamHandle — per-stream write surface (foundation_netio)

Obtained via `conn.stream(stream_id)`. A focused handle that encodes
responses into the connection's shared write buffer. Users never touch
binary frames directly.

```rust
/// Write handle for one h2 stream, obtained from [`H2Conn::stream`].
///
/// Wraps the connection's HPACK encoder + write buffer + stream ID.
/// Every method encodes the correct frame type and flags.
pub struct H2StreamHandle<'a> {
    stream_id: u32,
    hpack_enc: &'a mut hpack::Encoder,
    write_buf: &'a mut BytesMut,
}

impl H2StreamHandle<'_> {
    /// Send response HEADERS. `end_stream: true` for no-body responses.
    pub fn send_headers(&mut self, status: u16, headers: &[(Bytes, Bytes)], end_stream: bool);

    /// Send a DATA frame.
    pub fn send_data(&mut self, data: &[u8], end_stream: bool);

    /// Convenience: HEADERS + single DATA frame (the common unary case).
    pub fn send_response(&mut self, status: u16, headers: &[(Bytes, Bytes)], body: Option<Bytes>);

    /// Send RST_STREAM.
    pub fn send_reset(&mut self, error_code: ErrorCode);

    /// The stream ID.
    pub fn stream_id(&self) -> u32;
}
```

### Trait: `ServeH2` (foundation_http)

```rust
/// Handler for one h2 stream — the h2 equivalent of [`Serve`].
///
/// Contrast:
/// - `Serve::serve(bag, req, conn: SharedByteBufferStream)` — writes Http11 text
/// - `ServeH2::serve_h2(bag, req, conn: &H2Conn, stream_id)` — writes via H2StreamHandle
pub trait ServeH2: Send + Sync + 'static {
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
        conn: &H2Conn,
        stream_id: u32,
    ) -> io::Result<()>;
}
```

The handler receives the full `H2Conn` so it can create stream handles as needed.
For the common case it just calls `conn.stream(stream_id).send_response(...)`.

### H2ConnectionHandler (foundation_http)

The valtron `TaskIterator` that owns one multiplexed h2 connection.

```
H2ConnectionHandler
├── conn: H2Conn                                     (the h2 connection)
├── app: Arc<HttpApp<Arc<dyn ServeH2>>>              (routes → ServeH2 handlers)
├── shutdown, drain_guard, idle timeout
```

Each `next_status()` poll:

1. `conn.flush()` — drain write buffer to socket
2. `conn.read_frame()` — try to decode one frame
3. HEADERS frame → HPACK decode → `SimpleIncomingRequest` →
   `app.router().dispatch()` → `handler.serve_h2(bag, req, &conn, stream_id)`
4. `WouldBlock` → `TaskStatus::Delayed(10ms)`

Request decoding (HPACK → `SimpleIncomingRequest`) lives in `H2Conn` or a
free function — the handler doesn't touch HTTP/2 framing at all.

### Impl: `ConnectRpcServeH2` (foundation_connectrpc)

```rust
pub struct ConnectRpcServeH2 {
    handler: Arc<ConnectRpcHandler>,
}

impl ServeH2 for ConnectRpcServeH2 {
    fn serve_h2(&self, bag, req, conn: &H2Conn, stream_id: u32) -> io::Result<()> {
        let response = block_on(self.handler.dispatch(bag, req));
        let (status, headers, body) = response_to_h2_parts(response);
        let mut stream = conn.stream(stream_id);
        stream.send_response(status, &headers, body);
        Ok(())
    }
}
```

Same dispatch as `ConnectRpcServe` (which impls `Serve` for h1) — the only
difference is `conn.stream(id).send_response(...)` instead of
`Http11::response(resp).http_render_to_writer(&mut conn)`.

### `HttpServer` accept loop — h2c detection

After `accept()` and connection setup, peek 24 bytes from the socket:

```
let shared = SharedByteBufferStream::rwrite(raw_stream);
peek 24B
├── "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" → h2c
│   ├── let mut conn = H2Conn::new_server(shared);
│   ├── conn.server_handshake();
│   └── valtron::send(H2ConnectionHandler::new(conn, app, ...))
│
└── otherwise → HTTP/1.1
    └── valtron::send(ConnectionHandler::new(...))  (existing)
```

### End-to-end flow

```
TcpStream::accept()
  │
  ├─ peek 24B → h2c?
  │
  ├─ YES:
  │   let conn = H2Conn::new_server(shared);
  │   conn.server_handshake();
  │   valtron::send(H2ConnectionHandler::new(conn, app));
  │   │
  │   │  poll:
  │   │  ├─ conn.flush()
  │   │  ├─ conn.read_frame() → HEADERS frame (stream 1)
  │   │  ├─ HPACK decode → SimpleIncomingRequest { method: POST, path: "/echo/Echo", ... }
  │   │  ├─ router.dispatch("POST", "/echo/Echo")
  │   │  │   └─ Some(handler) → handler.serve_h2(bag, req, &conn, 1)
  │   │  │       └─ ConnectRpcServeH2:
  │   │  │           ├─ block_on(ConnectRpcHandler::dispatch(bag, req))
  │   │  │           ├─ → SimpleOutgoingResponse { status: 200, body: ... }
  │   │  │           └─ conn.stream(1).send_response(200, &[], Some(body))
  │   │  │               └─ HPACK-encodes :status + headers → write_buf
  │   │  │               └─ encodes DATA frame → write_buf
  │   │  ├─ WouldBlock → TaskStatus::Delayed(10ms)
  │   │  └─ GOAWAY / timeout → end task
  │   │
  │   └─ next poll: flush writes → socket, read next frame
  │
  └─ NO:
      ConnectionHandler { ... }  (existing HTTP/1.1 path)
```

### Scope

| Item | Crate | File |
|---|---|---|
| `H2Conn` | foundation_netio | `http2/conn.rs` |
| `H2StreamHandle` | foundation_netio | `http2/stream_handle.rs` |
| `ServeH2` trait | foundation_http | `shared/serve/h2.rs` |
| `H2ConnectionHandler` | foundation_http | `native/server/h2_connection.rs` |
| h2c detect branch | foundation_http | `native/server/mod.rs` |
| `ConnectRpcServeH2` | foundation_connectrpc | `server.rs` |
| `h2_echo` example | foundation_connectrpc | `examples/h2_echo/main.rs` |

### Out of scope

- TLS-ALPN entry path (deferred — requires TLS integration)
- `Upgrade: h2c` entry path (deferred — lower priority)
- Client-stream / bidi data forwarding to ServeH2 (initial: unary + server-stream only)
- Per-stream flow-control tuning (F32, deferred profiling)
- Connection-level HPACK state sharing between ServeH2 calls (each stream
  gets an `H2StreamHandle` that borrows the connection's HPACK encoder;
  the encoder accumulates state across streams naturally)

### Acceptance criteria

- `Client::unary()` over `H2Transport` round-trips against h2c server ✅
- Example `h2_echo` runs: same `Router::unary()` + `Client::new()` as `unary_echo`
- h2c detect routes to `H2ConnectionHandler` in `HttpServer::serve_loop()`
- All 87 F29 tests pass, zero warnings
