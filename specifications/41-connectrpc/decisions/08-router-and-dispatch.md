# Decision 08: Router & Multi-Protocol Dispatch

## Context

connect-go's `Handler` integrates with Go's `http.ServeMux` as an `http.Handler`:

```go
mux := http.NewServeMux()
mux.Handle(greetv1connect.NewGreetServiceHandler(server))
// NewGreetServiceHandler returns (string, *Handler) where string is the path prefix
```

The `Handler` struct:
```go
type Handler struct {
    spec             Spec
    implementation   StreamingHandlerFunc
    protocolHandlers map[string][]protocolHandler // Method → protocol handlers
    allowMethod      string                       // Allow header
    acceptPost       string                       // Accept-Post header
}
```

On each request, Handler:
1. Checks HTTP method against `protocolHandlers[method]`
2. For each protocol handler, checks `CanHandlePayload` (Content-Type match)
3. First matching protocol handler processes the request
4. If no match: 415 Unsupported Media Type or 405 Method Not Allowed

connect-go's routing is per-procedure: each RPC method gets its own `Handler` registered at its path (`/package.Service/Method`). The `http.ServeMux` routes by path prefix.

## Decision

### Path Format

Following the ConnectRPC spec and connect-go:
```
/{optional_routing_prefix}/{package.Service}/{Method}
```

Examples:
- `/connectrpc.greet.v1.GreetService/Greet`
- `/api/connectrpc.greet.v1.GreetService/Greet` (with routing prefix)

The router matches on the full `/{service}/{method}` suffix.

### Router

```rust
pub struct Router {
    /// Maps procedure path → registered handler entry.
    /// Key: "package.Service/Method" (without leading slash)
    handlers: HashMap<String, HandlerEntry>,

    /// Shared codec registry for all handlers.
    codecs: Arc<CodecRegistry>,

    /// Shared compression registry for all handlers.
    compression: Arc<CompressionRegistry>,

    /// Global handler options (interceptors, size limits, etc.)
    global_options: HandlerOptions,
}

struct HandlerEntry {
    spec: Spec,
    handler: HandlerKind,
    options: HandlerOptions,  // per-procedure options (merged with global)
    protocol_handlers: Vec<Box<dyn ProtocolHandler>>,
}

enum HandlerKind {
    Unary(Arc<dyn ErasedUnaryHandler>),
    ServerStream(Arc<dyn ErasedServerStreamHandler>),
    ClientStream(Arc<dyn ErasedClientStreamHandler>),
    BidiStream(Arc<dyn ErasedBidiStreamHandler>),
}
```

### Type-Erased Handler Traits

To store handlers of different `Req`/`Res` types in the same `HashMap`:

```rust
pub(crate) trait ErasedUnaryHandler: Send + Sync {
    fn handle(
        &self,
        ctx: &RequestContext,
        codec: &dyn Codec,
        body: &[u8],
    ) -> Result<(Vec<u8>, SimpleHeaders, SimpleHeaders), ConnectError>;
    // Returns (encoded_response_bytes, response_headers, response_trailers)
}

pub(crate) trait ErasedServerStreamHandler: Send + Sync {
    fn handle(
        &self,
        ctx: &RequestContext,
        codec: &dyn Codec,
        body: &[u8],
    ) -> Result<(SimpleHeaders, Box<dyn ErasedStreamIterator>), ConnectError>;
    // Returns (response_headers, stream_of_encoded_messages)
}

pub(crate) trait ErasedClientStreamHandler: Send + Sync {
    fn handle(
        &self,
        ctx: &RequestContext,
        codec: &dyn Codec,
        messages: Box<dyn ErasedStreamIterator>,
    ) -> Result<(Vec<u8>, SimpleHeaders, SimpleHeaders), ConnectError>;
}

pub(crate) trait ErasedBidiStreamHandler: Send + Sync {
    fn handle(
        &self,
        ctx: &RequestContext,
        codec: &dyn Codec,
        messages: Box<dyn ErasedStreamIterator>,
    ) -> Result<(SimpleHeaders, Box<dyn ErasedStreamIterator>), ConnectError>;
}

pub(crate) trait ErasedStreamIterator: Send {
    fn next(&mut self) -> Stream<Result<Vec<u8>, ConnectError>, ()>;
}
```

