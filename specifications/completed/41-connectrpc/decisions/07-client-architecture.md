# Decision 07: Client Architecture

## Context

connect-go's client API provides a generic `Client[Req, Res]` that handles all four RPC types:

```go
type Client[Req, Res any] struct {
    config         *clientConfig
    callUnary      func(context.Context, *Request[Req]) (*Response[Res], error)
    protocolClient protocolClient
    err            error
}

func NewClient[Req, Res any](httpClient HTTPClient, url string, options ...ClientOption) *Client[Req, Res]
```

Where `HTTPClient` is:
```go
type HTTPClient interface {
    Do(*http.Request) (*http.Response, error)
}
```

This is just Go's `*http.Client`. The client constructs HTTP requests, sends them via the `HTTPClient`, and processes responses.

For streaming, connect-go uses `duplexHTTPCall` which:
- Creates an `io.Pipe` for the request body (writer feeds reader concurrently)
- Starts the HTTP request in a goroutine
- Writes envelope frames to the pipe writer (Send)
- Reads envelope frames from the response body (Receive)
- For bidi streaming: Send and Receive can be called concurrently

Client options select protocol:
- `WithGRPC()` — use gRPC protocol
- `WithGRPCWeb()` — use gRPC-Web protocol
- Default — use Connect protocol

## Decision

### Transport Trait

The client transport is **streaming-capable** and lives in `11-transport-seam.md`: it
exposes an **async** `open(request: RequestDescriptor) -> TransportStream` (a bidirectional body exchange
the protocol's `ClientConn` drives, backed by bounded pipes) plus `capabilities() ->
TransportCapabilities` so the client can reject incompatible protocol+version combinations
before sending. Unary is the degenerate case, available as a `round_trip` convenience built
on `open`. **All types are existing foundation_netio types** (verified) — no new
request/response types:

```rust
pub trait Transport: Send + Sync + 'static {
    /// What this transport can do (duplex, trailers, HTTP versions, multiplexing) —
    /// used for capability matching (Decision 11).
    fn capabilities(&self) -> TransportCapabilities;

    /// Open a streaming exchange; returns the request sink + response source.
    /// ASYNC (`BoxFuture` keeps the trait dyn-safe): connecting / pool checkout does I/O
    /// and must park, not block a worker (async-canonical decision).
    /// `RequestDescriptor` is netio's existing head-only request (proto/url/headers/method);
    /// the body flows through the returned `TransportStream` (Decision 11).
    fn open(&self, request: RequestDescriptor)
        -> BoxFuture<'static, Result<TransportStream, TransportError>>;
}

// Unary convenience over `open` (send one, close, read one):
impl dyn Transport {
    pub async fn round_trip(&self, req: PreparedRequest)
        -> Result<SimpleResponse<SendSafeBody>, TransportError> { /* open + send + close + read */ }
}
```

Implementations: foundation_netio HTTP client (HTTP/1.1 today; HTTP/2/3 via the new
modules), WASM Fetch (unary + server-streaming only — `request_streaming: false`,
Decision 11 capability matching), custom. The client
request/response types are the **existing** netio ones: `PreparedRequest` (out, converts to
`SimpleIncomingRequest` for rendering) and `SimpleResponse<SendSafeBody>` (in);
`RequestDescriptor` is the head-only request the streaming seam takes. There is **no**
`SimpleOutgoingRequest`/`SimpleIncomingResponse` (they don't exist and aren't needed). The
streaming client types below are realized over `MessageSource` / `MessageSink` (not embedded
`EnvelopeReader`/`EnvelopeWriter`).

### Client[Req, Res]

```rust
pub struct Client<Req, Res> {
    transport: Arc<dyn Transport>,
    config: ClientConfig,
    protocol: Box<dyn ProtocolClient>,
    /// Per-procedure typed codec table (Decision 02): codec name →
    /// (Arc<dyn CodecFor<Req>>, Arc<dyn CodecFor<Res>>). Built by the generated constructor
    /// (where Req/Res are concrete) for the default codecs + any custom codec — replaces the
    /// former unified `Message` bound, which does not exist.
    codecs: ProcedureCodecs<Req, Res>,
}

