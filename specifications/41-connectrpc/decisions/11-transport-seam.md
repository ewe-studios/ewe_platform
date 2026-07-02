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
transport. It supersedes the streaming handler signatures in Decisions 04 and 08 and
the client streaming types in Decision 07.

## Decision

### Core primitive: the bounded message pipe

Streaming is modeled as two **bounded** pipes built on valtron's
`ConcurrentQueueStreamIterator`. Each pipe has a producer half and a consumer half that
live on opposite sides of the protocol↔transport boundary:

- **Request pipe**: transport-reader task (producer) → handler (consumer). The reader
  de-envelopes/decompresses the request body into **codec-encoded frames**; the typed
  `MessageSource<Req>` facade decodes each frame (codec) into `Req`.
- **Response pipe**: handler (producer) → transport-writer task (consumer). The typed
  `MessageSink<Res>` facade encodes `Res` (codec) into a frame; the writer envelopes,
  compresses, and flushes per frame.

The **codec and the concrete type are confined to the typed facade**
(`MessageSink`/`MessageSource`), never the seam. The seam (`HandlerConn`/`ClientConn`,
below) and the bounded queue carry **codec-encoded frame bytes + metadata** — there is no
`dyn Any` anywhere in the path. The facade also hosts a per-procedure **message-middleware**
chain (see "Facade message-middleware" below).

Boundedness is mandatory — it is what provides backpressure (the analog of `io.Pipe`
blocking a fast writer when the reader is slow), so a streaming RPC does not buffer an
entire stream in memory. Default depth: **4 messages per pipe**, configurable via
client/server options (see Open Question 1, resolved).

`MessageSink`/`MessageSource` are **internal adapters** over the two queues — not
user-facing (users write async fns/`Stream`s, Decision 04). They are the typed facade that
holds the codec:

```rust
/// INTERNAL. Producer adapter over the response queue.
struct MessageSink<T> { /* producer onto Arc<ConcurrentQueue<Stream<Frame, ()>>> */ }
impl<T: Send + 'static> MessageSink<T> {
    fn set_headers(&self, headers: SimpleHeaders) -> Result<(), ConnectError>;
    /// Encode + push one frame. Returns `Err(Full)` when the bounded queue is full; the
    /// **bridging task** (not this call) translates that into `TaskStatus::Pending` so the
    /// executor reschedules — the `send` call itself never blocks or yields scheduler states.
    fn send(&self, msg: T) -> Result<(), ConnectError>;
    fn close(self, trailers: SimpleHeaders) -> Result<(), ConnectError>;
}

/// INTERNAL. Consumer adapter over the request queue (a `ConcurrentQueueStreamIterator`).
struct MessageSource<T> { /* consumer of Arc<ConcurrentQueue<Stream<Frame, ()>>> */ }
impl<T: Send + 'static> MessageSource<T> {
    /// Next request message, owned-decoded (or an `OwnedView<T>` for the zero-copy variant —
    /// see "Zero-copy" below). `Ok(None)` at end of stream.
    fn receive(&mut self) -> Result<Option<T>, ConnectError>;
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

- The request adapter exposes `request_q` as a futures `Stream<Req>` (S1 enabler): maps
  `Next(frame)`→decode→`Ready(Some(Req))`, `Ignore`/`Wait`→`Pending`, closed→`Ready(None)`;
  empty → the handler future parks via Decision 00's wake-queue `Depends(QueueReadiness)`,
  woken by the reader task's push (`task.rs::QueueReadiness`, `is_ready = !queue.is_empty()`).
- Outputs are pushed to the **bounded** response queue; full → the producing step returns
  `Pending`/`Wait` and valtron reschedules. FIFO ordering is the queue's (delivers backlog
  before the latest).
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
are the yield points. Backpressure is the bounded queue returning `Full` (surfaced as
`Pending`/`Wait`); ordering is the queue's FIFO.

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

```rust
/// Server-side per-call seam. Implemented once per (protocol × transport), produced by
/// ProtocolHandler::new_conn. The framework attaches reader/writer tasks; the seam speaks
/// only in **frames (bytes)**, headers, and trailers — never typed messages or `dyn Any`.
pub trait HandlerConn: Send {
    fn spec(&self) -> &Spec;
    fn peer(&self) -> &Peer;
    fn request_headers(&self) -> &SimpleHeaders;

    /// Next request **frame** as `Bytes` (codec-encoded, de-enveloped + decompressed), or
    /// `None` at end of stream. `Bytes` (not `Vec<u8>`) so the facade can build a zero-copy
    /// `OwnedView`/`RecordBatch` without a copy (S5; transport already yields `Bytes`).
    fn receive(&mut self) -> Result<Option<Bytes>, ConnectError>;

