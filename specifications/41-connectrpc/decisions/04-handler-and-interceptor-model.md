# Decision 04: Handler & Interceptor Model

## Context

connect-go defines handler and interceptor APIs that are the core user-facing surface. Understanding them precisely is critical because they determine developer ergonomics.

### connect-go Handler API

**Unary:**
```go
func NewUnaryHandler[Req, Res any](
    procedure string,
    unary func(context.Context, *Request[Req]) (*Response[Res], error),
    options ...HandlerOption,
) *Handler
```

**Client Streaming:**
```go
func NewClientStreamHandler[Req, Res any](
    procedure string,
    handler func(context.Context, *ClientStream[Req]) (*Response[Res], error),
    options ...HandlerOption,
) *Handler
```

**Server Streaming:**
```go
func NewServerStreamHandler[Req, Res any](
    procedure string,
    handler func(context.Context, *Request[Req], *ServerStream[Res]) error,
    options ...HandlerOption,
) *Handler
```

**Bidi Streaming:**
```go
func NewBidiStreamHandler[Req, Res any](
    procedure string,
    handler func(context.Context, *BidiStream[Req, Res]) error,
    options ...HandlerOption,
) *Handler
```

### connect-go Request/Response Wrappers

```go
type Request[T any] struct {
    Msg    *T
    spec   Spec
    peer   Peer
    header http.Header
}

type Response[T any] struct {
    Msg     *T
    header  http.Header
    trailer http.Header
}

type Spec struct {
    StreamType       StreamType
    Schema           any
    Procedure        string
    IsClient         bool
    IdempotencyLevel IdempotencyLevel
}

type Peer struct {
    Addr     string
    Protocol string
    Query    url.Values
}
```

### connect-go Interceptor API

```go
type Interceptor interface {
    WrapUnary(UnaryFunc) UnaryFunc
    WrapStreamingClient(StreamingClientFunc) StreamingClientFunc
    WrapStreamingHandler(StreamingHandlerFunc) StreamingHandlerFunc
}

type UnaryFunc func(context.Context, AnyRequest) (AnyResponse, error)
type StreamingClientFunc func(context.Context, Spec) StreamingClientConn
type StreamingHandlerFunc func(context.Context, StreamingHandlerConn) error
```

Key design: interceptors wrap at two levels:
1. **Unary** — wraps the request→response function (can inspect/modify request and response)
2. **Streaming** — wraps the stream connection (can inspect/modify individual messages via Send/Receive)

### connect-go StreamingHandlerConn (server streaming interface)

```go
type StreamingHandlerConn interface {
    Spec() Spec
    Peer() Peer
    Receive(any) error           // read request message
    RequestHeader() http.Header  // read request headers
    Send(any) error              // write response message
    ResponseHeader() http.Header // write response headers
    ResponseTrailer() http.Header // write response trailers
}
```

Read side (Receive, RequestHeader) may be called concurrently with write side (Send, ResponseHeader, ResponseTrailer), but each side must not be called concurrently with itself.

## Decision

### StreamType

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamType {
    Unary,
    ClientStream,
    ServerStream,
    BidiStream,
}
```

### Spec & Peer

```rust
#[derive(Debug, Clone)]
pub struct Spec {
    pub stream_type: StreamType,
    pub procedure: String,           // e.g. "connectrpc.greet.v1.GreetService/Greet"
    pub is_client: bool,             // true for client-side, false for handler-side
    pub idempotency: IdempotencyLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdempotencyLevel {
    Unknown,
    NoSideEffects,  // safe for HTTP GET
    Idempotent,     // safe to retry
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub addr: String,              // remote address (IP:port on server, host:port on client)
    pub protocol: String,          // "connect", "grpc", "grpcweb"
}
```

### Request & Response Wrappers

```rust
pub struct Request<T> {
    pub msg: T,
    spec: Spec,
    peer: Peer,
    headers: SimpleHeaders,
}

impl<T> Request<T> {
    pub fn new(msg: T) -> Self;
    pub fn spec(&self) -> &Spec;
    pub fn peer(&self) -> &Peer;
    pub fn headers(&self) -> &SimpleHeaders;
    pub fn headers_mut(&mut self) -> &mut SimpleHeaders;
}

pub struct Response<T> {
    pub msg: T,
    headers: SimpleHeaders,
    trailers: SimpleHeaders,
}

impl<T> Response<T> {
    pub fn new(msg: T) -> Self;
    pub fn headers(&self) -> &SimpleHeaders;
    pub fn headers_mut(&mut self) -> &mut SimpleHeaders;
    pub fn trailers(&self) -> &SimpleHeaders;
    pub fn trailers_mut(&mut self) -> &mut SimpleHeaders;
}
```

### RequestContext

Replaces Go's `context.Context` with an explicit struct:

```rust
pub struct RequestContext {
    pub spec: Spec,
    pub peer: Peer,
    pub headers: SimpleHeaders,
    pub deadline: Option<Instant>,
    pub extensions: Extensions,     // type-map for custom middleware data
    canceled: Arc<AtomicBool>,
}

impl RequestContext {
    pub fn is_canceled(&self) -> bool;
    pub fn cancel(&self);
    pub fn remaining_timeout(&self) -> Option<Duration>;
}
```

`Extensions` is a type-map (like `http::Extensions`) that middleware can insert typed data into. The auth middleware inserts the authenticated identity here.

### Handler Traits

```rust
/// Unary RPC handler.
pub trait UnaryHandler<Req, Res>: Send + Sync + 'static {
    fn handle(&self, ctx: &RequestContext, request: Request<Req>) -> Result<Response<Res>, ConnectError>;
}

