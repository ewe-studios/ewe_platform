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
exposes `open(request: RequestHead) -> TransportStream` (a bidirectional body exchange the
protocol's `ClientConn` drives, backed by bounded pipes) plus `capabilities() ->
TransportCapabilities` so the client can reject incompatible protocol+version combinations
before sending. Unary is the degenerate case, available as a `round_trip` convenience built
on `open`:

```rust
pub trait Transport: Send + Sync + 'static {
    /// What this transport can do (duplex, trailers, HTTP versions, multiplexing) —
    /// used for capability matching (Decision 11).
    fn capabilities(&self) -> TransportCapabilities;

    /// Open a streaming exchange; returns the request sink + response source.
    fn open(&self, request: RequestHead) -> Result<TransportStream, TransportError>;
}

// Unary convenience over `open` (send one, close, read one):
impl dyn Transport {
    pub fn round_trip(&self, req: SimpleOutgoingRequest)
        -> Result<SimpleIncomingResponse, TransportError> { /* open + send + close + read */ }
}
```

Implementations: foundation_netio HTTP client (HTTP/1.1 today; HTTP/2/3 via the new
modules), WASM Fetch (unary + server-streaming only — `Duplex::None`), custom.
`SimpleOutgoingRequest` / `SimpleIncomingResponse` are the client-side counterparts to the
server types; reuse or adapt the existing `netcap` types. The streaming client types below
are realized over `MessageSource` / `MessageSink` (not embedded
`EnvelopeReader`/`EnvelopeWriter`).

### Client[Req, Res]

```rust
pub struct Client<Req, Res> {
    transport: Arc<dyn Transport>,
    config: ClientConfig,
    protocol: Box<dyn ProtocolClient>,
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req: Message, Res: Message + Default> Client<Req, Res> {
    pub fn new(transport: Arc<dyn Transport>, url: &str, options: ClientOptions)
        -> Result<Self, ConnectError>;

    // Async surface (mirrors the generated client + the server handler shapes).
    pub async fn unary(&self, ctx: &Ctx, request: Request<Req>)
        -> Result<Response<Res>, ConnectError>;

    /// Server streaming — await a response Stream.
    pub async fn server_stream(&self, ctx: &Ctx, request: Request<Req>)
        -> Result<impl Stream<Item = Result<Res, ConnectError>>, ConnectError>;

    /// Client streaming — send an async Stream of requests, await one response.
    pub async fn client_stream(&self, ctx: &Ctx, reqs: impl Stream<Item = Req>)
        -> Result<Response<Res>, ConnectError>;

    /// Bidi — async Stream in, async Stream out.
    pub async fn bidi_stream(&self, ctx: &Ctx, reqs: impl Stream<Item = Req>)
        -> Result<impl Stream<Item = Result<Res, ConnectError>>, ConnectError>;
}
```

### ClientConfig

```rust
pub struct ClientConfig {
    pub url: String,
    pub protocol: ProtocolSelection,         // Connect (default), gRPC, gRPC-Web
    pub codec: Arc<dyn Codec>,              // default: ProtoCodec
    pub compression: CompressionRegistry,
    pub send_compression: Option<String>,    // compress outgoing requests with this algorithm
    pub limits: SizeLimits,
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    pub default_timeout: Option<Duration>,
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
    pub fn with_codec(self, codec: Arc<dyn Codec>) -> Self;
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
}
```

### Unary Call Flow

1. Build `SimpleOutgoingRequest`:
   - Method: POST (or GET if idempotent + enabled)
   - URL: `{base_url}/{procedure}`
   - Headers: Content-Type, protocol-specific headers, timeout, compression, custom headers
   - Body: marshaled + optionally compressed request message
2. Apply client interceptor chain (wraps the call function)
3. `transport.round_trip(request)` → `SimpleIncomingResponse`
4. Check HTTP status:
   - 200: unmarshal response body
   - Non-200: parse error from body (Connect JSON error) or infer from HTTP status
5. Return `Response<Res>` with headers and trailers

### Streaming Client Types (lower-level handles)

These are the **lower-level** handles that the async `Client` methods above are built on —
exposed for callers who want manual control (drop a level). Each is a thin typed facade over
a `ClientConn` (Decision 11) obtained from `Transport::open`: outgoing messages go to the
conn's `MessageSink<Req>`, incoming to its `MessageSource<Res>`. They do **not** embed `EnvelopeReader`/`EnvelopeWriter` directly, and
the half-duplex (HTTP/1.1) vs full-duplex (HTTP/2/3) difference is handled by the transport
scheduling the conn's pipes — the API is identical.

```rust
/// Client's view of a server streaming RPC (one request, many responses).
pub struct ServerStream<Res> { conn: Box<dyn ClientConn>, _p: PhantomData<Res> }
impl<Res: Message + Default> ServerStream<Res> {
    pub fn receive(&mut self) -> Result<Option<Res>, ConnectError>; // None at end of stream
    pub fn response_headers(&self) -> &SimpleHeaders;               // available immediately
    pub fn response_trailers(&self) -> Result<&SimpleHeaders, ConnectError>; // after end
    /// Non-blocking close of the receive side (connection reuse).
    pub fn close(self);
}

/// Client's view of a client streaming RPC (many requests, one response).
pub struct ClientStream<Req, Res> { conn: Box<dyn ClientConn>, _p: PhantomData<(Req, Res)> }
impl<Req: Message, Res: Message + Default> ClientStream<Req, Res> {
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders; // before first send
    pub fn send(&mut self, msg: &Req) -> Result<(), ConnectError>;
    pub fn close_and_receive(self) -> Result<Response<Res>, ConnectError>;
}

/// Client's view of a bidirectional streaming RPC.
pub struct BidiStream<Req, Res> { conn: Box<dyn ClientConn>, _p: PhantomData<(Req, Res)> }
impl<Req: Message, Res: Message + Default> BidiStream<Req, Res> {
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    pub fn send(&mut self, msg: &Req) -> Result<(), ConnectError>;
    /// Header-only send (no body) is `send_headers` then proceed (C5).
    pub fn send_headers(&mut self) -> Result<(), ConnectError>;
    pub fn close_request(&mut self) -> Result<(), ConnectError>;
    pub fn receive(&mut self) -> Result<Option<Res>, ConnectError>;
    pub fn response_headers(&self) -> &SimpleHeaders;
    pub fn response_trailers(&self) -> Result<&SimpleHeaders, ConnectError>;
}
```

Over HTTP/1.1 `send` buffers into the bounded request pipe and the response side becomes
available after `close_request`/`close_and_receive` (half-duplex); over HTTP/2/3 the conn's
reader and writer run concurrently (full-duplex). Same types, no API change.

### HTTP GET for Idempotent Unary RPCs

When `IdempotencyLevel::NoSideEffects` and `with_http_get()` is enabled:

```rust
fn build_get_request(&self, request: &Request<Req>) -> Result<SimpleOutgoingRequest, ConnectError> {
    // GET encodes the message into the query via the stable codec below — no separate marshal.
    let mut query_params = vec![
        format!("encoding={}", self.config.codec.name()),
    ];

    // `marshal_stable` is on `Codec` (no separate `StableCodec`/`as_stable`), but it's a
    // `where Self: Sized` method — not callable through `Arc<dyn Codec>`. `config.codec` is the
    // negotiation handle; resolve it to the concrete codec (match on `config.codec.name()`),
    // then call the typed `marshal_stable::<Req>` on that concrete codec (no message erasure).
    let codec = resolve_concrete_codec(&self.config.codec);   // proto|json|arrow by name
    let is_binary = self.config.codec.is_binary();            // object-safe, fine on dyn
    let stable = codec.marshal_stable(&request.msg)?;         // concrete codec, concrete Req
    if is_binary {
        query_params.push(format!("message={}", base64url_encode(&stable)));
        query_params.push("base64=1".to_string());
    } else {
        query_params.push(format!("message={}", percent_encode(&stable)));
    }

    if let Some(compression) = &self.config.send_compression {
        if compression != "identity" {
            // Compress the message and base64-encode
            query_params.push(format!("compression={}", compression));
            query_params.push("base64=1".to_string());
        }
    }

    query_params.push(format!("connect={}", connect_protocol::QUERY_CONNECT_VERSION_VALUE));

    // Build GET request with query string, no body
}
```

### GET Fallback

connect-go supports falling back to POST if GET fails (URL too long, server rejects). Implement this:

```rust
fn call_unary_with_get_fallback(
    &self,
    ctx: &mut RequestContext,
    request: Request<Req>,
) -> Result<Response<Res>, ConnectError> {
    let get_request = self.build_get_request(&request)?;

    // Check URL length against configured max (default: 8KiB)
    if get_request.url().len() > self.config.get_url_max_bytes {
        return self.call_unary_post(ctx, request);
    }

    match self.transport.round_trip(get_request) {
        Ok(response) => self.process_unary_response(response),
        Err(_) if self.config.get_use_fallback => self.call_unary_post(ctx, request),
        Err(e) => Err(ConnectError::from(e)),
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

## Review-Gap Coverage

Transport + streaming client types are superseded by Decision 11. Remaining parity items,
folded in:

- **C1 — simple call variants:** add `call_client_stream_simple` / `call_bidi_stream_simple`
  that send headers immediately and return unwrapped types (simple codegen mode).
- **C2 — client context:** provide a client-context mechanism to set request headers and
  read response headers/trailers without the `Request`/`Response` wrappers.
- **C3 — Spec/Peer on streams:** expose `.spec()` / `.peer()` on all client stream types.
- **C4 — `ServerStream::close`:** non-blocking close of the receive side (connection
  reuse).
- **C5 — header-only send:** allow sending headers with no body (Decision 11
  `MessageSink::set_headers` + `close`).
- **C6 — default gzip accept:** the client accepts gzip by default.
- **C7 — default codecs:** proto + JSON registered by default (see Decisions 02 / 08).
- **T12 — version selection (decided):** `ClientOptions::with_preferred_http_version()`;
  Connect and gRPC-Web prefer the highest available version, gRPC requires ≥ HTTP/2 —
  enforced by Decision 11's capability matching.

## Open Questions

1. **SimpleOutgoingRequest**: foundation_netio's client types need verification. We may need `SimpleOutgoingRequest` (method, url, headers, body) as a new type if the existing client only supports `SimpleIncomingRequest`.
2. **Client streaming body — resolved:** no in-memory accumulation; the request body streams via chunked transfer-encoding over the pushable `SendSafeBody::Stream` (Decision 12 §7).
3. **Connection reuse**: HTTP/1.1 with keep-alive allows connection reuse across calls. Does foundation_netio's HTTP client handle this, or do we need connection pooling?
4. **Bidi over HTTP/1.1 — resolved:** rejected with `505` (capability matching, Decision 11), matching connect-go. Only client/server-streaming are half-duplex on HTTP/1.1.