impl<Req: Send + 'static, Res: Send + 'static> Client<Req, Res> {
    pub fn new(transport: Arc<dyn Transport>, url: &str,
               codecs: ProcedureCodecs<Req, Res>, options: ClientOptions)
        -> ConnectResult<Self>;

    // Async surface (mirrors the generated client + the server handler shapes).
    // Ctx is the by-value Arc-backed context handle (Decision 04 §Ctx).
    pub async fn unary(&self, ctx: Ctx, request: Request<Req>)
        -> ConnectResult<Response<Res>>;

    /// Server streaming — await a response Stream.
    pub async fn server_stream(&self, ctx: Ctx, request: Request<Req>)
        -> ConnectResult<impl Stream<Item = ConnectResult<Res>>>;

    /// Client streaming — send an async Stream of requests, await one response.
    pub async fn client_stream(&self, ctx: Ctx, reqs: impl Stream<Item = Req>)
        -> ConnectResult<Response<Res>>;

    /// Bidi — async Stream in, async Stream out.
    pub async fn bidi_stream(&self, ctx: Ctx, reqs: impl Stream<Item = Req>)
        -> ConnectResult<impl Stream<Item = ConnectResult<Res>>>;
}
```

### ClientConfig

```rust
pub struct ClientConfig {
    pub url: String,
    pub protocol: ProtocolSelection,         // Connect (default), gRPC, gRPC-Web
    pub codec_name: String,                 // SEND-codec wire token, resolved in the
                                            // ProcedureCodecs table (Decision 02); default: "proto"
    pub compression: CompressionRegistry,
    pub send_compression: Option<String>,    // compress outgoing requests with this algorithm
    pub limits: SizeLimits,
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    pub default_timeout: Option<Duration>,
    pub get_url_max_bytes: usize,            // GET→POST fallback threshold; default 8 KiB (P14)
    pub get_use_fallback: bool,              // retry as POST when a GET attempt fails; default true
    pub preferred_http_version: Option<Proto>, // T12; enforced by capability matching (Decision 11)
}

pub enum ProtocolSelection {
    Connect,
    Grpc,
    GrpcWeb,
}
```

### ClientOptions (Builder)

```rust
pub struct ClientOptions {
    // Mirrors connect-go's option pattern
}