/// Server streaming RPC handler.
/// Returns an iterator of response messages.
pub trait ServerStreamHandler<Req, Res>: Send + Sync + 'static {
    fn handle(
        &self,
        ctx: &RequestContext,
        request: Request<Req>,
    ) -> Result<(SimpleHeaders, Box<dyn StreamIterator<Res>>), ConnectError>;
    // Returns (response_headers, message_stream)
    // Trailers are sent after the stream completes
}

/// Client streaming RPC handler.
/// Receives an iterator of request messages, returns a single response.
pub trait ClientStreamHandler<Req, Res>: Send + Sync + 'static {
    fn handle(
        &self,
        ctx: &RequestContext,
        headers: &SimpleHeaders,
        requests: Box<dyn StreamIterator<Req>>,
    ) -> Result<Response<Res>, ConnectError>;
}

/// Bidirectional streaming RPC handler.
pub trait BidiStreamHandler<Req, Res>: Send + Sync + 'static {
    fn handle(
        &self,
        ctx: &RequestContext,
        headers: &SimpleHeaders,
        requests: Box<dyn StreamIterator<Req>>,
    ) -> Result<(SimpleHeaders, Box<dyn StreamIterator<Res>>), ConnectError>;
}
```

Where `StreamIterator` wraps foundation_core's progress-driven model:

```rust
/// Iterator over RPC messages, compatible with valtron execution.
pub trait StreamIterator<T>: Send {
    fn next(&mut self) -> Stream<Result<T, ConnectError>, ()>;
    // Returns Stream::Next(Ok(msg)) for each message,
    //         Stream::Next(Err(e)) on error,
    //         Stream::Pending(()) when no data ready yet,
    //         Stream::Init when stream is complete (EOF)
}
```

### Closure-Based Handler Constructors

For ergonomics, provide `handler_fn` constructors like connect-go's `NewUnaryHandler`:

```rust
pub fn unary_handler_fn<Req, Res, F>(f: F) -> impl UnaryHandler<Req, Res>
where
    F: Fn(&RequestContext, Request<Req>) -> Result<Response<Res>, ConnectError> + Send + Sync + 'static;

pub fn server_stream_handler_fn<Req, Res, F>(f: F) -> impl ServerStreamHandler<Req, Res>
where
    F: Fn(&RequestContext, Request<Req>) -> Result<(SimpleHeaders, Box<dyn StreamIterator<Res>>), ConnectError>
        + Send + Sync + 'static;

// ... etc for client_stream_handler_fn, bidi_stream_handler_fn
```

### Interceptor System

```rust
/// Interceptor wraps RPC execution for cross-cutting concerns.
/// Mirrors connect-go's Interceptor interface.
pub trait Interceptor: Send + Sync + 'static {
    /// Wrap a unary RPC function.
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc;

    /// Wrap a streaming client RPC function.
    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc;

    /// Wrap a streaming handler RPC function.
    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc;
}

/// Type-erased unary RPC function for interceptor chaining.
pub type UnaryFunc = Box<dyn Fn(&RequestContext, AnyRequest) -> Result<AnyResponse, ConnectError> + Send + Sync>;

/// Type-erased streaming handler function.
pub type StreamingHandlerFunc = Box<dyn Fn(&RequestContext, &mut dyn StreamingHandlerConn) -> Result<(), ConnectError> + Send + Sync>;

