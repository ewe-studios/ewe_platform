# Decision 01: Transport Layer & Runtime Model

## Context

ConnectRPC is an HTTP-based RPC framework supporting three wire protocols (Connect, gRPC, gRPC-Web) over HTTP/1.1 and HTTP/2. The reference connect-go implementation builds directly on Go's `net/http`, which provides a unified HTTP/1.1+HTTP/2 server and client with goroutine-per-request concurrency.

Our platform uses:
- **foundation_http** — Connection-owned, worker-pooled HTTP server with synchronous handler dispatch
- **foundation_netio** — Custom HTTP types (`SimpleIncomingRequest`, `SimpleOutgoingResponse`, `SimpleMethod`, `SimpleHeaders`, `SimpleBody`/`SendSafeBody`), TCP/TLS, WebSocket, SSE
- **foundation_core** — Valtron: a progress-driven async model using `Stream<D, P>` enum (`Init`/`Ignore`/`Delayed`/`Pending`/`Next`/`Wait`/`Spread`) with `ConcurrentQueueStreamIterator`, single/multi executor pools, WASM-compatible

The connect-go implementation assumes:
- Full HTTP/2 support (required for gRPC protocol, bidi streaming)
- Goroutine-based concurrency (reader/writer run concurrently per stream)
- `http.Handler` interface for server integration
- `http.Client` / `http.RoundTripper` for client transport
- `io.Pipe` for streaming request/response bodies

## Decision

Port the connect-go architecture onto foundation types with the following mapping:

### Server Side

| connect-go concept | foundation_connectrpc equivalent |
|---|---|
| `http.Handler` | `ConnectHandler` trait implementing foundation_http's handler model |
| `http.Request` | `SimpleIncomingRequest` from foundation_netio |
| `http.ResponseWriter` | `SimpleOutgoingResponse` from foundation_netio |
| `http.Header` | `SimpleHeaders` from foundation_netio |
| Request body (`io.Reader`) | `SimpleBody::Stream` / `SendSafeBody::Stream` iterators |
| Response body (`io.Writer`) | `SendSafeBody::ChunkedStream` for streaming, `SendSafeBody::Bytes` for unary |
| `context.Context` | `RequestContext` struct carrying deadline, headers, extensions, cancellation flag |
| Goroutine concurrency | Valtron executor tasks for concurrent read/write on streams |

### Client Side

| connect-go concept | foundation_connectrpc equivalent |
|---|---|
| `http.Client` | `ConnectClient` using foundation_netio's HTTP client |
| `http.RoundTripper` | `Transport` trait abstracting HTTP request/response exchange |
| `io.Pipe` (streaming body) | Foundation iterator-based body streams |
| Response reading | Iterator-based pull model |

### Streaming Body Model

connect-go uses Go's `io.Pipe` to create a writer that feeds a reader concurrently. Foundation uses iterator-based streaming:

- **Request body reading**: the reader task accumulates wire bytes in its task-owned buffer and de-envelopes via the shared `IncrementalDecoder` (Decision 12 §11), yielding zero-copy `Bytes` frames (Decisions 05/06/11) — the read path is `Bytes` end-to-end.
- **Response body writing (streaming)**: the handler is an async `Stream` of responses (Decision 04); the writer task envelopes/compresses each message and flushes per frame via the per-part `Http11` writer (Decision 12). The valtron `TaskStatus`/`Stream`/queue machinery is internal (Decision 11) — the handler does not return a valtron iterator.
- **Bidi streaming**: concurrent read (request `Stream<Req>`) and write (response `Stream<Res>`), each driven by an independent valtron task (Decision 11). **On HTTP/1.1 bidi is rejected with `505`** (no full-duplex); it is full-duplex only on HTTP/2/3 (or via the WebSocket transport, Decision 13). Client/server-streaming remain half-duplex on HTTP/1.1.

### HTTP/2 Support

gRPC protocol requires HTTP/2. Current foundation_netio supports HTTP/1.1 natively. For HTTP/2:

**Option A — h2 crate behind feature gate**: Add optional `h2` dependency for native HTTP/2 server/client. The protocol layer doesn't change; only the transport framing changes.

**Option B — Delegate HTTP/2 to external proxy**: Support Connect protocol (HTTP/1.1 capable) and gRPC-Web (HTTP/1.1 capable) natively. gRPC over HTTP/2 handled by fronting with an Envoy/nginx proxy that translates.

**Chosen: Option A with incremental delivery.** Phase 1 delivers Connect + gRPC-Web over HTTP/1.1 (covers browsers, curl, most clients). Phase 2 adds HTTP/2 for native gRPC. The protocol abstraction layer is protocol-agnostic from day one so adding HTTP/2 transport doesn't change handler code.

### WASM Compatibility

