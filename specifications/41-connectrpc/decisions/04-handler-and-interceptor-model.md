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

Unary handlers return a `Response`; streaming handlers use the **push model**: they
receive a `MessageSource<Req>` (pull request messages) and/or a `MessageSink<Res>` (push
response messages) and may interleave reads and writes freely. The signatures are
identical on every transport (HTTP/1.1 / HTTP/2 / HTTP/3) — duplex is a scheduling
property of the transport, not the API. `MessageSource` / `MessageSink` are bounded pipes
over valtron's `ConcurrentQueueStreamIterator`; end-of-stream is `receive() -> Ok(None)`.
They are defined in `11-transport-seam.md` (the transport seam), which this document
builds on.

```rust
/// Unary RPC handler.
pub trait UnaryHandler<Req, Res>: Send + Sync + 'static {
    fn handle(&self, ctx: &RequestContext, request: Request<Req>)
        -> Result<Response<Res>, ConnectError>;
}

/// Server streaming — handler pushes response messages, sets headers/trailers as it goes.
pub trait ServerStreamHandler<Req, Res>: Send + Sync + 'static {
    fn handle(&self, ctx: &RequestContext, request: Request<Req>, responses: MessageSink<Res>)
        -> Result<(), ConnectError>;
}

/// Client streaming — handler pulls request messages, returns one response.
/// Request headers come from `requests.headers()` (not a separate parameter).
pub trait ClientStreamHandler<Req, Res>: Send + Sync + 'static {
    fn handle(&self, ctx: &RequestContext, requests: MessageSource<Req>)
        -> Result<Response<Res>, ConnectError>;
}

/// Bidirectional streaming — handler holds both halves and may interleave reads/writes.
pub trait BidiStreamHandler<Req, Res>: Send + Sync + 'static {
    fn handle(&self, ctx: &RequestContext, requests: MessageSource<Req>, responses: MessageSink<Res>)
        -> Result<(), ConnectError>;
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
    F: Fn(&RequestContext, Request<Req>, MessageSink<Res>) -> Result<(), ConnectError>
        + Send + Sync + 'static;

// client_stream_handler_fn: Fn(&RequestContext, MessageSource<Req>) -> Result<Response<Res>, _>
// bidi_stream_handler_fn:   Fn(&RequestContext, MessageSource<Req>, MessageSink<Res>) -> Result<(), _>
```

### Interceptor System

Two extension points, split by where the concrete type is known:

1. **Seam interceptors** — cross-cutting, generic, over **bytes + metadata** (`Spec`, `Peer`,
   headers/trailers, frame sizes, status, timing — never the decoded message). Auth,
   logging, metrics, tracing, rate-limit, timeouts. Registered once, applied to every
   procedure. These are the `Interceptor` trait below.
2. **Facade message-middleware** — per-procedure content concerns (validation, redaction,
   defaulting, transforms), hosted by the typed `MessageSink`/`MessageSource` facade where
   the codec + type are known (Decision 11). Operates on frame bytes + metadata with an
   optional one-time typed decode. Replaces connect-go's `any`-message interceptor path,
   which can't work generically in Rust (no message reflection).

There is intentionally **no `dyn Any` boundary** — `AnyRequest`/`AnyResponse` are removed.

```rust
/// Seam interceptor — wraps RPC execution for cross-cutting concerns (bytes + metadata).
/// Mirrors connect-go's Interceptor interface.
pub trait Interceptor: Send + Sync + 'static {
    /// Wrap a unary RPC function.
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc;

    /// Wrap a streaming client RPC function.
    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc;

    /// Wrap a streaming handler RPC function.
    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc;
}

/// Unary interceptor function — metadata + encoded request/response **frames** (no typed
/// message; interceptors needing the message register as facade middleware, Decision 11).
pub type UnaryFunc =
    Box<dyn Fn(&RequestContext, UnaryCall) -> Result<UnaryReply, ConnectError> + Send + Sync>;

pub struct UnaryCall  { pub headers: SimpleHeaders, pub frame: Vec<u8> }   // encoded request
pub struct UnaryReply { pub headers: SimpleHeaders, pub trailers: SimpleHeaders, pub frame: Vec<u8> } // encoded response

/// Streaming handler interceptor — wraps the byte-level `HandlerConn` **by value** so it
/// can embed/wrap it (Decision 11; `&mut dyn` would block injecting a wrapper).
pub type StreamingHandlerFunc =
    Box<dyn Fn(&RequestContext, Box<dyn HandlerConn>) -> Result<(), ConnectError> + Send + Sync>;

/// Streaming client interceptor — wraps the byte-level `ClientConn`.
pub type StreamingClientFunc =
    Box<dyn Fn(&RequestContext, &Spec) -> Box<dyn ClientConn> + Send + Sync>;
```