/// Type-erased streaming client function.
pub type StreamingClientFunc = Box<dyn Fn(&RequestContext, &Spec) -> Box<dyn StreamingClientConn> + Send + Sync>;
```

### AnyRequest / AnyResponse (Type-Erased for Interceptors)

```rust
pub trait AnyRequest: Send {
    fn any_ref(&self) -> &dyn Any;
    fn spec(&self) -> &Spec;
    fn peer(&self) -> &Peer;
    fn headers(&self) -> &SimpleHeaders;
    fn headers_mut(&mut self) -> &mut SimpleHeaders;
}

pub trait AnyResponse: Send {
    fn any_ref(&self) -> &dyn Any;
    fn headers(&self) -> &SimpleHeaders;
    fn headers_mut(&mut self) -> &mut SimpleHeaders;
    fn trailers(&self) -> &SimpleHeaders;
    fn trailers_mut(&mut self) -> &mut SimpleHeaders;
}
```

### StreamingHandlerConn

```rust
/// Server's view of a streaming RPC, used by streaming interceptors.
pub trait StreamingHandlerConn: Send {
    fn spec(&self) -> &Spec;
    fn peer(&self) -> &Peer;
    fn request_headers(&self) -> &SimpleHeaders;
    fn receive(&mut self) -> Result<Box<dyn Any + Send>, ConnectError>;
    fn send(&mut self, msg: Box<dyn Any + Send>) -> Result<(), ConnectError>;
    fn response_headers(&self) -> &SimpleHeaders;
    fn response_headers_mut(&mut self) -> &mut SimpleHeaders;
    fn response_trailers(&self) -> &SimpleHeaders;
    fn response_trailers_mut(&mut self) -> &mut SimpleHeaders;
}
```

### Interceptor Chain

```rust
pub struct InterceptorChain {
    interceptors: Vec<Arc<dyn Interceptor>>,
}

impl InterceptorChain {
    pub fn new(interceptors: Vec<Arc<dyn Interceptor>>) -> Self;
    pub fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc;
    pub fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc;
    pub fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc;
}
```

Interceptors are applied in reverse order (first interceptor in the list executes first), matching connect-go's `newChain` behavior.

### UnaryInterceptorFunc (Convenience)

```rust
/// Simple interceptor that only wraps unary RPCs.
pub struct UnaryInterceptorFunc<F>(F);

impl<F> Interceptor for UnaryInterceptorFunc<F>
where
    F: Fn(UnaryFunc) -> UnaryFunc + Send + Sync + 'static,
{
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc { (self.0)(next) }
    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc { next }
    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc { next }
}
```

### Panic Recovery Interceptor

```rust
pub struct RecoverInterceptor<F> {
    handler: F,  // Fn(&RequestContext, &Spec, &SimpleHeaders, Box<dyn Any + Send>) -> ConnectError
}

impl<F> RecoverInterceptor<F> {
    pub fn new(handler: F) -> Self;
}
```

Wraps handler execution in `std::panic::catch_unwind`. On panic, calls the user's handler function which can log, emit metrics, and return a `ConnectError` (typically `Code::Internal`).

## Consequences

- Handler traits use foundation types (`SimpleHeaders`, `RequestContext`) not HTTP types
- Streaming uses `StreamIterator` compatible with valtron's progress model
- Interceptors use type-erased `AnyRequest`/`AnyResponse` matching connect-go's pattern
- `StreamingHandlerConn` provides the same concurrent read/write interface as connect-go
- Panic recovery via interceptor, not built into the framework
- `Extensions` type-map enables middleware to pass typed data through the request pipeline

## Open Questions

1. **Sync vs async handlers**: The handler traits above are synchronous (return `Result` directly). Valtron's model is progress-driven, not async/await. Should handlers be allowed to return `Stream<D, P>` for deferred execution, or always block the worker thread? connect-go handlers block their goroutine.
2. **StreamIterator EOF signaling**: Using `Stream::Init` to signal stream completion is unconventional. Should we use a separate `bool` flag or `Option<Result<T, ConnectError>>` return? Need to verify this matches foundation_core's iterator conventions.
3. **Interceptor overhead for unary**: connect-go applies interceptors once at client/handler creation time (not per-call). We should do the same — wrap the handler function once during registration, not on every request.
4. **Context cancellation propagation**: Go's `context.WithCancel` naturally propagates cancellation. Our `RequestContext.canceled` is a manual `AtomicBool`. How do long-running handlers check for cancellation? Should `StreamIterator::next` automatically return error if context is canceled?
5. **Concurrent read/write for StreamingHandlerConn**: connect-go allows Receive and Send to be called from different goroutines. In our model, the handler runs on a single worker thread. For bidi streaming, do we need to split into separate reader/writer tasks on the valtron executor?