impl ClientOptions {
    pub fn new() -> Self;
    pub fn with_grpc(self) -> Self;
    pub fn with_grpc_web(self) -> Self;
    pub fn with_proto_json(self) -> Self;
    /// NAME selection among the client's installed ProcedureCodecs entries (it becomes the
    /// emitted Content-Type; the response arrives in the same codec per spec). Options can
    /// only SELECT — installation is registration-time via `<Service>Client::new_with_codec`
    /// (Decisions 02/10). Unknown name → error at `Client::new`.
    pub fn with_codec(self, name: &str) -> Self;
    pub fn with_send_gzip(self) -> Self;
    pub fn with_send_compression(self, name: &str) -> Self;
    pub fn with_accept_compression(self, name: &str, compressor: Arc<dyn Compressor>) -> Self;
    pub fn with_read_max_bytes(self, n: usize) -> Self;
    pub fn with_send_max_bytes(self, n: usize) -> Self;
    pub fn with_compress_min_bytes(self, n: usize) -> Self;
    pub fn with_interceptor(self, interceptor: Arc<dyn Interceptor>) -> Self;
    pub fn with_timeout(self, timeout: Duration) -> Self;
    pub fn with_idempotency(self, level: IdempotencyLevel) -> Self;
    pub fn with_http_get(self) -> Self;  // enable GET for idempotent unary RPCs
    pub fn with_get_url_max_bytes(self, n: usize) -> Self;      // P14 (default 8 KiB)
    pub fn with_get_fallback(self, enabled: bool) -> Self;       // GET→POST fallback
    pub fn with_preferred_http_version(self, v: Proto) -> Self;  // T12
    pub fn with_pipe_depth(self, n: usize) -> Self;              // seam pipe depth (Decision 11; default 4)
}
```

### Client-side `Ctx` contract (decided; resolves C2)

**Pass the ctx you're standing in.** The canonical call site is a handler making a
downstream RPC with the server-side `Ctx` it already has — that one habit is what makes
deadlines shrink hop-by-hop, cancellation sweep the whole call tree, and the `bag` reach
client interceptors unchanged. The division of ownership: the **client owns everything
call-mechanical** (transport, protocol, codecs, URL/peer, per-call `Spec`); the **ctx
carries only the caller's circumstances** — remaining deadline, cancellation, extensions,
bag. Top-level code with no inbound context (main, CLIs, tests, wasm entry) mints the base
case — `Ctx::client(bag)` / `Ctx::background()`, the `context.Background()` analog: a
one-line starting point, not a concept anyone manages — optionally derived via
`with_deadline` / `with_extension` / `with_cancellation` (Decision 04 §Ctx).

For every RPC the **`Client` derives a fresh per-call `Ctx`**, and *that* context flows
through the client interceptor chain — so interceptors always see an accurate `Spec`. One
inbound/root ctx is reusable across all calls and methods.

| field | source |
|---|---|
| `deadline` | caller — merged as **min**(caller deadline, `ClientOptions.default_timeout`); becomes the wire timeout header + local enforcement |
| cancellation | caller — the per-call signal is `CancelSignal::linked(&caller_signal)` (Decision 04 rules): the caller's cancel stops the call; a call-local cancel/deadline never propagates up to the root |
| `extensions` | caller — shallow-cloned into the call; visible to client interceptors; never sent on the wire |
| `bag` | caller |
| `spec` | **Client, per call** (procedure, stream type, idempotency) — any caller value is overwritten |
| `peer` | **Client** (from URL / transport) |
| headers | **not from `Ctx`** — outbound headers ride `Request<T>.headers` / `request_headers_mut()` |

Deadline enforcement fires the **per-call** signal only — that is exactly why the link is
one-way down (opt-in `linked`, never an implicit tree). This per-call context, plus
`.spec()`/`.peer()` on the stream handles (C3), *is* the C2 "client-context mechanism";
response headers/trailers are read from the stream handles / `Response<T>`.

### Unary Call Flow

1. Build a `PreparedRequest`:
   - Method: POST (or GET if idempotent + enabled)
   - URL: `{base_url}/{procedure}`
   - Headers: Content-Type, protocol-specific headers, timeout, compression, custom headers
   - Body: marshaled + optionally compressed request message (`SendSafeBody`)
2. Apply client interceptor chain (wraps the call function)
3. `transport.round_trip(request).await` → `SimpleResponse<SendSafeBody>`
4. Check HTTP status:
   - 200: unmarshal response body
   - Non-200: parse error from body (Connect JSON error) or infer from HTTP status
5. Return `Response<Res>` with headers and trailers

### Streaming Client Types (lower-level handles)

These are the **lower-level** handles that the async `Client` methods above are built on —
exposed for callers who want manual control (drop a level). Each is a thin typed facade over
the **split halves** of a `ClientConn` (Decision 11 — `split()` yields independently-owned
`ClientSender`/`ClientReceiver`) obtained from `Transport::open`: outgoing messages go
through the sender half (`MessageSink<Req>`), incoming through the receiver half
(`MessageSource<Res>`). They do **not** embed `EnvelopeReader`/`EnvelopeWriter` directly, and
the half-duplex (HTTP/1.1) vs full-duplex (HTTP/2/3) difference is handled by the transport
scheduling the conn's pipes — the API is identical.

```rust
// ASYNC-CANONICAL (decided): these handles are async facades over the ClientConn pipes,
// parking via Decision 00 — a blocking receive would tie up a valtron worker. Sync
// convenience wrappers (valtron block_on) exist for off-pool callers only. Typed
// encode/decode goes through the handle's `Arc<dyn CodecFor<…>>` (Decision 02).

