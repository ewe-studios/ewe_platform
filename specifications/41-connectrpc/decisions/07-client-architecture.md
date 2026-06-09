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

```rust
/// HTTP transport abstraction for sending RPC requests.
/// Implementations: foundation_netio HTTP client, WASM Fetch, custom.
pub trait Transport: Send + Sync + 'static {
    /// Send an HTTP request and return the response.
    fn round_trip(
        &self,
        request: SimpleOutgoingRequest,
    ) -> Result<SimpleIncomingResponse, TransportError>;
}
```

Note: `SimpleOutgoingRequest` and `SimpleIncomingResponse` are the client-side counterparts. We may need to define these if foundation_netio's HTTP client doesn't already provide them, or adapt the existing `SimpleIncomingRequest` / `SimpleOutgoingResponse` types.

**Foundation_netio's HTTP client**: Check what client abstraction exists. If it provides `send(request) -> response`, we adapt it. If not, we define the transport interface and provide a default implementation using foundation_netio's TCP/TLS stack.

### Client[Req, Res]

```rust
pub struct Client<Req, Res> {
    transport: Arc<dyn Transport>,
    config: ClientConfig,
    protocol: Box<dyn ProtocolClient>,
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req: MessageRef, Res: MessageMut + Default> Client<Req, Res> {
    pub fn new(
        transport: Arc<dyn Transport>,
        url: &str,
        options: ClientOptions,
    ) -> Result<Self, ConnectError>;

    /// Unary RPC call.
    pub fn call_unary(
        &self,
        ctx: &mut RequestContext,
        request: Request<Req>,
    ) -> Result<Response<Res>, ConnectError>;

    /// Server streaming RPC call.
    pub fn call_server_stream(
        &self,
        ctx: &mut RequestContext,
        request: Request<Req>,
    ) -> Result<ServerStream<Res>, ConnectError>;

    /// Client streaming RPC call.
    pub fn call_client_stream(
        &self,
        ctx: &mut RequestContext,
    ) -> ClientStream<Req, Res>;

    /// Bidirectional streaming RPC call.
    pub fn call_bidi_stream(
        &self,
        ctx: &mut RequestContext,
    ) -> BidiStream<Req, Res>;
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

### Streaming Client Types

```rust
/// Client's view of a server streaming RPC.
pub struct ServerStream<Res> {
    response: SimpleIncomingResponse,
    reader: EnvelopeReader,
    response_headers: SimpleHeaders,
    _phantom: PhantomData<Res>,
}

impl<Res: MessageMut + Default> ServerStream<Res> {
    /// Read the next response message. Returns None at end of stream.
    pub fn receive(&mut self) -> Result<Option<Res>, ConnectError>;

    /// Response headers (available immediately).
    pub fn response_headers(&self) -> &SimpleHeaders;

    /// Response trailers (available after stream ends).
    pub fn response_trailers(&self) -> Result<&SimpleHeaders, ConnectError>;
}

/// Client's view of a client streaming RPC.
pub struct ClientStream<Req, Res> {
    writer: EnvelopeWriter,
    transport: Arc<dyn Transport>,
    request_builder: SimpleOutgoingRequestBuilder,
    body_parts: Vec<Vec<u8>>,  // accumulated envelope frames
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req: MessageRef, Res: MessageMut + Default> ClientStream<Req, Res> {
    /// Request headers (writable before first send).
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders;

    /// Send a request message.
    pub fn send(&mut self, msg: &Req) -> Result<(), ConnectError>;

    /// Close the request side and receive the response.
    pub fn close_and_receive(self) -> Result<Response<Res>, ConnectError>;
}

/// Client's view of a bidirectional streaming RPC.
pub struct BidiStream<Req, Res> {
    writer: EnvelopeWriter,
    reader: EnvelopeReader,
    // For HTTP/1.1: accumulate request, then read response (half-duplex)
    // For HTTP/2: concurrent read/write via separate transport channels
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req: MessageRef, Res: MessageMut + Default> BidiStream<Req, Res> {
    pub fn request_headers_mut(&mut self) -> &mut SimpleHeaders;
    pub fn send(&mut self, msg: &Req) -> Result<(), ConnectError>;
    pub fn close_request(&mut self) -> Result<(), ConnectError>;
    pub fn receive(&mut self) -> Result<Option<Res>, ConnectError>;
    pub fn response_headers(&self) -> &SimpleHeaders;
    pub fn response_trailers(&self) -> Result<&SimpleHeaders, ConnectError>;
}
```

### HTTP GET for Idempotent Unary RPCs

When `IdempotencyLevel::NoSideEffects` and `with_http_get()` is enabled:

```rust
fn build_get_request(&self, request: &Request<Req>) -> Result<SimpleOutgoingRequest, ConnectError> {
    let encoded = self.config.codec.marshal(request.msg.as_ref())?;

    let mut query_params = vec![
        format!("encoding={}", self.config.codec.name()),
    ];

    if let Some(stable_codec) = self.config.codec.as_stable() {
        let stable = stable_codec.marshal_stable(request.msg.as_ref())?;
        if stable_codec.is_binary() {
            query_params.push(format!("message={}", base64url_encode(&stable)));
            query_params.push("base64=1".to_string());
        } else {
            query_params.push(format!("message={}", percent_encode(&stable)));
        }
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
- Client streaming over HTTP/1.1: accumulate all messages in memory, then send as single request body
- Server streaming: read envelopes from response body iterator
- Bidi streaming over HTTP/1.1: half-duplex (send all, then receive all)
- Bidi streaming over HTTP/2: full-duplex (requires HTTP/2 transport — Phase 2)
- HTTP GET support for idempotent RPCs with automatic POST fallback

## Open Questions

1. **SimpleOutgoingRequest**: foundation_netio's client types need verification. We may need `SimpleOutgoingRequest` (method, url, headers, body) as a new type if the existing client only supports `SimpleIncomingRequest`.
2. **Client streaming body accumulation**: Over HTTP/1.1, the client must buffer all request messages before sending. This could be memory-intensive for large streams. Should we set a default buffer limit, or leave it to the user?
3. **Connection reuse**: HTTP/1.1 with keep-alive allows connection reuse across calls. Does foundation_netio's HTTP client handle this, or do we need connection pooling?
4. **Concurrent bidi over HTTP/1.1**: connect-go supports "half-duplex bidi over HTTP/1.1" by consuming the full request before streaming the response. This means `send()` accumulates locally and `close_request()` triggers the actual HTTP request. The response stream is only available after `close_request()`. Is this acceptable, or should we error on bidi over HTTP/1.1?
