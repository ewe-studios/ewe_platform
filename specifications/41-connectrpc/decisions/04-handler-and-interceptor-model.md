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
    pub protocol: String,          // "connect", "grpc", "grpc-web" (hyphenated, R11)
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
/// Per-RPC state. OWNED + cheap `Clone`: every non-`Copy` field is Arc-backed internally,
/// so a clone is a handful of refcount bumps — deliberately NOT `Arc<RequestContext>`.
/// After construction the shared parts are immutable; writes are rebuild-and-move (the
/// `with_*` methods on `Ctx`), so two clones can never silently diverge on shared state.
/// The one shared-mutable object is the `CancelSignal` — its own Arc'd type, so every
/// clone of this call observes the same flag.
#[derive(Clone)]
pub struct RequestContext {
    pub spec: Arc<Spec>,
    pub peer: Arc<Peer>,
    pub headers: Arc<SimpleHeaders>,
    pub deadline: Option<Instant>,            // Copy
    pub extensions: Extensions,               // Arc-valued type-map, itself cheap-Clone (Decision 12 §13)
    pub connection: Arc<ConnectionContext>,   // Q13; carried from SimpleIncomingRequest (Decision 12 §13)
    cancel: CancelSignal,
}

impl RequestContext {
    pub fn is_canceled(&self) -> bool;        // sync poll (escape hatch)
    pub async fn cancelled(&self);            // awaitable — race against unrelated work in `select!`
    pub fn remaining_timeout(&self) -> Option<Duration>;
    pub fn cancel_signal(&self) -> &CancelSignal; // the transport holds a clone of this and
                                                  // fires it on RST/close/deadline
}

/// Cancellation is its own shared object (`Arc<AtomicBool>` + parked-waker list inside).
/// RULES (decided): a `Ctx`/`RequestContext` **clone shares** the call's signal — a clone
/// IS the same call. Beyond that, propagation is NEVER implicit: derived work that must
/// not be affected by the call's cancellation **detaches** with a fresh signal
/// (`ctx.with_cancellation(CancelSignal::new())`) — independent workers own their own
/// signal. Linking to a parent is **opt-in and one-way down**:
/// `CancelSignal::linked(&parent)` fires when the parent fires OR when cancelled itself,
/// and local firing never propagates upward.
#[derive(Clone)]
pub struct CancelSignal(/* Arc<{ AtomicBool, parked wakers }> */);
impl CancelSignal {
    pub fn new() -> Self;                          // independent signal
    pub fn linked(parent: &CancelSignal) -> Self;  // opt-in downward propagation
    pub fn cancel(&self);
    pub fn is_canceled(&self) -> bool;
    pub async fn cancelled(&self);
}
```

`Extensions` is a type-map whose values are **`Arc<dyn Any + Send + Sync>`** (netio enabler,
Decision 12 §13), which makes the map itself cheaply clonable — required by the COW write
model below.

**Extensions travel pathway (decided):**

```
AuthMiddleware (HTTP layer) ──insert──► SimpleIncomingRequest.extensions   (plain &mut — the request is owned there)
        ▼
ConnectRpcHandler dispatch ──MOVE (take(), zero-copy, no lock)──► RequestContext.extensions
        ▼                                                          (assembled, then Ctx is constructed)
seam interceptors: write = COW rebuild-and-move — ctx.with_extension(v) clones the map
        ▼            (pointer bumps), inserts, returns a NEW Ctx passed to `next`
