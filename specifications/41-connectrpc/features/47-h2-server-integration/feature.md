---
feature: "HTTP/2 server integration: H2Conn, request→response pipe bridge, H2ConnectionHandler"
description: "Full h2 server: SimpleIncomingRequestHeader + Stream<H2IncomingFrame> → dispatch → Pipe<H2Frame>, all 4 RPC modes, zero buffering"
status: "pending"
priority: "high"
phase: 2
depends_on: ["29-http2-substrate", "30-http2-multiplexers", "22-router-dispatch"]
estimated_effort: "medium"
created: 2026-07-09
---
# Feature 47-h2-server-integration

## Description

Full HTTP/2 server integration. `H2ConnectionHandler` reads h2 frames, decodes
each stream into `SimpleIncomingRequestHeader` + `Stream<H2IncomingFrame>`,
dispatches through `ConnectRpcHandler`, and bridges the handler's output to
`Pipe<H2Frame>` — all four RPC modes, no OOM collection.

## Design

### Types (foundation_netio)

```rust
/// Decoded HEADERS frame without the body — the "everything except body" type.
///
/// `uri` and `url` are computed once from `:scheme`/`:authority`/`:path` so no
/// downstream caller ever reconstructs (or fabricates) them.
pub struct SimpleIncomingRequestHeader {
    pub method: SimpleMethod,
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub headers: SimpleHeaders,
    pub connection: Arc<ConnectionContext>,
    pub uri: Uri,
    pub url: SimpleUrl,
}

/// Fallible: a malformed `:path`/`:authority` has no meaningful fallback.
/// The caller answers with a stream PROTOCOL_ERROR. Never fabricate a `Uri`.
/// Origin-form requests (no `:authority`) parse path-only, which `Uri::parse`
/// accepts. `url` is derived from the same `Uri`, mirroring `redirects.rs`.
pub fn header_from_hpack(
    pairs: &[(Bytes, Bytes)],
    connection: Arc<ConnectionContext>,
) -> Result<SimpleIncomingRequestHeader, InvalidUri>;

/// One frame after HEADERS on an incoming stream.
/// Stream ends when the body stream returns Ready(None).
pub enum H2IncomingFrame {
    Data(Bytes),
    Reset(ErrorCode),
}

/// One frame the handler pushes into the per-stream response pipe.
/// The connection handler serializes these to the shared write_buf.
pub enum H2Frame {
    Headers { status: u16, headers: Vec<(Bytes, Bytes)>, end_stream: bool },
    Data { payload: Bytes, end_stream: bool },
    Reset { error_code: ErrorCode },
}

/// Concrete h2 connection wrapping H2Connection<SharedByteBufferStream<RawStream>>.
pub struct H2Conn { inner: H2Connection<SharedByteBufferStream<RawStream>> }

impl H2Conn {
    pub fn new_server(stream: SharedByteBufferStream<RawStream>) -> Self;
    pub fn server_handshake(&mut self) -> io::Result<()>;
    pub fn read_frame(&mut self) -> io::Result<(Head, Bytes)>;
    pub fn flush(&mut self) -> io::Result<()>;
    pub fn decode_headers(&mut self, block: &[u8]) -> Result<Vec<(Bytes, Bytes)>, &'static str>;
    pub fn encode_frame(&mut self, stream_id: u32, frame: &H2Frame);
}
```

### H2Serve trait (foundation_http)

The h2 sibling of `Serve`. It mirrors the `Serve` / `ConnectRpcServe` layering —
the trait lives in `foundation_http`, the impl (`ConnectRpcServeH2`) in
`foundation_connectrpc`. No dependency inversion.

```rust
pub trait H2Serve: Send + Sync + 'static {
    /// Returns a future the connection handler spawns on the valtron pool.
    /// It MUST NOT touch H2Conn: the poll loop is the only writer.
    fn serve_h2(
        &self,
        bag: Arc<ContextBag>,
        header: SimpleIncomingRequestHeader,
        body: PipeReceiver<H2IncomingFrame>,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>>;
}
```