/// Client's view of a server streaming RPC (one request, many responses). Holds only the
/// receiver half — the sender was consumed at open (request sent, then `close_send`).
pub struct ServerStream<Res> { recv: Box<dyn ClientReceiver>, codec: Arc<dyn CodecFor<Res>> }
impl<Res: Send + 'static> ServerStream<Res> {
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>>; // None at end of stream
    /// Sync OK: `server_stream()` awaited the response head before returning this handle
    /// (`ClientReceiver::response_headers` itself is async, Decision 11).
    pub fn response_headers(&self) -> &SimpleHeaders;
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders>; // after end
    // (&mut: delegates to the seam's `&mut self` method and may drive the conn to EOS)
    /// Non-blocking close of the receive side (connection reuse).
    pub fn close(self);
}

/// Client's view of a client streaming RPC (many requests, one response).
pub struct ClientStream<Req, Res> {
    send: Box<dyn ClientSender>,
    recv: Box<dyn ClientReceiver>,
    req_codec: Arc<dyn CodecFor<Req>>,
    res_codec: Arc<dyn CodecFor<Res>>,
}
impl<Req: Send + 'static, Res: Send + 'static> ClientStream<Req, Res> {
    /// Delegates to the SENDER half (`ClientSender::request_headers_mut`, Decision 11) —
    /// valid before the first send.
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()>;
    pub async fn close_and_receive(self) -> ConnectResult<Response<Res>>;
}

/// Client's view of a bidirectional streaming RPC.
pub struct BidiStream<Req, Res> {
    send: Box<dyn ClientSender>,
    recv: Box<dyn ClientReceiver>,
    req_codec: Arc<dyn CodecFor<Req>>,
    res_codec: Arc<dyn CodecFor<Res>>,
}
impl<Req: Send + 'static, Res: Send + 'static> BidiStream<Req, Res> {
    /// Delegates to the SENDER half (valid before the first send — Decision 11).
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    pub async fn send(&mut self, msg: &Req) -> ConnectResult<()>;
    /// Header-only send (no body) — `ClientSender::flush_headers` (C5, Decision 11).
    pub async fn send_headers(&mut self) -> ConnectResult<()>;
    pub async fn close_request(&mut self) -> ConnectResult<()>;
    pub async fn receive(&mut self) -> ConnectResult<Option<Res>>;
    /// Bidi: the head arrives whenever the server sends it — awaiting may park
    /// (async-canonical; over `ClientReceiver::response_headers`, Decision 11).
    pub async fn response_headers(&mut self) -> ConnectResult<&SimpleHeaders>;
    pub async fn response_trailers(&mut self) -> ConnectResult<&SimpleHeaders>;

    /// CONCURRENT send/receive (fresh-review A2): split into independently-owned typed
    /// halves so one task sends while another receives — required for deadlock-freedom on
    /// bounded pipes (a server that fills its response pipe while not draining requests
    /// would otherwise deadlock a client that can only alternate). connect-go parity
    /// (its Send/Receive may be called concurrently).
    pub fn split(self) -> (BidiSender<Req>, BidiReceiver<Res>);
}