The core protocol logic (codec, compression, envelope framing, error types, interceptors) must compile to `wasm32-unknown-unknown`. Transport-specific code (TCP sockets, TLS, HTTP/2 frames) is `#[cfg(not(target_arch = "wasm32"))]` gated. WASM clients use browser Fetch API via foundation_wasm and wasm-bindgen bridges (we build each and feature gate them).

## Consequences

- At the **HTTP boundary** the transport deals in `SimpleIncomingRequest` / `SimpleOutgoingResponse` — no new HTTP type system. The **typed handler API** (async fns over concrete `Req`/`Res`) is Decision 04; the byte seam between them is Decision 11.
- **Handlers are async** — async fns / async `Stream`s driven by valtron via `from_future`/`from_stream` (Decisions 04/11/00). The valtron `TaskStatus`/queue machinery is internal; handlers do not return valtron iterators. (Earlier "iterator model, not async" framing was superseded.)
- gRPC (HTTP/2) is a Phase 2 transport addition, not a protocol-layer concern
- Connect + gRPC-Web work on HTTP/1.1 from day one
- Valtron executor handles concurrent stream processing for bidi RPCs
- WASM client support is possible because core protocol logic has no OS dependencies

## Decided Details

- **T1 — multi-version mapping (decided):** the connect-go → foundation type mapping in
  this doc is HTTP-version-agnostic. Every transport module (`simple_http/`, `http2/`,
  `http3/`) produces `SimpleIncomingRequest` / consumes `SimpleOutgoingResponse`; the
  unified seam is Decision 11 (`HandlerConn` / `ClientConn`, streaming `Transport`). The
  mapping below applies per version unchanged.
- **T2 — negotiation strategy (decided):** the server selects per connection — ALPN
  (`h2`, `http/1.1`) over TLS; HTTP/2 connection-preface ("prior knowledge") detection
  for cleartext h2c; `Alt-Svc: h3=...` advertises HTTP/3 for client upgrade on a later
  connection. The `ConnectionHandler` branch lives in Decision 12.
- **T3 — connection branching:** handled in Decision 12 (the `http2/` `ConnectionHandler`
  branch); HTTP/3 adds a third branch.
- **T6 — server push (decided):** ConnectRPC does not use HTTP/2 server push; the
  `http2/` module rejects/ignores `PUSH_PROMISE` frames.
- **T13 — reference sources:** record the local `h2` source path alongside the recorded
  `h3` and `iroh` paths when available.
- **T14 — QUIC backend (decided): use `quinn-proto`.** Research (sources at
  `/home/darkvoid/Boxxed/@formulas/src.rust/src.Quinn/quinn/quinn-proto`) confirms
  `quinn-proto` is a **pure sans-IO state machine with no tokio/runtime dependency** —
  deps are `bytes`/`rustls`/`ring`/`rand`/`slab`/`thiserror`/`tracing`/`tinyvec`; all async
  lives in the separate `quinn/` wrapper. Its API is the classic feed-bytes/poll-transmit
  shape (`Endpoint::handle`/`accept`, `Connection::poll_transmit`/`poll_timeout`/
  `handle_timeout`/`poll`, and `streams().open/accept` + `recv_stream.read` /
  `send_stream.write/finish/reset/stop`). So the `quic/` module does **not** reimplement
  QUIC. `quinn-proto` is a standalone crate (v0.12 on crates.io), so we **add it as a
  normal Cargo dependency** (feature-gated) rather than vendoring — vendor only if we later
  need to patch it. We write only the thin Valtron-driven event loop + UDP socket plumbing
  in netcap (optionally using `quinn-udp` for cross-platform GSO/GRO) + the `http3/quic.rs`
  trait impl over `quinn-proto`'s `Connection`/streams. It already uses rustls, matching
  `netcap/ssl`. This significantly reduces the Phase-3 scope.
- **Type sufficiency for HTTP/2 & HTTP/3 (decided): no new handler-facing types, no
  per-version wrappers.** `SimpleIncomingRequest` / `SimpleOutgoingResponse` /
  `SimpleHeaders` / `SendSafeBody` / `Proto` (already carries `HTTP20` / `HTTP30`), plus the
  new `ConnectionContext` (Q13) and the trailers field (B3), fully represent HTTP/2 and
  HTTP/3 at the handler boundary:
  - pseudo-headers (`:method`/`:path`/`:scheme`/`:authority`/`:status`) are mapped to the
    request/response fields *inside* the module (T8, Decision 12 §6);
  - trailers are a response part (B3);
  - the body is version-agnostic with a push-able backing (B6);
  - cancellation (RST_STREAM / stream reset) maps to `RequestContext` (Decision 11);
  - the HTTP version is on `Proto`, and per-request stream id lives in `ConnectionContext`.

  **We do not wrap request/response in Http2/Http3-specific types** — that would reintroduce
  the transport coupling the seam removes. New types live **only inside** `http2/` and
  `http3/`: the multiplexed *connection* object (frame codec + HPACK/QPACK + stream table +
  flow control) and its per-stream send/receive handles, which produce/consume the universal
  types and are never exposed to handlers. netcap's `Connection` / `Listener` gain `Quic` /
  `Iroh` variants (Decision 12 §9) for the accept/byte-stream side.