**Rejected shape:** `serve_h2(.., conn: &mut H2Conn, stream_id: u32)` returning
`()`. Handing the handler `&mut H2Conn` forces it to run inline on the poll
loop, which means (a) `block_on(dispatch(..))` stalls the whole connection so a
slow stream 1 head-of-line-blocks stream 3 — h2 multiplexing is defeated; and
(b) the request body must be fully buffered into a `Vec<u8>` before dispatch,
and streaming responses are inexpressible. Only unary would ever work. The pipe
shape keeps `H2Conn` owned solely by the poll loop.

### ServerApp — protocol → app routing (foundation_http)

`HttpServer` no longer holds a single `HttpApp`. Middleware and handler types
differ between protocols (`Serve` takes `SimpleIncomingRequest`, `H2Serve` takes
a header + body pipe), so the two apps cannot be unified behind one type.

```rust
pub enum ServerApp {
    Http1(Arc<HttpApp<Arc<dyn Serve>>>),
    Http2(Arc<HttpApp<Arc<dyn H2Serve>>>),
    Both {
        http1: Arc<HttpApp<Arc<dyn Serve>>>,
        http2: Arc<HttpApp<Arc<dyn H2Serve>>>,
    },
}
```

Three variants, not four: `Any(Option, Option)` would make the both-`None`
case — a server that can serve nothing — representable, and `All(a, b)` is just
`Any(Some(a), Some(b))`, two encodings of one state. `Both` makes the empty
server unrepresentable by construction. HTTP/3 later adds a variant.

**Protocol mismatch is a hard failure, never a silent default.** After preface
detection the server looks up the app for the detected protocol; if absent:

| Detected | No app for it | Response |
|---|---|---|
| h2c preface | `ServerApp::Http1` | GOAWAY + close |
| HTTP/1.1 | `ServerApp::Http2` | `505 HTTP Version Not Supported`, close |

Falling back to `HttpApp::default()` (an empty router that 404s every request)
is forbidden — it reports "route not found" for what is actually "this server
does not speak your protocol".

### h2c preface detection (foundation_http)

Detection **compares bytes**, it does not count them:

```rust
// CLIENT_PREFACE = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"  (24 bytes)
let is_h2c = stream.peek(CLIENT_PREFACE_LEN)? == CLIENT_PREFACE;
```

A length-only check (`n >= CLIENT_PREFACE.len()`) misroutes every HTTP/1.1
request of 24+ bytes into the h2 handler. The peek must not consume: on the h1
path the bytes are still needed by `HTTPStreams`; on the h2 path
`server_handshake()` consumes the preface itself.

### ConnectRpcHandler — new h2 dispatch method

`dispatch()` already handles decision phase (path lookup, method, protocol, codec, capability).
We add `dispatch_h2()` that reuses the same decision phase but bridges output directly
to `Pipe<H2Frame>` — no `SimpleOutgoingResponse` collection.

```rust
impl ConnectRpcHandler {
    /// Existing: full dispatch → collected SimpleOutgoingResponse.
    pub fn dispatch(&self, bag: Arc<ContextBag>, request: SimpleIncomingRequest)
        -> BoxFuture<'static, SimpleOutgoingResponse>;

    /// h2-streaming dispatch: bridges handler output to H2Frame pipe.
    /// Handles all four RPC modes. No intermediate collection.
    pub fn dispatch_h2(
        &self,
        bag: Arc<ContextBag>,
        header: &SimpleIncomingRequestHeader,
        body: impl Stream<Item = H2IncomingFrame> + Send + 'static,
        tx: PipeSender<H2Frame>,
    ) -> BoxFuture<'static, io::Result<()>>;
}
```

#### How `dispatch_h2()` works per mode