handler: reads the final view (get_auth_info / ctx.extension::<T>())
```

Visibility is **downstream-only**: a layer holding an earlier clone does not see later
insertions — data flows toward the handler (the `context.WithValue` shape), never upward.
Pre-construction writers use `&mut` (HTTP middleware, dispatch); post-construction writers
rebuild through the by-value chain; there is no `Arc<Mutex<_>>` anywhere in this path.

### Ctx — the per-call context handle (decided)

`Ctx` is the context parameter every handler and client method receives, **by value, in all
four RPC kinds** (server and client — no `&Ctx` in any signature). It is an **owned,
cheaply-`Clone` value with Arc-backed internals** — deliberately not `Arc<RequestContext>`:
a clone is a handful of refcount bumps and `'static`, which is what lets streaming handlers
return `impl Stream + Send + 'static` (Decision 10 S2; an `async move` just captures a
clone). **Write = rebuild-and-move, read = share:** after construction the shared parts are
immutable; a layer that wants to change something calls a `with_*` method (COW rebuild) and
passes the new `Ctx` forward — the by-value chain makes this natural, and two clones can
never silently diverge on shared state. The only cross-clone mutable object is the
`CancelSignal` (its own Arc'd type, above).

```rust
/// Owned + cheap Clone (refcount bumps only). Passed by value everywhere.
#[derive(Clone)]
pub struct Ctx {
    /// App-scoped shared dependencies — foundation_http's `ContextBag`: DB pools, config,
    /// caches, service singletons, alert/event/remote-trigger facilities. The same bag the
    /// HTTP layer hands `Serve` handlers, so HTTP and RPC handlers share one dependency store.
    pub bag: Arc<ContextBag>,
    /// Per-RPC state (owned, itself cheap-Clone with Arc'd internals — see RequestContext):
    /// `Spec`, `Peer`, request headers, deadline, cancellation, `Extensions`, and the
    /// `ConnectionContext` (Q13).
    pub request: RequestContext,
}

impl Ctx {
    // ── Constructors (base case only — the server-side Ctx is built by dispatch, and the
    //    canonical client call site passes the inbound ctx it already has; these exist for
    //    top-level code with NO inbound context — Decision 07 §contract) ──
    /// Client-side base context: app deps only, empty per-call state.
    pub fn client(bag: Arc<ContextBag>) -> Ctx;
    /// Empty-bag base — tests, CLIs, wasm entry points.
    pub fn background() -> Ctx;

    // ── COW derivations: rebuild-and-move; the original is untouched ──
    pub fn with_deadline(self, d: Duration) -> Ctx;
    pub fn with_extension<T: Send + Sync + 'static>(self, v: T) -> Ctx;
    /// Replace the cancel signal: detach (`CancelSignal::new()`) or opt-in link
    /// (`CancelSignal::linked(&parent)`). Plain `clone()` always SHARES the current signal.
    pub fn with_cancellation(self, signal: CancelSignal) -> Ctx;

    // ── Delegates for the common surface ──
    pub fn spec(&self) -> &Spec;
    pub fn peer(&self) -> &Peer;
    pub fn extension<T: 'static>(&self) -> Option<&T>;
    pub fn is_canceled(&self) -> bool;
    pub async fn cancelled(&self);
    pub fn remaining_timeout(&self) -> Option<Duration>;
    pub fn cancel_signal(&self) -> &CancelSignal;
}
```

Handlers reach shared services via `ctx.bag.get::<DbPool>()` and RPC state via `ctx.request`
(or the delegates). Interceptor fn-types take the same `Ctx` by value.

**Why owned-`Clone` and not `Arc<RequestContext>` (recorded):** the write phase is linear
(middleware → dispatch → interceptors → handler), but the read phase fans out concurrently —
the handler future / its returned `'static` stream, the framework's reader/writer tasks,
interceptor epilogues (code after `next(ctx).await`), and the transport's cancel path all
hold the context at once. Owned-`Clone` with Arc'd internals serves that fan-out at
refcount cost while keeping move semantics for writes; the shared *mutable* surface is
confined to `CancelSignal` by construction.

### Handler API (async functions / streams)

Handlers are **plain async Rust** — `async fn`s and futures `Stream`s. None of the valtron
machinery (`TaskStatus` tasks, the `ConcurrentQueue` seam, the `Stream<D,P>` boundary)
appears in handler code: valtron's `from_future` / `from_stream` bridge turns the user's
async into tasks, and each `.await` is the yield point — async/await *is* the state machine
the executor drives. The internal wiring is in `11-transport-seam.md`.

```rust
// unary — ctx BY VALUE (Ctx is owned + cheap-Clone, §Ctx above); errors are ConnectResult (Decision 03)
async fn greet(&self, ctx: Ctx, req: Request<GreetReq>)
    -> ConnectResult<Response<GreetRes>>;

// server streaming — return an async Stream of responses
async fn list(&self, ctx: Ctx, req: Request<ListReq>)
    -> ConnectResult<impl Stream<Item = ConnectResult<File>>>;

// client streaming — consume an async Stream, return one response
async fn upload(&self, ctx: Ctx, reqs: impl Stream<Item = ConnectResult<Chunk>>)
    -> ConnectResult<Response<UploadRes>>;

// bidi — async Stream in, async Stream out (interleave with .await)
async fn echo(&self, ctx: Ctx, reqs: impl Stream<Item = ConnectResult<EchoReq>>)
    -> ConnectResult<impl Stream<Item = ConnectResult<EchoRes>>>;
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
    Arc<dyn Fn(Ctx, UnaryCall) -> BoxFuture<'static, ConnectResult<UnaryReply>> + Send + Sync>;

// `codec_name` rides the call because the interceptor chain is composed ONCE at
// registration (§Decided Details OQ#3) while the codec is negotiated per request — the innermost typed
// wrapper reads it here to resolve its ProcedureCodecs pair (Decisions 02/08).
pub struct UnaryCall  { pub headers: SimpleHeaders, pub codec_name: String, pub frame: Bytes } // encoded request
pub struct UnaryReply { pub headers: SimpleHeaders, pub trailers: SimpleHeaders, pub frame: Bytes } // encoded response

/// Streaming handler interceptor — async; wraps the byte-level `HandlerConn` **by value** so
/// it can embed/wrap it (Decision 11; `&mut dyn` would block injecting a wrapper).
/// `StreamCall` is the streaming mirror of `UnaryCall`: the chain is composed once at
/// registration, so the per-request codec name must ride the call (same reasoning as B3).
pub type StreamingHandlerFunc =
    Arc<dyn Fn(Ctx, StreamCall) -> BoxFuture<'static, ConnectResult<()>> + Send + Sync>;

pub struct StreamCall { pub codec_name: String, pub conn: Box<dyn HandlerConn> }

/// Streaming client interceptor — async like the others (opening a conn does I/O via
/// `Transport::open`, Decision 11); wraps the byte-level `ClientConn`.
pub type StreamingClientFunc =
    Arc<dyn Fn(Ctx, Spec) -> BoxFuture<'static, ConnectResult<Box<dyn ClientConn>>> + Send + Sync>;
```

The fn-types are **future-returning** (`BoxFuture`), not sync `-> Result`: the innermost
`UnaryFunc` is the async handler invocation, so a sync closure could only call it via
`block_on` (forbidden). Owned args (`Ctx`/`Spec` by value, `Bytes` frames) keep
the returned future `'static` so it composes and spawns (see S2 / Decision 00).

No `AnyRequest` / `AnyResponse`: the seam is bytes + metadata. Per-procedure typed access is
the facade message-middleware (Decision 11). This removes RS2's `dyn Any` `'static`
constraint (zero-copy views become possible) and the RS5 / RS9 `Any` issues.

### Streaming interceptor conn

The streaming interceptor's view of a connection **is** Decision 11's `HandlerConn`, which
carries **encoded frames (bytes) + metadata**, not typed messages. Its surface (Decision 11
is normative): metadata accessors `spec` / `peer` / `request_headers` on the unsplit conn,
plus `split()` into the halves that move frames — `ConnReceiver::receive` → `Bytes`, and
`ConnSender::send` ← `Bytes` / `send_headers` / `close(error, trailers)` (server-side
trailers are an argument to `close`, not an accessor). Its frame-moving methods are **async** (`BoxFuture`, dyn-safe —
Decision 11), so a wrapping interceptor `.await`s the inner conn; waiting is expressed by
the future parking, never by a "not ready" return value. Interceptors wrap the **unsplit**
conn; the framework later calls `split()` (Decision 11) to obtain the receiver/sender
halves — a wrapper's `split` wraps the inner halves, so interception survives the split
and sits on both directions of the frame path. It is passed **by value** (`Box<dyn HandlerConn>`, see
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
    handler: F,  // Fn(&Ctx, &Spec, &SimpleHeaders, Box<dyn Any + Send>) -> ConnectError
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
  `ctx.request.connection.peer`, `ctx.request.connection.tls`, …) while keeping `extensions`
  for untyped user/middleware data. *The netio-side plumbing — defining `ConnectionContext`
  and carrying it on `SimpleIncomingRequest` — is a foundation enabler tracked in
  **Decision 12 §13**.* Connection-scoped fields are shared across multiplexed HTTP/2 and
  HTTP/3 requests on the same connection; the per-request HTTP/2/3 **stream id** stays
  request-scoped. (Folds in T10 — iroh public key — and the netcap `Endpoint<I>` identity
  generic from Decision 12 §9.)

## Decided Details

- **OQ#3 — interceptor composition (decided):** Interceptors wrap the handler function
   **once at registration**, not per-call (connect-go parity) — the composed call chain is
   built at handler-creation time.
- **Q4 — cancellation propagation (decided: Option B + valtron poll-tree).** The
   transport pushes cancellation (conn close / RST_STREAM / QUIC reset / deadline) by firing
   the call's `CancelSignal` — it holds a clone of the same signal every `Ctx` clone of the
   call shares (§Ctx rules; Decision 11). On cancel the request `Stream` terminates
   (`next().await` → `None`/`Err(Canceled)`) and `send` returns `Err(Canceled)`, so the
   idiomatic handler loop unwinds at its own `.await` points — no manual checks. Escape
   hatches for awaits on *unrelated* work: `ctx.is_canceled()` (sync poll) and
   `ctx.cancelled().await` (race via `select!`). The window is bounded several ways:
   (a) once any unrelated await returns, the **next** stream `send`/`receive` fails with
   `Canceled`; and (b) inline-awaited work is cancelled by **DROPPING** the future/stream
   (destructors run, releasing pipe slots / fd registrations / buffers — merely *not
   polling* would leak them and is NOT a cancellation mechanism), which cancellation
   propagates down the ownership tree of nested awaited futures; work spawned as a
   **separate valtron task** is not owned by the dropped future, so it observes the shared
   `CancelSignal` itself — its pipe awaits/parks already compose the signal (Decision 11
   §Cancellation), and it terminates cooperatively (the only un-cancellable case is work
   already escaped onto a blocking background thread, as everywhere). Unary = the framework
   drops the handler future (cancel-by-drop).
