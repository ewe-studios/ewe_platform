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
    pub fn is_canceled(&self) -> bool;        // sync poll (escape hatch)
    pub fn cancel(&self);                      // transport calls this on RST/close/deadline
    pub async fn cancelled(&self);             // awaitable — race against unrelated work in `select!`
    pub fn remaining_timeout(&self) -> Option<Duration>;
}
```

`Extensions` is a type-map (like `http::Extensions`) that middleware can insert typed data into. The auth middleware inserts the authenticated identity here.

### Handler API (async functions / streams)

Handlers are **plain async Rust** — `async fn`s and futures `Stream`s. None of the valtron
machinery (`TaskStatus` tasks, the `ConcurrentQueue` seam, the `Stream<D,P>` boundary)
appears in handler code: valtron's `from_future` / `from_stream` bridge turns the user's
async into tasks, and each `.await` is the yield point — async/await *is* the state machine
the executor drives. The internal wiring is in `11-transport-seam.md`.

```rust
// unary
async fn greet(&self, ctx: &Ctx, req: Request<GreetReq>)
    -> Result<Response<GreetRes>, ConnectError>;

// server streaming — return an async Stream of responses
async fn list(&self, ctx: &Ctx, req: Request<ListReq>)
    -> Result<impl Stream<Item = Result<File, ConnectError>>, ConnectError>;

// client streaming — consume an async Stream, return one response
async fn upload(&self, ctx: &Ctx, reqs: impl Stream<Item = Result<Chunk, ConnectError>>)
    -> Result<Response<UploadRes>, ConnectError>;

// bidi — async Stream in, async Stream out (interleave with .await)
async fn echo(&self, ctx: &Ctx, reqs: impl Stream<Item = Result<EchoReq, ConnectError>>)
    -> Result<impl Stream<Item = Result<EchoRes, ConnectError>>, ConnectError>;
```

The same signatures hold on every transport (HTTP/1.1 / HTTP/2 / HTTP/3); **duplex is a
scheduling property** of how the framework drives the two internal seam queues, not the API.

**Threading:** futures crossing the multi-threaded pool are `Send + 'static` (standard for
async-on-threadpool); valtron also exposes a non-`Send` / local executor path, and
single-threaded / wasm code can wrap non-`Send` state in `SendWrapper` to satisfy the bound.
So `!Send` handler state is never a blocker.

There are **no** user-facing `MessageSink` / `MessageSource` / handler traits — those are
internal adapters over the seam queues (Decision 11). Generated service traits (Decision 10)
express each method as an `async fn` of exactly these shapes; the codec/seam are internal.
`handler_fn`-style constructors, where used, simply take an `async` fn/closure of the
matching shape.

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

/// Unary interceptor function — **async** (returns a future), because the call it wraps is
/// an async handler. Metadata + encoded request/response **frames** (no typed message;
/// interceptors needing the message register as facade middleware, Decision 11).
pub type UnaryFunc =
    Arc<dyn Fn(RequestContext, UnaryCall) -> BoxFuture<'static, Result<UnaryReply, ConnectError>> + Send + Sync>;

pub struct UnaryCall  { pub headers: SimpleHeaders, pub frame: Bytes }   // encoded request
pub struct UnaryReply { pub headers: SimpleHeaders, pub trailers: SimpleHeaders, pub frame: Bytes } // encoded response

/// Streaming handler interceptor — async; wraps the byte-level `HandlerConn` **by value** so
/// it can embed/wrap it (Decision 11; `&mut dyn` would block injecting a wrapper).
pub type StreamingHandlerFunc =
    Arc<dyn Fn(RequestContext, Box<dyn HandlerConn>) -> BoxFuture<'static, Result<(), ConnectError>> + Send + Sync>;

/// Streaming client interceptor — wraps the byte-level `ClientConn`.
pub type StreamingClientFunc =
    Arc<dyn Fn(RequestContext, Spec) -> Box<dyn ClientConn> + Send + Sync>;
```

The fn-types are **future-returning** (`BoxFuture`), not sync `-> Result`: the innermost
`UnaryFunc` is the async handler invocation, so a sync closure could only call it via
`block_on` (forbidden). Owned args (`RequestContext`/`Spec` by value, `Bytes` frames) keep
the returned future `'static` so it composes and spawns (see S2 / Decision 00).

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

- Handlers are async fns/`Stream`s; valtron drives them via `from_future`/`from_stream` — the `TaskStatus`/`Stream`/queue machinery is internal (Decision 11)
- Interceptors are **seam-level over bytes + metadata**; per-procedure typed concerns are facade message-middleware — there is no `AnyRequest`/`AnyResponse` `dyn Any` boundary
- The streaming interceptor conn is Decision 11's byte-level `HandlerConn` (passed by value), not a separate `StreamingHandlerConn`
- Panic recovery via interceptor, not built into the framework
- `Extensions` type-map enables middleware to pass typed data through the request pipeline

## Additional Decisions

Smaller decided items (the async handler model, interceptor fn-types, and by-value conn are
already in the body above):

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

1. **Sync vs async handlers — resolved.** Handlers are **async fns / async `Stream`s** (the Handler API above). valtron drives them via `from_future`/`from_stream`; `.await` is the yield point and nothing blocks a worker (Decision 11).
2. **StreamIterator EOF signaling — resolved.** EOF is `Iterator::next() == None` / `receive() -> Ok(None)`. `Stream::Init` means *initializing*, never completion (Decision 11). (Moot anyway — handlers are async, not raw `StreamIterator`s.)
3. **Interceptor overhead for unary**: connect-go applies interceptors once at client/handler creation time (not per-call). We should do the same — wrap the handler function once during registration, not on every request.
4. **Context cancellation propagation — resolved (Option B + valtron poll-tree).** The
   transport pushes cancellation (conn close / RST_STREAM / QUIC reset / deadline) into
   `RequestContext.cancel()` (Decision 11). On cancel the request `Stream` terminates
   (`next().await` → `None`/`Err(Canceled)`) and `send` returns `Err(Canceled)`, so the
   idiomatic handler loop unwinds at its own `.await` points — no manual checks. Escape
   hatches for awaits on *unrelated* work: `ctx.is_canceled()` (sync poll) and
   `ctx.cancelled().await` (race via `select!`). The window is bounded several ways:
   (a) once any unrelated await returns, the **next** stream `send`/`receive` fails with
   `Canceled`; and (b) because valtron drives awaited work by **polling**, the wrapping task
   can observe the cancel signal and simply **stop polling the inner future/stream** — so
   cancellation propagates down the poll tree into nested awaited tasks too (the only
   un-cancellable case is work already escaped onto a blocking background thread, as
   everywhere). Unary = the framework drops the handler future (cancel-by-drop).
5. **Concurrent read/write — resolved.** Bidi is reader-task → `request_q` → async handler → `response_q` → writer-task; reader and writer are independent valtron tasks (full-duplex on HTTP/2/3), so send and receive progress concurrently with no thread management in the handler (Decision 11).