**Unary** (body stream yields 0–N Data frames, handler returns a single response):
```
1. Decision phase: lookup path, match protocol, resolve codec, check capability
2. Collect body from body stream into Bytes (typically 0 or 1 DATA frames)
3. Build SimpleIncomingRequest from header + collected body
4. protocol.decode_unary_request() → run erased handler (produces frame bytes)
5. protocol.encode_unary_response() → framed response bytes
6. H2Frame::Headers { 200, end_stream: false } → tx
7. H2Frame::Data { response_bytes, end_stream: true } → tx
8. return Ok(())
```

**Server-stream** (body stream yields 0–N Data frames, handler returns a stream):
```
1. Decision phase
2. Collect body from body stream (Request has no body in typical server-stream,
   but the body stream is always present and may yield DATA frames)
3. Build SimpleIncomingRequest from header + collected body
4. protocol.new_conn() → HandlerExchange { conn, reader_task, writer_task }
   - request pipe: closed immediately (request body already collected)
   - response pipe: writer_task writes framed response bytes here
5. handler.handle(ctx, codec_name, conn) — produces stream through conn
6. Bridge: tx.send(H2Frame::Headers { 200, end_stream: false }).await?;
   while let Some(chunk) = resp_rx.receive().await {
       tx.send(H2Frame::Data { payload: chunk, end_stream: false }).await?;
   }
   tx.send(H2Frame::Data { payload: Bytes::new(), end_stream: true }).await?;
7. join!(handler_fut, writer_task, bridge)
8. return Ok(())
```

**Client-stream** (body stream has DATA frames, handler returns unary):
```
1. Decision phase
2. Build SimpleIncomingRequest with streaming body
3. protocol.new_conn() → HandlerExchange
   - request pipe: feed from body stream: H2IncomingFrame::Data(chunk) → req_tx
   - response pipe: writer_task writes framed response here
4. handler.handle(ctx, codec_name, conn) — consumes request stream, produces response
5. Bridge: feed body → req_tx, collect response → H2Frame::Headers + H2Frame::Data → tx
6. join!(body_feed, reader_task, writer_task, handler_fut)
7. return Ok(())
```

**Bidi-stream** (body stream has DATA frames, handler produces stream):
```
1. Decision phase
2. Build SimpleIncomingRequest with streaming body
3. protocol.new_conn() → HandlerExchange
4. handler.handle(ctx, codec_name, conn) — reads request frames, writes response frames
5. Bridge: body → req_tx, response pipe → H2Frame::Data → tx, interleaved
   tx.send(H2Frame::Headers { 200, end_stream: false }).await?;
   futures::join!(
       feed_body(body_stream, req_tx),     // H2IncomingFrame::Data → req_tx
       drain_resp(resp_rx, tx.clone()),     // resp_rx chunks → H2Frame::Data → tx
       handler_fut,
       reader_task,
       writer_task,
   );
6. tx.send(H2Frame::Data { payload: Bytes::new(), end_stream: true }).await?;
7. return Ok(())
```

### H2ConnectionHandler (foundation_http)

Valtron `TaskIterator`. Each poll:

```
1. conn.flush()
2. conn.read_frame() → (head, payload)
3. Dispatch:
   HEADERS (new stream):
     - conn.decode_headers(payload) → (name, value) pairs
     - header_from_hpack(pairs, conn_ctx)? → SimpleIncomingRequestHeader
       Err(InvalidUri) → encode RST_STREAM(PROTOCOL_ERROR), drop stream, continue
     - router.dispatch(&method, &url) → None → RST_STREAM / 404 HEADERS
     - create Pipe<H2IncomingFrame> for body → body_rx, Pipe<H2Frame> for response → resp_tx
     - valtron::send(FutureTask::new(serve_h2(..)))  ← SPAWNED, not called inline
       → the handler future runs on the valtron pool, pushes H2Frames into resp_tx
     - store (body_tx, resp_rx) in streams map for draining
     - if END_STREAM was set on HEADERS: body_tx.close() immediately (no body)
   DATA (stream_id):
     - streams[stream_id].body_tx.send(H2IncomingFrame::Data(payload))
   DATA(END_STREAM):
     - streams[stream_id].body_tx.close()
   RST_STREAM (stream_id):
     - streams[stream_id].body_tx.send(H2IncomingFrame::Reset(code)) → close
     - remove stream from map
4. Drain all active response pipes:
   for (sid, rx) in &mut streams:
       while let Some(frame) = rx.try_recv() {
           conn.encode_frame(sid, &frame);
       }
5. Reap: drop streams whose resp_rx is closed and drained
6. WouldBlock → TaskStatus::Delayed(10ms)
   (matches existing ConnectionHandler pattern — same gap for both h1 and h2)
```