    /// Send response headers (idempotent until the first frame/flush).
    fn send_headers(&mut self, headers: SimpleHeaders) -> Result<(), ConnectError>;

    /// Envelope + flush one already-encoded response **frame** to the transport.
    fn send(&mut self, frame: Bytes) -> Result<(), ConnectError>;

    /// Finish: write trailers (Connect end-stream / gRPC-Web trailer frame /
    /// HTTP/2 trailing HEADERS) per protocol, then close.
    fn close(self: Box<Self>, error: Option<ConnectError>, trailers: SimpleHeaders)
        -> Result<(), ConnectError>;
}

/// Client-side per-call seam, produced by ProtocolClient::new_conn over a Transport stream.
pub trait ClientConn: Send {
    fn spec(&self) -> &Spec;
    fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    fn send(&mut self, frame: Bytes) -> Result<(), ConnectError>;       // already-encoded frame
    fn close_send(&mut self) -> Result<(), ConnectError>;
    fn receive(&mut self) -> Result<Option<Bytes>, ConnectError>;       // encoded frame (Bytes), None at EOS
    fn response_headers(&self) -> &SimpleHeaders;
    fn response_trailers(&self) -> Result<&SimpleHeaders, ConnectError>;
}
```

### Layering: typed facade vs byte seam

`MessageSink<T>` / `MessageSource<T>` are the **typed, per-RPC facade** the handler sees.
They own the `Arc<dyn Codec>` and are the only place the concrete `Req`/`Res` is known:

- `MessageSink<Res>::send(res)` → run the facade's **message-middleware** → `codec.marshal::<Res>(&res)`
  → push the frame into the bounded queue → `HandlerConn::send(frame)`.
- `MessageSource<Req>::receive()` → `HandlerConn::receive()?` (a `Bytes` frame) → run middleware
  → `codec.unmarshal::<Req>(frame)` (or `unmarshal_owned_view::<Req>` for the zero-copy variant)
  → `Some(req)`. Concrete codec + concrete type — no `dyn Any`, no `Default`+`&mut` erasure.

The seam (`HandlerConn`/`ClientConn`) carries **only encoded frames + metadata** (no
`dyn Any`). **Zero-copy (decided, supported):** the facade can decode into an owned message
*or* hand the handler a `buffa::OwnedView<V>` — a self-referential `Bytes`+view bundle that
is **`'static + Send + Sync`** ("suitable for async and RPC frameworks", buffa DESIGN.md),
so it survives `.await` and crosses the pool. For the **Arrow** codec the message *is* an
Arc-backed `RecordBatch`, inherently `'static`/`Send`/zero-copy with no per-field decode.
A *naked* borrowed `MessageView<'a>` is the only thing that does **not** work under async
(can't be held across `.await`); `OwnedView`/`RecordBatch` are the supported forms. (Updates
RS2 in Decision 02: zero-copy is supported via owning views, not impossible.)

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
    /// `RequestDescriptor` is netio's existing head-only request (proto/url/headers/method);
    /// the body flows through the returned `TransportStream` (not in the head).
    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError>;
}

/// A live transport exchange. The protocol's ClientConn drives these.
pub struct TransportStream {
    pub send_body: BodySink,                 // bounded; protocol writes envelope frames
    // response head = netio's `SimpleResponse<()>` (status + headers, no body)
    pub response: Box<dyn FnOnce() -> Result<SimpleResponse<()>, TransportError> + Send>,
    pub recv_body: BodySource,               // bounded; protocol reads envelope frames
}

/// Convenience: unary round-trip is built on `open` for transports/callers that prefer it.
/// Uses the existing netio client types — `PreparedRequest` in, `SimpleResponse<SendSafeBody>`
/// out (no fictional `SimpleOutgoingRequest`/`SimpleIncomingResponse`; verified — Decision 07).
impl dyn Transport {
    pub fn round_trip(&self, req: PreparedRequest)
        -> Result<SimpleResponse<SendSafeBody>, TransportError> { /* open + send + close + read */ }
}
```

This reconciles Decision 05 (`ClientConn`) and Decision 07 (`round_trip`): `round_trip`
becomes a unary convenience over `open`. Closes T7, Q6, B6.

### Capability matching (makes "any protocol on any transport" precise)

A protocol declares what it needs; a transport declares what it offers. The router
(server) and client check `requirements ⊆ capabilities` and reject mismatches uniformly,
replacing the scattered special cases (H6, T4, T12, the WASM-Fetch limits).

```rust
pub enum Duplex { None, Half, Full }
pub enum TrailerSupport { Header, InBody, Http2Trailers, None }

