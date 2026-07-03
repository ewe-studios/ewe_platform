# Decision 11: Transport Seam & Streaming Handler Model

## Context

The single most important requirement for this port is that **the RPC protocol layer
(Connect / gRPC / gRPC-Web) is decoupled from the HTTP transport** so the same
handler and protocol code runs unchanged over HTTP/1.1, HTTP/2, HTTP/3, QUIC, and
iroh. connect-go achieves this by layering the protocol on top of two seams:

- `http.Handler` / `http.RoundTripper` — the request/response exchange
- `io.Pipe` + goroutines — concurrent send/receive on a streaming body

and a per-call connection object (`handlerConnCloser` server-side,
`streamingClientConn` client-side) that the protocol writes to without knowing the
transport.

The earlier decisions left three things unresolved or wrong for this goal:

1. **Q8 (push vs pull)** — undecided. Decisions 04/08 chose a **pull** model where the
   handler *returns* an output iterator. That shape cannot interleave reads and writes
   in one handler body, so full-duplex bidi over HTTP/2 would require an API change
   (T5). It also leaks a "single straight-line worker" assumption (04 Q5) that the
   runtime does not actually impose — **valtron is a multi-threaded worker-pool
   executor**, so reader/writer can run as concurrent tasks.
2. **The seam itself is named but never defined** — `ProtocolHandler::new_conn ->
   Box<dyn HandlerConn>` and `ProtocolClient::new_conn -> Box<dyn ClientConn>` (Decision
   05) reference `HandlerConn` / `ClientConn` that no document specifies.
3. **The client `Transport` trait is unary-only** — `round_trip(request) -> response`
   (Decision 07) cannot express streaming, so the streaming client types in Decision 07
   reach around it, contradicting Decision 05's `ClientConn` approach (T7, Q6, B6).

This decision resolves Q8, defines the seam, and replaces the unary-only client
transport. Decisions 04, 07, and 08 were updated in place to this model.

## Decision

### Core primitive: `FramePipe` — the bounded frame pipe (feature 00-F4)

Streaming is modeled as two **bounded** pipes of the named primitive `FramePipe`:

```rust
/// What a pipe carries: codec-level payloads plus a NORMALIZED end-of-stream — never raw
/// envelope bytes (bare `Bytes` could not carry the envelope flags, so Connect 0x02 /
/// gRPC-Web 0x80 / h2 trailing HEADERS would be indistinguishable from messages).
pub enum Frame {
    /// One codec-encoded message (de-enveloped + decompressed on receive;
    /// not yet enveloped on send).
    Message(Bytes),
    /// The protocol's end-of-stream, normalized: Connect EndStreamResponse (flag 0x02),
    /// gRPC/gRPC-Web status + trailers (0x80 frame / trailing HEADERS) all decode to this.
    EndStream { error: Option<ErrorTrace<ConnectError>>, trailers: SimpleHeaders },
}

/// THE pipe primitive (Decision 00 L1b / 00-F4) is GENERIC over its item: `Pipe<T>` — a
/// bounded ConcurrentQueue<T> plus two waker stashes (producer/consumer) and both readiness
/// accessors (QueueReadiness / QueueVacancyReadiness). Async ends await (waker stashed on
/// the pipe, fired by the opposite end's push/pop); valtron-task ends return Depends.
/// Everywhere it appears it REPLACES the polling handoff of
/// `ConcurrentQueueStreamIterator` (max_turns/park_duration) — no spinning at any depth.
/// Instantiations: `FramePipe = Pipe<Frame>` (the seam), `Pipe<Bytes>` (the transport
/// byte pipes below, and the pushable client request body — Decision 12 §7).
pub struct Pipe<T> { /* … */ }
pub type FramePipe = Pipe<Frame>;   // halves: PipeSender<T> / PipeReceiver<T>
```

**Normative layering — who owns wire framing (fresh-review A3):** enveloping and
compression belong to the **transport-side reader/writer tasks**, and only there — they are
protocol-specific, and the conn/facade layers never touch wire framing:

- **Request pipe**: transport-reader task (producer) → handler (consumer). The reader
  de-envelopes + decompresses wire bytes and pushes `Frame`s; `ConnReceiver::receive`
  yields `Frame::Message` payloads (`EndStream` → `Ok(None)` + stored metadata); the typed
  `MessageSource<Req>` facade decodes each payload (codec) into `Req`.
- **Response pipe**: handler (producer) → transport-writer task (consumer). The typed
  `MessageSink<Res>` facade encodes `Res` (codec); `ConnSender::send` pushes
  `Frame::Message`, `ConnSender::close` pushes `Frame::EndStream`; the writer task
  envelopes, compresses, flushes per frame, and renders `EndStream` in its protocol's
  form (0x02 JSON envelope / 0x80 trailer frame / trailing HEADERS).

The **codec and the concrete type are confined to the typed facade**
(`MessageSink`/`MessageSource`), never the seam. The seam (`HandlerConn`/`ClientConn`,
below) and the bounded queue carry **codec-encoded frame bytes + metadata** — there is no
`dyn Any` anywhere in the path. The facade also hosts a per-procedure **message-middleware**
chain (see "Facade message-middleware" below).

Boundedness is mandatory — it is what provides backpressure (the analog of `io.Pipe`
blocking a fast writer when the reader is slow), so a streaming RPC does not buffer an
entire stream in memory. Default depth: **4 messages per pipe**, configurable via
client/server options (§Decided Details, pipe depth).

`MessageSink`/`MessageSource` are **internal adapters** over the two queues — not
user-facing (users write async fns/`Stream`s, Decision 04). They are the typed facade that
holds the codec:

```rust
/// INTERNAL. Producer facade over the (possibly interceptor-wrapped) conn / response pipe.
/// Owns the typed codec handle (`Arc<dyn CodecFor<T>>`, Decision 02) — the only place `T`
/// is encoded. ASYNC-CANONICAL: every frame-moving method is async; flow control is
/// AWAITED, never surfaced as an error (Decision 03 norm — `Full`/`Pending` are not errors).
struct MessageSink<T> { /* Box<dyn ConnSender> half + Arc<dyn CodecFor<T>> */ }
impl<T: Send + 'static> MessageSink<T> {
    async fn set_headers(&mut self, headers: SimpleHeaders) -> ConnectResult<()>;
    /// Encode + send one frame. On a full bounded pipe the future PARKS: its waker is
    /// stashed on the pipe and fired by the consumer's `pop` (Decision 00 L1b two-sided
    /// wake) — no `Err(Full)`, no bare-`Pending` re-poll.
    async fn send(&mut self, msg: T) -> ConnectResult<()>;
    async fn close(self, trailers: SimpleHeaders) -> ConnectResult<()>;
}

/// INTERNAL. Consumer facade over the conn / request pipe. Owns `Arc<dyn CodecFor<T>>` —
/// the only place `T` is decoded.
struct MessageSource<T> { /* Box<dyn ConnReceiver> half + Arc<dyn CodecFor<T>> */ }
impl<T: Send + 'static> MessageSource<T> {
    /// Next request message, owned-decoded (or an `OwnedView<…>` for the zero-copy variant —
    /// see "Zero-copy" below). `Ok(None)` at end of stream. ASYNC: an empty pipe parks the
    /// caller via the pipe's stashed waker / `Depends(QueueReadiness)` (Decision 00).
    async fn receive(&mut self) -> ConnectResult<Option<T>>;
    fn headers(&self) -> &SimpleHeaders;
}
```

> Types are concrete structs, **not** `Box<dyn StreamIterator<T>>` — the real
> `StreamIterator` is a supertrait of `Iterator` with associated types `D`/`P` and can't be
> `StreamIterator<T>` (closes RS1). End-of-stream is `Ok(None)` / `Iterator::next() == None`,
> never `Stream::Init` (closes H9).

### Handler model: async, bridged onto the seam (resolves Q8, H1, H2, C2)