No `AnyRequest` / `AnyResponse`: the seam is bytes + metadata. Per-procedure typed access is
the facade message-middleware (Decision 11). This removes RS2's `dyn Any` `'static`
constraint (zero-copy views become possible) and the RS5 / RS9 `Any` issues.

### Streaming interceptor conn

The streaming interceptor's view of a connection **is** Decision 11's `HandlerConn`, which
carries **encoded frames (bytes) + metadata** (`spec` / `peer` / `request_headers` /
`receive` → `Vec<u8>` / `send` ← `&[u8]` / `send_headers` / `response_trailers` / `close`),
not typed messages. It is passed **by value** (`Box<dyn HandlerConn>`, see
`StreamingHandlerFunc` above) so an interceptor can wrap or embed it — `&mut dyn` would
prevent injecting a wrapper. There is no separate `StreamingHandlerConn` type; it was
unified into `HandlerConn`. Interceptors that need the typed message register as facade
message-middleware (Decision 11) instead.

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

## Additional Decisions

Smaller decided items (the streaming handler shape, push model, interceptor fn-types, and
by-value conn are already in the body above):

- **RS4 — panic recovery:** wrap handler invocation in `AssertUnwindSafe`; treat the
  connection as poisoned after a caught panic.
- **RS5 / RS9 — moot:** `AnyRequest` / `AnyResponse` are removed (byte+metadata seam), so
  the sealed-`Any` and `Any` `Send`/`Sync` concerns no longer apply.
- **H4 — unary cardinality:** receive exactly one message; zero or more than one →
  `unimplemented`.
- **H14 — `Spec.schema`:** add an optional schema/descriptor handle for interceptors and
  dynamic message construction.
- **H15 — `Peer.query`:** add a server-side query map (for GET RPCs).
- **H16 — `Request::http_method()`:** expose GET vs POST.
- **H18 / H19 — recover/thunk asymmetry:** `RecoverInterceptor` wraps unary +
  streaming-handler only (never streaming-client); the thunk sentinel is client-side only.
- **Q3 — moot:** there is no `AnyRequest`/`any_ref()` boundary anymore; the seam is bytes,
  the typed message lives only in the facade. (Was: "does `any_ref` return `&T` or
  `&Request<T>`" — no longer applicable.)
- **Q13 — connection metadata (decided): typed `ConnectionContext`.** Introduce a typed
  `ConnectionContext` for connection-scoped state — peer identity (incl. the iroh Ed25519
  public key, carried via netcap `Endpoint<I>`), TLS/mTLS peer certificate, negotiated
  ALPN / HTTP version, 0-RTT flag, and QUIC connection id. `SimpleIncomingRequest` carries
  the `ConnectionContext`, and `RequestContext` references it (handlers reach
  `ctx.connection.peer`, `ctx.connection.tls`, …) while keeping `extensions` for untyped
  user/middleware data. Connection-scoped fields are shared across multiplexed HTTP/2 and
  HTTP/3 requests on the same connection; the per-request HTTP/2/3 **stream id** stays
  request-scoped. (Folds in T10 — iroh public key — and the netcap `Endpoint<I>` identity
  generic from Decision 12 §9.)

## Open Questions

1. **Sync vs async handlers**: The handler traits above are synchronous (return `Result` directly). Valtron's model is progress-driven, not async/await. Should handlers be allowed to return `Stream<D, P>` for deferred execution, or always block the worker thread? connect-go handlers block their goroutine.
2. **StreamIterator EOF signaling**: Using `Stream::Init` to signal stream completion is unconventional. Should we use a separate `bool` flag or `Option<Result<T, ConnectError>>` return? Need to verify this matches foundation_core's iterator conventions.
3. **Interceptor overhead for unary**: connect-go applies interceptors once at client/handler creation time (not per-call). We should do the same — wrap the handler function once during registration, not on every request.
4. **Context cancellation propagation**: Go's `context.WithCancel` naturally propagates cancellation. Our `RequestContext.canceled` is a manual `AtomicBool`. How do long-running handlers check for cancellation? Should `StreamIterator::next` automatically return error if context is canceled?
5. **Concurrent read/write for StreamingHandlerConn**: connect-go allows Receive and Send to be called from different goroutines. In our model, the handler runs on a single worker thread. For bidi streaming, do we need to split into separate reader/writer tasks on the valtron executor?