## Future-Phase Transport Roadmap (approach decided, not yet built)

### HTTP/3 (Phase 3)
Replicate the `h3` crate's design tokio-free (source:
`/home/darkvoid/Boxxed/@formulas/src.rust/src.tokio/h3/`) — QPACK, frame codec,
connection/stream mapping — implementing h3's backend-agnostic QUIC trait abstraction over
the QUIC backend. Rewrite `tokio::sync` → `ConcurrentQueueStreamIterator`/crossbeam and
`Poll` → synchronous/iterator equivalents. The **QUIC backend is `quinn-proto`** (see §T14:
sans-IO, tokio-free, added as a Cargo dependency); we write only the Valtron-driven driver
+ UDP plumbing in netcap (optionally `quinn-udp` for GSO/GRO). Module layout:

```
backends/foundation_netio/src/
├── simple_http/   # HTTP/1.1
├── http2/         # HTTP/2 (replicated from h2)
├── http3/         # HTTP/3 framing (replicated from h3); quic.rs = backend-agnostic trait
├── quic/          # quinn-proto (Cargo dep) + Valtron driver + UDP
└── netcap/        # shared TCP/TLS/RawStream
```
Produces `SimpleIncomingRequest` / consumes `SimpleOutgoingResponse` like the others;
`ConnectionHandler` adds a third branch. `Proto::HTTP30` already exists. Scope ≈ 6–10
features.

#### Our QUIC trait abstraction (valtron-native; h3 is a reference, not gospel)

We take h3's `quic.rs` as the catalogue of operations a QUIC backend must expose, but
**not** its `Poll`/`Context`/`Waker` shape — that exists only because h3 is driven by a
tokio reactor. valtron's `Stream<D, P>` (`Next` / `Pending` / `Wait` / `Delayed`) *is* the
readiness mechanism, and it also matches `quinn-proto` below (sans-IO: `streams().open` /
`accept`, `recv_stream.read`, `send_stream.write/finish/reset/stop`, `Connection::poll() ->
Event`). So we define our own trait set, synchronous + progress-returning, with no futures:

```rust
pub enum QuicConnError   { ApplicationClose { code: u64 }, Timeout, Internal(String), Other(BoxedError) }
pub enum QuicStreamError { ConnClosed(QuicConnError), Terminated { code: u64 }, Other(BoxedError) }

pub trait QuicConnection: Send {
    type RecvStream: QuicRecvStream;
    type SendStream: QuicSendStream;
    type BidiStream: QuicBidiStream;

    // Accept inbound streams: Next(stream) when one arrives, Pending/Wait when none yet,
    // ends (None) when the connection closes.
    fn accept_recv(&mut self) -> Stream<Result<Self::RecvStream, QuicConnError>, ()>;
    fn accept_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicConnError>, ()>;

    // Open outbound streams: Pending while blocked on the peer's stream-limit / flow control.
    fn open_bidi(&mut self) -> Stream<Result<Self::BidiStream, QuicStreamError>, ()>;
    fn open_send(&mut self) -> Stream<Result<Self::SendStream, QuicStreamError>, ()>;

    fn close(&mut self, code: u64, reason: &[u8]);   // fire-and-forget
}

pub trait QuicSendStream: Send {
    // h3's poll_ready + send_data folded into one progress-returning call:
    // Next(written) on accept, Pending when the flow-control window is full.
    fn send(&mut self, buf: &mut impl Buf) -> Stream<Result<usize, QuicStreamError>, ()>;
    fn finish(&mut self)  -> Stream<Result<(), QuicStreamError>, ()>;
    fn reset(&mut self, code: u64);                  // fire-and-forget
    fn id(&self) -> StreamId;                        // sync
}

pub trait QuicRecvStream: Send {
    // Next(Some(bytes)) per chunk, Next(None) at end-of-stream, Pending/Wait when nothing buffered.
    fn read(&mut self) -> Stream<Result<Option<Bytes>, QuicStreamError>, ()>;
    fn stop_sending(&mut self, code: u64);           // fire-and-forget
    fn id(&self) -> StreamId;                        // sync
}

pub trait QuicBidiStream: QuicSendStream + QuicRecvStream {
    fn split(self) -> (impl QuicSendStream, impl QuicRecvStream)
    where
        Self: Sized;   // static dispatch throughout — these traits are NOT dyn-compatible
                       // anyway (`send(&mut impl Buf)` is an APIT method); associated
                       // types + generics are the intended usage
}
```