Users write **async functions / futures `Stream`s** (the signatures in Decision 04); they
never see `MessageSink`/`MessageSource`/`Stream<D,P>`/the queues. valtron's `from_future` /
`from_stream` turn the user's async into a `TaskIterator` that returns `TaskStatus`; **each
`.await` is the yield point** — async/await *is* the state machine the executor drives.

> **Depends on Decision 00 (valtron async readiness).** The "no spin-loop" guarantee is only
> true once Decision 00 Level 1 lands: today `from_future`/`from_stream` poll with a no-op
> waker and busy-re-poll on `Pending`. Decision 00 makes `FutureTask` return
> `Depends(QueueReadiness)` so an awaiting handler **parks** until its wake queue gets a
> token. Decision 00 is a hard prerequisite for this section.

There is **no blocking and no hand-written `Recv/Send` state machine** (that would be
valtron Anti-Pattern 3). The wiring:

```
request body ─[reader task]→ request_q ──(req-frame → Stream<Req> adapter)──▶ async handler
async handler ──(returns async Stream<Res>, consumed via from_stream)──▶ response_q ─[writer task]→ wire
```

- The request `Stream<Req>` the handler receives is built **through the conn** (S1 enabler):
  each item is `MessageSource<Req>::receive().await` → `ConnReceiver::receive().await` (the
  receiver half of the possibly interceptor-wrapped conn, Decision 04) → the request pipe.
  An empty pipe parks the handler future (stashed waker / `Depends(QueueReadiness)`,
  Decision 00), woken by the reader task's push. Interceptor wrappers therefore sit **in the
  frame path** — the adapter never bypasses the conn.