/// The typed halves `BidiStream::split` yields — each wraps its seam half + codec handle
/// (exactly `ClientStream`'s two field pairs, separated):
pub struct BidiSender<Req>   { send: Box<dyn ClientSender>,   codec: Arc<dyn CodecFor<Req>> }
pub struct BidiReceiver<Res> { recv: Box<dyn ClientReceiver>, codec: Arc<dyn CodecFor<Res>> }
// BidiSender: send / send_headers(flush) / close_request;  BidiReceiver: receive /
// response_headers / response_trailers — same semantics as the unsplit methods.
```

Over HTTP/1.1 `send` buffers into the bounded request pipe and the response side becomes
available after `close_request`/`close_and_receive` (half-duplex); over HTTP/2/3 the conn's
reader and writer run concurrently (full-duplex). Same types, no API change.

### HTTP GET for Idempotent Unary RPCs

When `IdempotencyLevel::NoSideEffects` and `with_http_get()` is enabled:

```rust
fn build_get_request(&self, request: &Request<Req>) -> ConnectResult<PreparedRequest> {
    // GET encodes the message into the query via the stable codec — no separate marshal.
    let mut query_params = vec![
        format!("encoding={}", self.config.codec_name),
    ];

    // `marshal_stable` is on `CodecFor<Req>` (Decision 02) — dyn-callable; `Codec` is its
    // supertrait, so `is_binary` is reachable through the same handle. Resolved once from
    // the ProcedureCodecs table (the only codec authority — no registry).
    let codec = self.codecs.for_request(&self.config.codec_name)?; // Arc<dyn CodecFor<Req>>
    let stable = codec.marshal_stable(&request.msg)?;

    // Compression is applied BEFORE query encoding — the compressed bytes are what ride the
    // URL (an earlier sketch pushed `message=` first and only then flagged compression);
    // compressed payloads are always base64url, regardless of the codec's text/binary form.
    let (payload, compressed) = match &self.config.send_compression {
        Some(algo) if algo != "identity" => {
            let compressor = self.config.compression.get(algo)
                .ok_or_else(|| ConnectError::internal(format!("unknown compression {algo}")))?;
            query_params.push(format!("compression={algo}"));
            (compressor.compress(&stable)?, true)
        }
        _ => (stable, false),
    };

    if codec.is_binary() || compressed {
        query_params.push(format!("message={}", base64url_encode(&payload)));
        query_params.push("base64=1".to_string());
    } else {
        query_params.push(format!("message={}", percent_encode(&payload)));
    }

    query_params.push(format!("connect={}", connect_protocol::QUERY_CONNECT_VERSION_VALUE));

    // Build GET request with query string, no body
}
```

### GET Fallback

Fallback to POST triggers in exactly two DETERMINISTIC cases (fresh-review-2 #11) — never
on transport errors, which would risk double-executing a request that may already have run:

1. **URL too long** — pre-flight length check against `get_url_max_bytes` (never sent).
2. **Server rejected the GET without executing it** — a `405`/`415` response (a server
   without GET support), retried as POST once iff `get_use_fallback` (safe: rejection
   precedes execution). Any other response — including errors — is processed normally.

```rust
async fn call_unary_with_get_fallback(
    &self,
    ctx: Ctx,
    request: Request<Req>,
) -> ConnectResult<Response<Res>> {
    let get_request = self.build_get_request(&request)?;

    // Case 1: pre-flight URL length check (default max: 8 KiB).
    if get_request.url().len() > self.config.get_url_max_bytes {
        return self.call_unary_post(ctx, request).await;
    }

    match self.transport.round_trip(get_request).await {
        // Case 2: GET rejected before execution → safe single retry as POST.
        Ok(r) if self.config.get_use_fallback && matches!(r.status(), 405 | 415) =>
            self.call_unary_post(ctx, request).await,
        Ok(response) => self.process_unary_response(response),
        // Transport errors are NOT retried (the GET may have executed — no double-send).
        Err(e) => Err(ConnectError::from(e).into()),
    }
}
```

## Consequences

- Client is generic over `Transport` — works with foundation_netio, WASM Fetch, or custom HTTP
- Unary calls are straightforward: build request, send, parse response
- Client streaming over HTTP/1.1: the request body is streamed via chunked transfer-encoding using the pushable `SendSafeBody::Stream` (Decision 12 §7) — no full-stream in-memory buffering
- Server streaming: read envelopes from response body iterator
- Bidi over HTTP/1.1: **rejected with 505** (Decision 11 capability matching, connect-go parity); only client/server-streaming are half-duplex there
- Bidi streaming over HTTP/2: full-duplex (requires HTTP/2 transport — Phase 2)
- HTTP GET support for idempotent RPCs with automatic POST fallback

## Decided Details

- **C1 — simple call variants:** add `call_client_stream_simple` / `call_bidi_stream_simple`
  that send headers immediately and return unwrapped types (simple codegen mode).
- **C2 — client context:** resolved by §Client-side `Ctx` contract above (per-call derived
  `Ctx` + stream-handle accessors).
- **C3 — Spec/Peer on streams:** expose `.spec()` / `.peer()` on all client stream types.
- **C4 — `ServerStream::close`:** non-blocking close of the receive side (connection
  reuse).
- **C5 — header-only send:** allow sending headers with no body (Decision 11
  `ClientSender::flush_headers` — the client-side header op; `MessageSink::set_headers`
  is the server-side counterpart).
- **C6 — default gzip accept:** the client accepts gzip by default.
- **C7 — default codecs:** proto + JSON registered by default (see Decisions 02 / 08).
- **T12 — version selection (decided):** `ClientOptions::with_preferred_http_version()`;
  Connect and gRPC-Web prefer the highest available version, gRPC requires ≥ HTTP/2 —
  enforced by Decision 11's capability matching.

- **Connection ownership / who spawns the pump — decided (Decision 11 §Connection ownership):**
  `Transport::open` **is** the client connection owner and **spawns the byte pump** (valtron)
  before returning the `TransportStream`; the pump holds the socket-facing pipe halves, the caller
  gets `send_body`/`recv_body`. Over foundation_netio the pump is the driven
  `ClientRequest::send_async` task — its request-receiver half is the pushable body
  (`PushableRequestBody::into_sender()` → `send_body`), and it copies the returned lazy response
  stream into `recv_body`. Not a lazy drive inside the `response` future (that deadlocks a request
  larger than the pipe depth — the push loop parks while the only drainer sits behind the
  un-awaited head). This is the valtron materialization of connect-go's implicit `net/http`
  request goroutine.
- **Connection reuse — decided (verified in foundation_netio):** no new pooling in connectrpc; reuse is split by protocol.
   - **HTTP/1.1:** foundation_netio's `HttpConnectionPool` (`client/native/pool.rs` + `connection.rs`) already provides keep-alive reuse — per-`host:port` LIFO checkout/checkin of exclusive `SharedByteBufferStream<RawStream>`s, `max_per_host` cap, `max_idle_time` staleness eviction. The client uses it transparently: on response drop the stream is drained and returned to the pool, honoring `Connection: close` (`FinalizedResponse::drop`). The connectrpc client gets this for free through the Decision 11 transport seam.
   - **HTTP/2 / HTTP/3:** this pool does **not** apply — its exclusive one-request-per-connection ownership model is inherently HTTP/1.1. h2/h3 reuse is multiplexing many concurrent streams over **one shared connection per origin**, owned by the Decision 12 multiplexer (`http2/` / `http3/`); h3 has no `RawStream` to pool at all (QUIC endpoint owns the connection). Not a gap — the standard split (cf. hyper: h1 idle pool vs. shared h2 connection per origin).
   - **Caveat (h1 streaming):** a long-lived streaming RPC over HTTP/1.1 holds its connection exclusively for the stream's entire lifetime (inherent to h1, not a pool flaw). Heavy h1 streaming workloads need `max_per_host` headroom; truly concurrent streaming belongs on h2.
   - Netio follow-up (not a connectrpc concern): the pool self-describes as conservative (`Arc<Mutex<…>>`, sync); background cleanup / async-aware primitives are noted for a later phase.