**Deliberate deviations from h3:**
- **No `Poll`/`Context`/`Waker`** — the http3 driver is a valtron task that communicates
  waiting and readiness through the full **`TaskStatus`** vocabulary — `Pending` / `Wait` /
  `Delayed`, and crucially `Depends(EventReadiness)` to actually **park** on the native
  reactor (`RegisteredFd: EventReadiness`, Decisions 00/14) — rather than Rust's
  `Poll`/`Context`/`Waker`. This richer readiness signalling is the whole reason we don't use
  `h3` as-is. (T9 resolved here.) The per-call trait methods still hand back one `Stream`
  value at a time (below); it's the *driving task* that maps those into `TaskStatus`.
- **`poll_ready` folded into `send`** — write back-pressure is communicated by the driving
  task's `TaskStatus`: `Depends(QueueVacancyReadiness)` when the send pipe is full
  (Decision 00 L1b; async senders park on the pipe's stashed waker instead), or `Depends`
  on socket-writable via the reactor. So a separate `poll_ready` readiness method is
  redundant.

- **`accept_*` / `read` return one `Stream` value per call** (call repeatedly to advance —
  `Next(v)` / `Pending` / `Wait` / ends on close), in valtron's poll-per-call style.
- **`Is0rtt` is not a stream trait** — 0-RTT is connection/stream metadata, so it's a flag on
  `ConnectionContext` (Decision 04 Q13), not an I/O method.
- **`SendStreamUnframed` dropped** unless raw byte streaming (WebTransport-style) is needed
  later — noted as a possible future add, not core.
- **Error enums kept** (application-close / timeout / terminated / internal — protocol-
  meaningful, not tokio-specific).

Fire-and-forget ops (`reset` / `stop_sending` / `close` / `id`) stay plain sync methods.
Backends: `quic/` implements these over `quinn-proto`; the optional `iroh/` backend (below)
implements them over its quinn connection.

### iroh peer-to-peer (Phase 3+)
Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.WebTransport/src.n0-computer/iroh/`.
iroh = QUIC (built on quinn) + P2P (hole-punching, relay fallback, discovery) + public-key
identity. **It is tokio-based.** Two possible paths, with different value:

- **(a) Standalone P2P transport — first-class, the reason to integrate iroh.** iroh
  `Endpoint` → bidi/uni QUIC streams used directly (custom protocols, data sync, P2P
  messaging — no HTTP framing). Bridge iroh's streams into foundation_netio at the
  connection boundary (wrap iroh's tokio futures with a Valtron adapter / `block_on` at that
  seam). The peer's Ed25519 public key flows into auth via netcap `Endpoint<I>` identity →
  `ConnectionContext` (Decision 04 Q13 / Decision 09 T10). This is "use iroh for what it is."

- **(b) HTTP/3-over-iroh — optional / deferred, build only on demand.** Buys exactly one
  thing: running the *same* ConnectRPC services (Connect/gRPC/gRPC-Web handlers, router,
  codegen, interceptors) **unchanged** over a P2P link — because those protocols are
  HTTP-framed, reaching them P2P needs HTTP/3 over QUIC. The cost: iroh drags tokio while
  our `http3/` is tokio-free/valtron, so an iroh `QuicConnection` impl must bridge
  tokio↔valtron at the connection boundary — the two-executor friction we built everything
  to avoid. (And iroh already works with upstream tokio `h3` via `h3-quinn`, so a caller who
  needs HTTP/3-RPC over iroh *and* accepts tokio could use stock `h3` rather than our
  reimplementation.) **Decision: do not build (b) speculatively.** Justify it only if
  "unmodified ConnectRPC stack reachable P2P" becomes a stated requirement; if so, our
  backend-agnostic `QuicConnection` trait makes iroh just another backend impl (paying the
  bridge). Scope ≈ 2–3 features on top of HTTP/3, *if* pursued.

### Phasing
- **Phase 1:** HTTP/1.1 — Connect + gRPC-Web (no gRPC; bidi streaming rejected with 505 via Decision 11 capability matching).
- **Phase 2:** HTTP/2 (replicated from h2) — full gRPC, full-duplex bidi, client multiplexing.
- **Phase 3:** HTTP/3 (replicated from h3) + `quinn-proto` QUIC backend.
- **Phase 3+:** iroh P2P + public-key auth.

- **Phase 4 (last):** WebSocket transport — full-duplex bidi over an HTTP/1.1 `Upgrade`. The
  505 above applies only to **plain HTTP/1.1 POST** bidi; a WebSocket channel *is* our
  transport seam, so it can carry bidi on HTTP/1.1 deployments. Non-standard (our-stack-only)
  and additive. See **[Decision 13](13-websocket-transport.md)** — deferred, implemented last.
