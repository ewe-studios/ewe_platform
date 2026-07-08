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
pub struct SimpleIncomingRequestHeader {
    pub method: SimpleMethod,
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub headers: SimpleHeaders,
    pub connection: Arc<ConnectionContext>,
}

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
     - map pseudo-headers → SimpleIncomingRequestHeader
     - create Pipe<H2IncomingFrame> for body → body_rx, Pipe<H2Frame> for response → resp_tx
     - valtron::send(dispatch_h2 task)  ← SPAWNED, not called inline
       → dispatch_h2 runs on valtron pool, pushes H2Frames into resp_tx
     - store (body_tx, resp_rx) in streams map for draining
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
5. WouldBlock → TaskStatus::Delayed(10ms)
   (matches existing ConnectionHandler pattern; reactor parking from
   Decision 12 §12 / Feature 10 would upgrade this to Depends(RegisteredFd)
   but that is wired at a different valtron layer — same gap for both h1
   and h2 handlers)
```

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
| `SimpleIncomingRequestHeader` | foundation_netio | `http2/types.rs` |
| `H2IncomingFrame` | foundation_netio | `http2/types.rs` |
| `H2Frame` | foundation_netio | `http2/types.rs` |
| `H2Conn` | foundation_netio | `http2/conn.rs` |
| `H2ConnectionHandler` | foundation_http | `native/server/h2_connection.rs` |
| h2c detect in HttpServer | foundation_http | `native/server/mod.rs` |
| `dispatch_h2()` | foundation_connectrpc | `router/dispatch.rs` |
| `h2_echo` example | foundation_connectrpc | `examples/h2_echo/main.rs` |
| `H2Transport` client streaming | foundation_connectrpc | `transport/h2.rs` |

### Acceptance criteria

- Unary: `Router::unary()` over h2c round-trips ✅
- Server-stream: handler produces stream, chunks delivered as h2 DATA frames ✅
- Client-stream: client sends DATA frames, handler receives them via `H2IncomingFrame` stream ✅
- Bidi-stream: interleaved send/recv via `futures::join!` ✅
- All 87 F29 tests pass, zero warnings ✅