### BLOCKER: `H2Connection` assumes a blocking socket

Discovered while running `h2_echo` end-to-end. `H2Connection` (F29) was written
against a blocking socket, but `HttpServer` sets every accepted socket
non-blocking. Both of its read paths use `read_exact`:

- `server_handshake()` — reads the 24-byte preface, then the client SETTINGS,
  then flushes, then waits for the client's SETTINGS ACK. The ACK cannot have
  arrived yet, so it returns `WouldBlock` **after** consuming the preface. The
  poll loop parks and calls `server_handshake()` again, which re-reads 24 bytes
  — now the SETTINGS frame — and reports `invalid HTTP/2 connection preface`.
  Observed exactly this: detection sees the correct preface, then the handshake
  fails on the retry. The function is not resumable.
- `read_frame()` — `read_exact(header)` / `read_exact(payload)` can consume a
  partial frame and then error, leaving the stream mid-frame. Same latent
  corruption, just harder to trigger on loopback.

Fix (frame-atomic, resumable):

1. `H2Conn` keeps a clone of the `SharedByteBufferStream` (the buffer is shared)
   and pre-buffers with `peek` before consuming: `read_frame` peeks 9 bytes,
   parses the length, peeks `9 + len`, and only then delegates. If the whole
   frame is not buffered it returns `WouldBlock` having consumed nothing.
   `read_exact` then never touches the socket and cannot partially fail.