- Outputs flow `MessageSink<Res>::send().await` → conn `send` → **bounded** response pipe. A
  full pipe parks the producer via the vacancy wake (Decision 00 L1b: waker stashed on the
  pipe, fired by the writer task's `pop`) — never bare-`Pending` re-poll, never an error.
  FIFO ordering is the queue's (delivers backlog before the latest).
- The reader/writer ends stay plain **valtron tasks**; adapters convert in both directions —
  a valtron task/queue is surfaced as a futures `Stream` where a stream shape fits, and the
  handler's async `Stream` is driven as a task via `from_stream`.
- `Stream<D,P>` is only the **boundary** form (`From<TaskStatus>`), surfaced to sync
  collectors (`collect_one`/`execute`); it never leaks to handler authors unless they
  deliberately drop a level.

**Duplex = transport scheduling of the two queues**, not the handler API: HTTP/2/3 run the
reader and writer concurrently (full-duplex); HTTP/1.1 runs the writer after the request
drains (half-duplex). **Bidi needs full-duplex, so it is rejected on HTTP/1.1 with `505`**;
client- and server-streaming are half-duplex and allowed there (matches connect-go's
`StreamType & Bidi` check). Enforced by capability matching (below).

**Threading:** pool futures are `Send + 'static`; valtron's local/non-send path and
`SendWrapper` (for single-threaded/wasm) cover `!Send` state — never a blocker.

### Concurrency = cooperative valtron tasks; duplex = scheduling, not API

A streaming RPC is up to three cooperating valtron tasks bridged by the two queues:

```
request body ─▶ [reader task] ─▶ request_q ─▶ [handler task (async, bridged)] ─▶ response_q ─▶ [writer task] ─▶ response body
```

None of them block: each does one step per poll and returns a `TaskStatus`
(`Ready`/`Pending`/`Wait`/`Delayed`/`Depends`/`Ignore`); the executor schedules. The handler
task is the user's async future driven by `from_future`/`from_stream` — its `.await` points
are the yield points. Backpressure is the bounded pipe parking its producer (vacancy wake /
`Depends(QueueVacancyReadiness)`, Decision 00 L1b); ordering is the queue's FIFO.

Duplex is decided entirely by how the transport schedules reader/writer, never by the
handler API:

| Transport | Reader/Writer scheduling | Streaming kinds |
|---|---|---|
| HTTP/1.1 | Reader drains request, then writer runs | unary / client-stream / server-stream (half-duplex); **bidi → 505** |
| HTTP/2, HTTP/3, QUIC, iroh | Reader and writer run concurrently | all, incl. full-duplex bidi |

Closes T5 (full-duplex needs no handler change) and the bounded-channel question.

### The seam: `HandlerConn` (server) and `ClientConn` (client)

These are the byte-stream objects each protocol writes to without knowing the transport.
They are what `ProtocolHandler::new_conn` / `ProtocolClient::new_conn` (Decision 05)
return. The framework wires a `HandlerConn`'s pipes to the reader/writer tasks above.

Both seams are **async-canonical** (every frame-moving method returns a dyn-safe
`BoxFuture`) and — fresh-review A2 — **split into independently owned halves**, because the
adopted connect-go contract ("the read side may be called concurrently with the write side;
neither side concurrently with itself") is impossible over one `Box` with `&mut` methods:
`MessageSource` and `MessageSink` each need their own half. Interceptors wrap the **unsplit**
conn (Decision 04); a wrapper's `split` wraps the inner halves, so frame-path interception
survives the split. There is deliberately **no "not ready" return value**: waiting is
expressed by the future parking (empty pipe → consumer waker; full pipe → vacancy waker,
Decision 00 L1b), and flow control never appears in `ConnectResult` (Decision 03 norm).

```rust
/// Server-side per-call seam. Implemented once per (protocol × transport), produced by
/// ProtocolHandler::new_conn. The seam speaks only in **frames (bytes)**, headers, and
/// trailers — never typed messages or `dyn Any`.
pub trait HandlerConn: Send {
    fn spec(&self) -> &Spec;
    fn peer(&self) -> &Peer;
    fn request_headers(&self) -> &SimpleHeaders;
    /// Split into independently-owned halves: the receiver feeds `MessageSource<Req>`, the
    /// sender feeds `MessageSink<Res>`; read and write then proceed concurrently on
    /// full-duplex transports (each half used by exactly one task at a time).
    fn split(self: Box<Self>) -> (Box<dyn ConnReceiver>, Box<dyn ConnSender>);
}

pub trait ConnReceiver: Send {
    /// Next request **frame** as `Bytes` (codec-encoded; de-enveloped + decompressed by the
    /// reader task — see §Layering), or `None` at end of stream. `Bytes` (not `Vec<u8>`) so
    /// the facade can build a zero-copy `OwnedView`/`RecordBatch` (S5). Parks while the
    /// request pipe is empty; cancellation also wakes it (§Cancellation).
    fn receive(&mut self) -> BoxFuture<'_, ConnectResult<Option<Bytes>>>;
}

pub trait ConnSender: Send {
    /// Send response headers (idempotent until the first frame/flush).
    fn send_headers(&mut self, headers: SimpleHeaders) -> BoxFuture<'_, ConnectResult<()>>;
    /// Hand one already-encoded response frame to the response pipe — the WRITER TASK
    /// envelopes/compresses (§Layering); this method never touches wire framing.
    /// Parks while the pipe is full (vacancy wake, Decision 00 L1b).
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, ConnectResult<()>>;
    /// Finish: emits the protocol's end-of-stream (Connect end-stream envelope / gRPC-Web
    /// trailer frame / HTTP/2 trailing HEADERS — rendered by the writer task), then closes.
    /// Consumes the box → `'static`.
    fn close(self: Box<Self>, error: Option<ErrorTrace<ConnectError>>, trailers: SimpleHeaders)
        -> BoxFuture<'static, ConnectResult<()>>;
}

/// Client-side per-call seam, produced by ProtocolClient::new_conn over a Transport stream.
pub trait ClientConn: Send {
    fn spec(&self) -> &Spec;
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders;   // before split / first send
    fn split(self: Box<Self>) -> (Box<dyn ClientSender>, Box<dyn ClientReceiver>);
}

pub trait ClientSender: Send {
    /// Outbound request headers — valid until the first `send`/`flush_headers` (the
    /// protocol renders the head at first write). After `split` this is THE header
    /// handle; the unsplit accessor exists for pre-split convenience only.
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    /// Header-only send (C5): render the head now, before any body frame.
    fn flush_headers(&mut self) -> BoxFuture<'_, ConnectResult<()>>;
    /// Parks on request-pipe vacancy (Decision 00 L1b).
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, ConnectResult<()>>;
    /// Half-close the request direction; the receiver half stays live.
    fn close_send(self: Box<Self>) -> BoxFuture<'static, ConnectResult<()>>;
}

pub trait ClientReceiver: Send {
    /// Encoded frame (`Bytes`); parks while the pipe is empty. END-OF-STREAM SEMANTICS
    /// (normative, connect-go parity): the receiver consumes the protocol's end-of-stream
    /// internally and stores its trailers; if that end-of-stream carries a terminal error
    /// (Connect `EndStreamResponse.error`, `grpc-status != 0`), `receive` returns
    /// `Err(trace)` — NEVER a clean `Ok(None)`. `Ok(None)` means clean EOS only. Trailers
    /// are available via `response_trailers` after either outcome.
    fn receive(&mut self) -> BoxFuture<'_, ConnectResult<Option<Bytes>>>;
    /// Resolving the head may wait on the network (`TransportStream.response`); facades
    /// that awaited the head at construction (server-streaming) expose a sync accessor.
    fn response_headers(&mut self) -> BoxFuture<'_, ConnectResult<&SimpleHeaders>>;
    /// Available after EOS (populated from the consumed end-of-stream).
    fn response_trailers(&mut self) -> BoxFuture<'_, ConnectResult<&SimpleHeaders>>;
}
```

The split is also what makes concurrent client `send`/`receive` **deadlock-free on bounded
pipes**: with one owned handle a caller could only alternate the two, so a server that
fills its response pipe while not draining requests would deadlock a client parked in
`send` — connect-go supports concurrent Send/Receive for exactly this reason. `BidiStream`
exposes the halves (Decision 07).

### Layering: typed facade vs byte seam

`MessageSink<T>` / `MessageSource<T>` are the **typed, per-RPC facade** the handler sees.
They own the `Arc<dyn CodecFor<T>>` (Decision 02 — resolved by name from the per-procedure
`ProcedureCodecs` table at dispatch) and are the only place the concrete `Req`/`Res` is known:

- `MessageSink<Res>::send(res).await` → run the facade's **message-middleware** → `codec.marshal(&res)`
  (via `dyn CodecFor<Res>`) → `ConnSender::send(frame).await` (parks on a full pipe, Decision 00 L1b).
- `MessageSource<Req>::receive().await` → `ConnReceiver::receive().await` (a `Bytes` frame) → run middleware
  → `codec.unmarshal(frame)` (or the concrete codec's inherent `unmarshal_owned_view` for the
  zero-copy variant — statically dispatched in the generated facade, Decision 02)
  → `Some(req)`. Typed via `CodecFor` — no `dyn Any`, no `Default`+`&mut` erasure.

The seam (`HandlerConn`/`ClientConn`) carries **only encoded frames + metadata** (no
`dyn Any`). **Zero-copy (decided, supported):** the facade can decode into an owned message
*or* hand the handler a `buffa::OwnedView<V>` — a self-referential `Bytes`+view bundle that
is **`'static + Send + Sync`** ("suitable for async and RPC frameworks", buffa DESIGN.md),
so it survives `.await` and crosses the pool. For the **Arrow** codec the message *is* an
Arc-backed `RecordBatch`, inherently `'static`/`Send`/zero-copy with no per-field decode.
A *naked* borrowed `MessageView<'a>` is the only thing that does **not** work under async
(can't be held across `.await`); `OwnedView`/`RecordBatch` are the supported forms. (Updates
RS2 in Decision 02: zero-copy is supported via owning views, not impossible.) **View-typed
procedures are single-codec by construction** (fresh-review A5; Decisions 02/10):
`OwnedView<V>` satisfies no `CodecFor` family bound, so no generic `ProcedureCodecs` table
exists for it — codegen emits the zero-copy variant with one fixed concrete codec (proto →
`OwnedView`, arrow → `RecordBatch`), statically dispatched; any other content-type → 415.

#### Facade message-middleware

Because the facade knows the codec + type, it hosts an ordered, per-procedure middleware
chain for content concerns (validation, redaction, defaulting, transforms). Each middleware
takes the **frame bytes + metadata**, and the facade can decode once and lend the typed
message to any middleware that wants it (decoded a single time, shared). Ordering:

```
send:     Res ─[typed middleware: &Res]→ codec.marshal ─[byte middleware: &frame + meta]→ seam → (compress, envelope) → wire
receive:  wire → (de-envelope, decompress) → seam ─[byte middleware]→ codec.unmarshal ─[typed middleware: &mut Req]→ handler
```

Byte middleware see the **uncompressed, codec-encoded** frame (compression stays the
outermost byte step — Decision 06). This is the per-procedure counterpart to the
cross-cutting seam interceptors in Decision 04, and replaces the `dyn Any`-message
interceptor boundary that does not work in Rust (no message reflection).

### Streaming-capable client `Transport` (replaces unary-only `round_trip`)

```rust
/// HTTP transport abstraction. One implementation per transport: foundation_netio
/// HTTP/1.1, http2/, http3/, iroh/, WASM Fetch.
pub trait Transport: Send + Sync + 'static {
    /// Report what this transport can do, so the client can reject incompatible
    /// protocol+stream-type combinations before sending (see capability matching).
    fn capabilities(&self) -> TransportCapabilities;

    /// Open a streaming exchange: returns sinks/sources for the request and response
    /// bodies. Unary is the degenerate case (send one, close, receive one).
    /// ASYNC (`BoxFuture` keeps the trait dyn-safe): connecting / pool checkout does I/O
    /// and must park, not block a worker (async-canonical decision).
    /// `RequestDescriptor` is netio's existing head-only request (proto/url/headers/method);
    /// the body flows through the returned `TransportStream` (not in the head).
    fn open(&self, request: RequestDescriptor)
        -> BoxFuture<'static, Result<TransportStream, TransportError>>;
}

/// A live transport exchange — BYTE-LEVEL. The transport is protocol-agnostic: it moves
/// wire body bytes and knows NOTHING of envelopes, compression, or protocols (it could not
/// produce `Frame`s — normalization is protocol-specific). `ByteSink`/`ByteSource` are the
/// `Pipe<Bytes>` instantiation of the 00-F4 primitive.
pub struct TransportStream {
    pub send_body: ByteSink,                 // wire request-body bytes out
    // Response head = netio's `SimpleResponse<()>` (status + headers, no body).
    // AWAITABLE (async-canonical): resolving the head may wait on the network and must
    // park, not block a worker.
    pub response: BoxFuture<'static, Result<SimpleResponse<()>, TransportError>>,
    pub recv_body: ByteSource,               // wire response-body bytes in
}

// WHO OWNS ENVELOPING (closes the wiring; fresh-review-2 #1): the PROTOCOL layer.
// Client: `ProtocolClient::new_conn(spec, headers, transport_stream)` constructs the
// per-call reader/writer TASK PAIR (de-envelope/decompress ⇄ envelope/compress, consuming
// the negotiated compression) plus the `ClientConn` over the internal `FramePipe`s between
// them, and returns all three (`ClientExchange`); the framework spawns the tasks on
// valtron. Server mirror: after protocol match the dispatcher calls
// `ProtocolHandler::new_conn(request, byte_in, byte_out, compression)` which returns
// `HandlerExchange { conn, reader_task, writer_task }`; the dispatcher spawns the tasks
// (Decision 05 signatures). Layering, end to end:
//   transport ⇄ BYTES (`ByteSink`/`ByteSource`) ⇄ protocol reader/writer tasks
//             ⇄ `FramePipe`s (`Frame`) ⇄ conn halves ⇄ typed facade ⇄ handler

/// Transport-layer failure (previously referenced but undefined — fresh-review A10).
/// The client maps it via `From<TransportError> for ConnectError`:
///   Connect(_) | Reset | Io(_)  ⇒ unavailable      (retryable-class, connect-go parity)
///   Timeout                     ⇒ deadline_exceeded
///   Protocol(_)                 ⇒ internal
///   Canceled                    ⇒ canceled
/// (Server-inferred codes for non-Connect HTTP responses use Decision 03's reverse table;
/// this enum covers failures where no response exists at all.)
pub enum TransportError {
    Connect(BoxedError),      // dial / TLS / pool-checkout failure
    Timeout,                  // connect or response-head deadline elapsed
    Reset,                    // RST_STREAM / QUIC reset / connection closed mid-exchange
    Protocol(String),         // malformed wire data below the RPC protocol layer
    Canceled,                 // local cancellation (CancelSignal fired)
    Io(std::io::Error),
}

/// Convenience: unary round-trip is built on `open` for transports/callers that prefer it.
/// Uses the existing netio client types — `PreparedRequest` in, `SimpleResponse<SendSafeBody>`
/// out (no fictional `SimpleOutgoingRequest`/`SimpleIncomingResponse`; verified — Decision 07).
impl dyn Transport {
    pub async fn round_trip(&self, req: PreparedRequest)
        -> Result<SimpleResponse<SendSafeBody>, TransportError> { /* open + send + close + read */ }
}
```

This reconciles Decision 05 (`ClientConn`) and Decision 07 (`round_trip`): `round_trip`
becomes a unary convenience over `open`. Closes T7, Q6, B6.

### Capability matching (makes "any protocol on any transport" precise)

A protocol declares what it needs; a transport declares what it offers. The router
(server) and client check `requirements ⊆ capabilities` and reject mismatches uniformly,
replacing the scattered special cases (H6, T4, T12, the WASM-Fetch limits).

> **Remodeled (fresh-review A1).** The earlier single lattice (`min_duplex: Half` for every
> non-bidi kind, `TrailerSupport` as one axis) rejected its own examples — unary on WASM
> Fetch computed as incompatible. Two modeling fixes: **requirements are per
> (protocol × stream type)**, and the axes are the *actual* transport abilities — "can I
> stream the request body" is not duplexing, and Connect's `Trailer-` headers / gRPC-Web's
> in-body `0x80` frame need nothing from a transport beyond a body, so only **real HTTP/2
> trailing HEADERS** is a transport capability.

```rust
pub struct TransportCapabilities {
    /// Can the transport keep pushing request-body frames after sending has begun?
    /// HTTP/1.1 chunked = yes; HTTP/2/3 = yes; WebSocket = yes; WASM Fetch = NO.
    pub request_streaming: bool,
    /// Can reader and writer run concurrently on one exchange (full duplex)?
    /// HTTP/2/3, WebSocket = yes; HTTP/1.1 POST, Fetch = no.
    pub full_duplex: bool,
    /// Real HTTP/2 trailing HEADERS — the only trailer form that is a TRANSPORT capability.
    pub h2_trailers: bool,
    pub http_versions: &'static [Proto],   // e.g. [HTTP11], [HTTP20, HTTP30]
    pub multiplexed: bool,
}

/// Computed per (protocol × stream type) — never a single per-protocol lattice.
pub struct CallRequirements {
    pub request_streaming: bool,   // ClientStream | BidiStream
    pub full_duplex: bool,         // BidiStream only
    pub h2_trailers: bool,         // gRPC only (its status lives in trailing HEADERS)
    pub min_http_version: Proto,   // gRPC ⇒ HTTP20; Connect / gRPC-Web ⇒ HTTP11
}

pub fn requirements(protocol: ProtocolKind, stream_type: StreamType) -> CallRequirements;

/// `Ok` iff every required ability is offered.
pub fn check_compatible(req: &CallRequirements, cap: &TransportCapabilities)
    -> ConnectResult<()>;
```

Examples this enforces with one rule: gRPC on HTTP/1.1 → rejected (`h2_trailers` +
version floor); bidi on HTTP/1.1 → rejected with `505` (`full_duplex`, H6); Connect/gRPC-Web
bidi on HTTP/2 → allowed; **unary/server-streaming on WASM Fetch → allowed** (neither needs
`request_streaming`), client-stream/bidi on Fetch → rejected — exactly the §Decided Details Fetch matrix.

### Cancellation flows through the seam

The transport pushes cancellation (connection close, `RST_STREAM`, QUIC stream reset) by
firing the call's `CancelSignal` — it holds a clone of the same signal every `Ctx` clone of
that call shares (Decision 04 §Ctx rules). `MessageSource::receive` and `MessageSink::send`
observe it and return a `Canceled` / `DeadlineExceeded` error (as
`ErrorTrace<ConnectError>`, the ConnectResult norm) so the handler unwinds. **Wake wiring
(normative):** a parked `receive`/`send` must actually wake on cancel — the async path's
`FramePipe` futures poll the pipe *and* the call's `CancelSignal` (the signal fires stashed
wakers on `cancel()`); the task path parks on
`Depends(AnyReadiness([pipe_readiness, cancel_signal]))` (Decision 00 L1b) — never on the
pipe readiness alone. This is the only cancellation mechanism handlers see, regardless of
transport (closes Decision 04 Q4).