The type-erased boundary works at the byte level: handlers receive raw bytes and return raw bytes. The wrapper between the typed handler and the erased handler does codec marshal/unmarshal.

### Handler Registration

```rust
impl Router {
    pub fn new() -> Self;

    pub fn unary<Req, Res, H>(
        &mut self,
        procedure: &str,
        handler: H,
        options: HandlerOptions,
    ) where
        Req: MessageMut + Default + 'static,
        Res: MessageRef + 'static,
        H: UnaryHandler<Req, Res>;

    pub fn server_stream<Req, Res, H>(
        &mut self,
        procedure: &str,
        handler: H,
        options: HandlerOptions,
    ) where
        Req: MessageMut + Default + 'static,
        Res: MessageRef + 'static,
        H: ServerStreamHandler<Req, Res>;

    pub fn client_stream<Req, Res, H>(
        &mut self,
        procedure: &str,
        handler: H,
        options: HandlerOptions,
    ) where
        Req: MessageMut + Default + 'static,
        Res: MessageRef + 'static,
        H: ClientStreamHandler<Req, Res>;

    pub fn bidi_stream<Req, Res, H>(
        &mut self,
        procedure: &str,
        handler: H,
        options: HandlerOptions,
    ) where
        Req: MessageMut + Default + 'static,
        Res: MessageRef + 'static,
        H: BidiStreamHandler<Req, Res>;
}
```

### HandlerOptions

```rust
pub struct HandlerOptions {
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    pub codecs: Option<Arc<CodecRegistry>>,            // override global
    pub compression: Option<Arc<CompressionRegistry>>,  // override global
    pub limits: SizeLimits,
    pub idempotency: IdempotencyLevel,
    pub require_connect_protocol_header: bool,
}

impl HandlerOptions {
    pub fn new() -> Self;
    pub fn with_interceptors(self, interceptors: Vec<Arc<dyn Interceptor>>) -> Self;
    pub fn with_read_max_bytes(self, n: usize) -> Self;
    pub fn with_send_max_bytes(self, n: usize) -> Self;
    pub fn with_idempotency(self, level: IdempotencyLevel) -> Self;
    pub fn with_codec(self, codec: Arc<dyn Codec>) -> Self;
    pub fn with_compression(self, name: &str, compressor: Arc<dyn Compressor>) -> Self;
    pub fn with_recover<F>(self, handler: F) -> Self
    where
        F: Fn(&RequestContext, &Spec, &SimpleHeaders, Box<dyn Any + Send>) -> ConnectError + Send + Sync + 'static;
}
```

### Request Dispatch Flow

When an HTTP request arrives at the ConnectRPC handler:

```
1. Extract path → lookup in handlers HashMap
   ├── Not found → 404 Not Found
   └── Found HandlerEntry

2. Check HTTP method
   ├── Not in allowed methods → 405 Method Not Allowed (with Allow header)
   └── Method allowed

3. For each protocol handler in HandlerEntry.protocol_handlers:
   ├── Check can_handle(request) (Content-Type match)
   │   ├── No match → try next protocol handler
   │   └── Match found
   │
   4. Parse timeout from headers → create RequestContext with deadline
   5. Negotiate compression
   6. Determine codec from Content-Type
   │
   7. Branch on handler kind:
   │   ├── Unary:
   │   │   ├── Read full request body
   │   │   ├── Decompress if needed
   │   │   ├── Apply interceptor chain
   │   │   ├── Call handler
   │   │   ├── Marshal response
   │   │   ├── Compress if negotiated
   │   │   └── Write HTTP response
   │   │
   │   ├── ServerStream:
   │   │   ├── Read full request body (single message)
   │   │   ├── Decompress + unmarshal
   │   │   ├── Apply interceptor chain
   │   │   ├── Call handler → get response stream
   │   │   ├── For each message in stream:
   │   │   │   ├── Marshal + compress
   │   │   │   └── Write envelope frame
   │   │   └── Write end-of-stream frame
   │   │
   │   ├── ClientStream:
   │   │   ├── Create envelope reader from request body
   │   │   ├── Apply interceptor chain
   │   │   ├── Call handler with message iterator
   │   │   ├── Marshal response
   │   │   └── Write response (envelope-framed)
   │   │
   │   └── BidiStream:
   │       ├── Create envelope reader from request body
   │       ├── Apply interceptor chain
   │       ├── Call handler with input stream → get output stream
   │       ├── For each output message:
   │       │   └── Write envelope frame
   │       └── Write end-of-stream frame
   │
   8. If no protocol handler matched → 415 Unsupported Media Type

Error at any point → ErrorWriter writes protocol-appropriate error response
```

### Integration with foundation_http

The Router produces a handler compatible with foundation_http's routing:

```rust
impl Router {
    /// Create a foundation_http handler that dispatches ConnectRPC requests.
    /// This handler should be registered as a catch-all or prefix route.
    pub fn into_handler(self) -> ConnectRpcHandler;
}

pub struct ConnectRpcHandler {
    router: Arc<Router>,
}

// Implements foundation_http's handler trait (ServeWriter or equivalent)
// When a request arrives:
// 1. Extract procedure from URL path
// 2. Look up handler in router
// 3. Execute dispatch flow above
// 4. Write response to SimpleOutgoingResponse
```

### Generated Service Registration

Code generation produces registration helpers:

```rust
// Generated for each service:
pub trait GreetServiceHandler: Send + Sync + 'static {
    fn greet(&self, ctx: &RequestContext, req: Request<GreetRequest>) -> Result<Response<GreetResponse>, ConnectError>;
    fn greet_group(&self, ctx: &RequestContext, headers: &SimpleHeaders, reqs: Box<dyn StreamIterator<GreetRequest>>) -> Result<Response<GreetGroupResponse>, ConnectError>;
}

// Registration function (generated):
pub fn register_greet_service<S: GreetServiceHandler>(router: &mut Router, service: Arc<S>) {
    router.unary(
        "connectrpc.greet.v1.GreetService/Greet",
        /* wrapper that delegates to service.greet() */,
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    router.client_stream(
        "connectrpc.greet.v1.GreetService/GreetGroup",
        /* wrapper that delegates to service.greet_group() */,
        HandlerOptions::new(),
    );
}
```

## Consequences

- One `Router` handles all three protocols automatically (Connect, gRPC, gRPC-Web)
- Protocol detection is per-request via Content-Type, not per-registration
- Type erasure at the byte boundary keeps the router generic-free
- `into_handler()` bridges to foundation_http for HTTP server integration
- Generated code provides ergonomic service registration
- Per-procedure options (interceptors, limits, idempotency) supported

## Open Questions

1. **Path prefix routing**: foundation_http's `Router<S>` does tree-based route matching. Should we register each procedure as a separate route, or register a single prefix route and do sub-routing internally? connect-go registers each procedure separately with Go's `ServeMux`.
2. **Middleware ordering with foundation_http**: foundation_http has its own middleware chain (CORS, logging, auth, compression). How does this interact with ConnectRPC interceptors? Should ConnectRPC middleware run inside or outside foundation_http middleware?
3. **Graceful shutdown**: connect-go doesn't handle this (Go's HTTP server does). Foundation_http's server model — does it support draining in-flight requests?
4. **Multiple services on one router**: connect-go handles this via `http.ServeMux` path multiplexing. Our Router handles it natively via the HashMap. Confirm that procedure paths are globally unique across services (they should be, since they include the full package+service name).