2. Split `H2Connection::server_handshake` into `server_handshake_recv` (preface
   + client SETTINGS + our SETTINGS/ACK flush) and `server_handshake_ack` (await
   the client's ACK), and drive them from a small resumable state machine in
   `H2Conn` (`Preface → Ack → Done`), each phase pre-buffered as above.

Also found: `SharedByteBufferStream::peek` panics on EOF. `peekby` returns
`PeekState::NoNext` when the peer has closed and the buffer is empty, and the
`peek` impl matches `_ => unreachable!("We should never hit this state")`. A
peer that connects and immediately disconnects therefore panics the detect task.
This must be fixed (return `Ok(0)` or an EOF error) before the h2 path is
exposed to the open internet.

### Deferred: reactor parking

Both `ConnectionHandler` (h1) and `H2ConnectionHandler` (h2) use
`TaskStatus::Delayed(duration)` with escalation when the socket returns
`WouldBlock`. This is polite — the valtron executor re-polls on timer expiry —
but wasteful: the fd may have data 3ms later but we wait 10ms.

The infrastructure for true reactor parking already exists:

- `RawStream` impls `AsRawFd` (Decision 12 §12)
- `foundation_nativeapis` provides `RegisteredFd<T>` (Feature 10, complete)
- `RegisteredFd: EventReadiness` — the `Depends` trait
- `TaskStatus::Depends(Arc<dyn EventReadiness>)` — a valtron task parks on the
  kernel wake rather than a timer

What's missing: the integration layer that registers the fd when the handler
starts and unregisters it on close. The existing `ConnectionHandler` doesn't
have this either — it's not a regression, it's a shared deferred gap.

**Separate feature: `47b-h2-reactor-parking`** (or a common h1+h2 parking
feature) would:
1. Accept a `RegisteredFd` constructor in `H2ConnectionHandler::new()`
2. On `WouldBlock`, spawn a tiny evented task that registers the fd and parks
   on `Depends(RegisteredFd)` — wakes when kernel signals data
3. On poll exit, return `Pending` which the executor schedules on wake
4. On connection close, unregister the fd

This is h2- and h1-agnostic — any `TaskIterator` over a non-blocking fd can
use it. Tracked here as a dependency so this feature doesn't claim reactor
parking is addressed.

**What is spawned, and what is not:** `HttpServer` already spawns one valtron
task per accepted *connection* — `H2ConnectionHandler` is that task. Nothing
re-spawns a thread per connection, and no `thread::spawn` + `block_on` appears
anywhere in this feature. What F47 adds is one valtron task per h2 *stream*,
which is the whole point of h2: a connection carries many concurrent streams.

**Why spawning works for multiplexing:** The poll loop never blocks on a
handler. It creates pipes, spawns `dispatch_h2()` on the valtron pool, and
continues reading frames. Multiple streams can be active at once — stream 1
streaming a response while stream 3 is still receiving request DATA.
The shared `write_buf` is only touched by this one poll loop (one
TaskIterator = one writer), so there is no contention. Frame ordering across
streams is determined by the order the poll loop drains pipes, which is
naturally fair (round-robin or in stream-ID order).

### H2Transport (connectrpc client side)

Already built in F30. `H2Transport::open()` connects via TCP, handshakes h2,
spawns valtron pump, returns `TransportStream`. `Client::unary()` already works.

For F47 client streaming support, the `H2Pump` TaskIterator bridges request body
from `send_rx` (Pipe<Bytes>) to h2 DATA frames on the outgoing stream, and
response DATA frames back to `body_tx`. This is the same bridge pattern as the
server side.

### Scope

| Item | Crate | File |
|---|---|---|
| `SimpleIncomingRequestHeader` (+ `uri`/`url`) | foundation_netio | `http2/types.rs` |
| `header_from_hpack()` → `Result` | foundation_netio | `http2/types.rs` |
| `H2IncomingFrame` | foundation_netio | `http2/types.rs` |
| `H2Frame` | foundation_netio | `http2/types.rs` |
| `H2Conn` | foundation_netio | `http2/conn.rs` |
| `H2Serve` trait | foundation_http | `shared/serve/h2.rs` |
| `ServerApp` enum | foundation_http | `shared/app/mod.rs` |
| `H2ConnectionHandler` | foundation_http | `native/server/h2_connection.rs` |
| h2c preface detect + mismatch fail | foundation_http | `native/server/mod.rs` |
| `dispatch_h2()` | foundation_connectrpc | `router/dispatch.rs` |
| `ConnectRpcServeH2` | foundation_connectrpc | `h2_serve.rs` |
| `h2_echo` example | foundation_connectrpc | `examples/h2_echo/main.rs` |
| `H2Transport` client streaming | foundation_connectrpc | `transport/h2.rs` |

### Acceptance criteria

- Unary: `Router::unary()` over h2c round-trips ✅
- Server-stream: handler produces stream, chunks delivered as h2 DATA frames ✅
- Client-stream: client sends DATA frames, handler receives them via `H2IncomingFrame` stream ✅
- Bidi-stream: interleaved send/recv via `futures::join!` ✅
- Poll loop never blocks: no `block_on` and no full-body `Vec` buffering in
  `H2ConnectionHandler`; two concurrent streams interleave on one connection ✅
- An HTTP/1.1 request ≥24 bytes is **not** misdetected as h2c ✅
- `ServerApp::Http1` + h2c preface → GOAWAY; `ServerApp::Http2` + h1 → 505.
  Never an empty-router 404 ✅
- All 87 F29 tests pass, zero warnings ✅