## Consequences

- **Handler signatures are transport-invariant.** The same unary/server/client/bidi
  handler runs over HTTP/1.1, HTTP/2, HTTP/3, QUIC, and iroh with no change. Duplex is a
  scheduling property of the transport, not part of the API.
- **The seam is concrete.** `HandlerConn`/`ClientConn` are defined; `MessageSink`/
  `MessageSource` are their typed facade; `Transport::open` carries them on the client.
- **Backpressure is built in** via bounded pipes — no full-stream buffering.
- **Capability matching** turns "any protocol on any transport" into an enforced
  predicate rather than an aspiration; gRPC's HTTP/2 floor is expressed as data.
- **Closes/answers:** Q8, H1, H2, H3, T5, T7, Q6, B6, RS1, H9, and the missing
  `HandlerConn`/`ClientConn` definitions; reconciles Decisions 05 and 07 (both updated
  in place — no superseded text remains in 04/07/08).
- **Foundation prerequisites now have a defined approach (Decision 12).** Per-message
  flush (B1) uses the connection writer the `Serve` handler already owns; incremental
  writing (B2) and trailers (B3) come from decomposing `Http11ResponseIterator` into
  per-part iterators (status / headers / body-chunk / trailers). The writer task in this
  decision emits one body-chunk part and flushes per message; `close` emits the trailers
  part. HTTP/2 (B5) is the owned `http2/` module. This decision defines the API those
  foundation changes must satisfy.