pub struct TransportCapabilities {
    pub max_duplex: Duplex,
    pub trailers: TrailerSupport,
    pub http_versions: &'static [Proto],   // e.g. [HTTP11], [HTTP20, HTTP30]
    pub multiplexed: bool,
}

pub struct ProtocolRequirements {
    pub min_duplex: Duplex,                 // Bidi ⇒ Full; others ⇒ Half
    pub trailers: TrailerSupport,           // gRPC ⇒ Http2Trailers; gRPC-Web ⇒ InBody; Connect ⇒ Header
    pub min_http_version: Proto,            // gRPC ⇒ HTTP20
}

/// `Ok` iff the transport can carry this protocol at this stream type.
pub fn check_compatible(req: &ProtocolRequirements, cap: &TransportCapabilities)
    -> Result<(), ConnectError>;
```

Examples this enforces with one rule: gRPC on HTTP/1.1 → rejected (needs HTTP/2 +
HTTP/2 trailers); bidi on HTTP/1.1 → rejected with `505` (H6); Connect/gRPC-Web bidi on
HTTP/2 → allowed.

### Cancellation flows through the seam

The transport pushes cancellation (connection close, `RST_STREAM`, QUIC stream reset)
into `RequestContext.cancel()`. `MessageSource::receive` and `MessageSink::send` observe
`RequestContext` and return `ConnectError { code: Canceled | DeadlineExceeded }` so the
handler unwinds. This is the only cancellation mechanism handlers see, regardless of
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
  `HandlerConn`/`ClientConn` definitions; reconciles Decisions 05 and 07.
- **Supersedes:** streaming handler signatures in Decision 04, the erased streaming
  handler traits in Decision 08, and the client streaming types in Decision 07. Those
  docs should be updated to reference this seam.
- **Foundation prerequisites now have a defined approach (Decision 12).** Per-message
  flush (B1) uses the connection writer the `Serve` handler already owns; incremental
  writing (B2) and trailers (B3) come from decomposing `Http11ResponseIterator` into
  per-part iterators (status / headers / body-chunk / trailers). The writer task in this
  decision emits one body-chunk part and flushes per message; `close` emits the trailers
  part. HTTP/2 (B5) is the owned `http2/` module. This decision defines the API those
  foundation changes must satisfy.

## Open Questions

1. **Pipe depth default — resolved: message-count bound, default 4, configurable.**
   The pipes are `ConcurrentQueue::bounded(n)` — valtron bounds by count only, and its
   `ConcurrentQueueStreamIterator` is polling-based (`max_turns` + `park_duration`), so a
   depth-1 rendezvous pipe would park-stall on every message. **Default depth 4 per pipe**,
   overridable via client/server options: it amortizes the polling handoff while keeping
   in-flight frames tightly bounded. A byte-budget bound was rejected — it needs custom
   accounting valtron doesn't have, and `read_max_bytes` defaults to unlimited (Decision 06
   P17/Q7) so it would need its own default anyway; revisit only if profiling demands it.
   Worst-case in-flight memory ≈ depth × max message size, per direction (unbounded only if
   the operator leaves `read_max_bytes`/`send_max_bytes` at the connect-go-parity unlimited
   default — same posture as connect-go, orthogonal to depth).
2. **Half-duplex detection point — resolved.** On HTTP/1.1 the writer task starts only after
   the reader signals request-complete (client/server-streaming); bidi is rejected up front
   via capability matching (505). That is the exact gating point (connect-go parity).
3. **Frame `Bytes` plumbing — mostly resolved; one open sub-item.** The seam carries
   codec-encoded frames the transport hands up as `Bytes` (`quinn-proto`/h2); zero-copy builds
   a `buffa::OwnedView<V>` or Arc-backed `RecordBatch`, each holding its own Arc'd bytes (no
   reuse decision needed). **Still open:** whether the **owned-decode** path reuses a buffer
   pool — tune during implementation.
4. **WASM Fetch capabilities — resolved.** Fetch cannot do request-body streaming or trailers,
   so its `TransportCapabilities` reports `Duplex::None` / `TrailerSupport::None`; the client
   restricts WASM to unary + server-streaming over Connect/gRPC-Web (matches the Decision 07
   baseline-transport matrix). Bridge-surface confirmation is a foundation_wasm implementation
   detail, not a design question.