## Decided Details

1. **Pipe depth default — resolved: message-count bound, default 4, configurable.**
   The pipes are `FramePipe`s over `ConcurrentQueue::bounded(n)` — valtron bounds by count
   only. Even with the L1b waker hooks (no polling handoff), a depth-1 rendezvous pipe would
   pay a park/wake round-trip on every single message. **Default depth 4 per pipe**,
   overridable via client/server options: it amortizes wake round-trips on bursty producers
   while keeping in-flight frames tightly bounded. A byte-budget bound was rejected — it needs custom
   accounting valtron doesn't have, and `read_max_bytes` defaults to unlimited (Decision 06
   P17/Q7) so it would need its own default anyway; revisit only if profiling demands it.
   Worst-case in-flight memory ≈ depth × max message size, per direction (unbounded only if
   the operator leaves `read_max_bytes`/`send_max_bytes` at the connect-go-parity unlimited
   default — same posture as connect-go, orthogonal to depth).
2. **Half-duplex detection point — resolved.** On HTTP/1.1 the writer task starts only after
   the reader signals request-complete (client/server-streaming); bidi is rejected up front
   via capability matching (505). That is the exact gating point (connect-go parity).
3. **WASM Fetch capabilities — decided.** Fetch cannot stream a request body or read HTTP/2
   trailing HEADERS, so its `TransportCapabilities` reports `request_streaming: false`,
   `full_duplex: false`, `h2_trailers: false`; `check_compatible` then allows unary +
   server-streaming over Connect/gRPC-Web (neither needs request streaming) and rejects
   client-stream/bidi — matching the Decision 07 baseline-transport matrix by computation,
   not by special case. Bridge-surface confirmation is a foundation_wasm implementation
   detail, not a design question.
