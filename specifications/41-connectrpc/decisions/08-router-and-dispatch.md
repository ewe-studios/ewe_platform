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
    /// Key: "/package.Service/Method" — WITH the leading slash (R1): the full path standard
    /// clients send; generated constants match (Decision 10).
    handlers: HashMap<String, HandlerEntry>,

    // NOTE: the router holds NO codec state — each procedure's ProcedureCodecs table (the
    // single codec authority, Decision 02) lives inside its erased wrapper, and the entry
    // exposes only the object-safe `ProcedureMeta` view for dispatch (see HandlerEntry).
    // The former global CodecRegistry is deleted.

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
    /// Erased, object-safe view of the procedure's codec metadata (fresh-review A4): the
    /// generic `ProcedureCodecs<Req, Res>` itself lives INSIDE the erased wrapper closures
    /// (it cannot sit in this non-generic struct) — this view is what the type-erased
    /// dispatcher uses for the 415 membership check and Accept-Post construction.
    codec_meta: Arc<dyn ProcedureMeta>,
}

/// Object-safe metadata surface the generic registration wrapper implements over its
/// ProcedureCodecs table. Dispatch-time only — typed resolution never happens here.
pub(crate) trait ProcedureMeta: Send + Sync {
    fn has_codec(&self, name: &str) -> bool;              // 415 gate
    fn codec_names(&self) -> Vec<&str>;                   // Accept-Post / error messages
    fn is_binary(&self, name: &str) -> Option<bool>;      // GET query encoding (Decision 07)
}

enum HandlerKind {
    Unary(Arc<dyn ErasedUnaryHandler>),
    // All three streaming kinds share the byte-level `ErasedStreamHandler` (Decision 11
    // `HandlerConn`); the StreamType in `Spec` distinguishes them for dispatch/validation.
    ServerStream(Arc<dyn ErasedStreamHandler>),
    ClientStream(Arc<dyn ErasedStreamHandler>),
    BidiStream(Arc<dyn ErasedStreamHandler>),
}
```

### Type-Erased Handler Traits

To store handlers of different `Req`/`Res` types in one `HashMap`, the router holds them
**type-erased** — in two styles:

- **Unary** erases to an **async** bytes function `(Ctx, codec name, Bytes) ->
  BoxFuture<ConnectResult<(Bytes, headers, trailers)>>`. The generic registration wrapper
  owns the concrete types: it holds the per-procedure `ProcedureCodecs<Req, Res>` table
  (Decision 02) built at registration, resolves the request's codec name to its
  `Arc<dyn CodecFor<Req>>` / `Arc<dyn CodecFor<Res>>` pair **before** building the future,
  and runs decode → `.await` the async handler → encode itself. Owned args keep the returned future `'static`; typed
  marshal goes through `dyn CodecFor` — no `dyn Any` message.
- **Streaming** erases to the `HandlerConn` seam (Decision 11), whose `send` / `receive`
  carry **codec-encoded frame bytes** — not `dyn Any` messages. The **codec is confined to
  the typed `MessageSource<Req>` / `MessageSink<Res>` facade** the handler holds; the seam
  and its bounded queue carry only frames + metadata. Per-procedure content concerns
  (validation, redaction, transforms) run as **message-middleware hosted by that facade**
  (bytes + metadata, with optional one-time typed decode — Decision 11). Keeping `dyn Any`
  out of the path re-enables zero-copy views and is uniform with the unary byte boundary
  above. (Cross-cutting concerns — auth, logging, metrics — are seam interceptors over
  bytes + metadata; see Decision 04.)

```rust
// Erased handlers are ASYNC (they wrap async handlers, Decision 04) — `handle` returns a
// future the router drives via valtron `from_future` (Decision 00/11). The dispatcher
// passes only the parsed codec NAME (the Content-Type wire token — Decision 05 string
// mechanics; no registry, no `Arc<dyn Codec>` handle); the wrapper resolves its typed
// `Arc<dyn CodecFor<Req/Res>>` pair from its registration-time ProcedureCodecs table
// BEFORE building the returned future, so nothing borrowed is captured.
//
// COMPOSITION ORDER (fresh-review B3): the erased wrapper IS the innermost func
// (decode → handler → encode); the interceptor chain is composed AROUND it once at
// registration (Decision 04 §Decided Details OQ#3). `handle` invokes the pre-composed
// chain, threading the per-call codec name through the call value — `UnaryCall.codec_name`
// for unary, `StreamCall.codec_name` for all three streaming kinds (Decision 04) — the
// chain itself is codec-agnostic and never rebuilt per call.
pub(crate) trait ErasedUnaryHandler: Send + Sync {
    fn handle(&self, ctx: Ctx, codec_name: &str, body: Bytes)
        -> BoxFuture<'static, ConnectResult<(Bytes, SimpleHeaders, SimpleHeaders)>>;
    // -> (encoded_response_bytes, response_headers, response_trailers)
}

/// Server / client / bidi streaming all drive the same byte-level `HandlerConn`
/// (Decision 11): the erased handler is invoked with the conn and pushes/pulls
/// codec-encoded message bytes through it.
pub(crate) trait ErasedStreamHandler: Send + Sync {
    fn handle(&self, ctx: Ctx, codec_name: &str, conn: Box<dyn HandlerConn>)
        -> BoxFuture<'static, ConnectResult<()>>;
}
```

This keeps the router generic-free; the dispatch flow additionally enforces capability
matching (Decision 11) — bidi / gRPC require their transport capabilities, and bidi over
HTTP/1.1 is rejected with `505` (H6).

### Handler Registration

```rust
impl Router {
    pub fn new() -> Self;

    // There are NO user-facing handler traits (Decision 04) — registration takes async
    // fns/closures of the Decision 04 shapes, plus the per-procedure typed codec table
    // (`ProcedureCodecs<Req, Res>`, Decision 02) that the generated register fn builds.
    // Unary shown in full; the three streaming registrations take the matching Decision 04
    // async signatures (their `F` bounds follow the same pattern).
    pub fn unary<Req, Res, F, Fut>(
        &mut self,
        procedure: &str,                    // "/package.Service/Method" (R1, leading slash)
        codecs: ProcedureCodecs<Req, Res>,  // codec name → typed CodecFor pair
        handler: F,
        options: HandlerOptions,
    ) where
        Req: Send + 'static,
        Res: Send + 'static,
        F: Fn(Ctx, Request<Req>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ConnectResult<Response<Res>>> + Send + 'static;

    pub fn server_stream<Req, Res, F>(
        /* async fn(Ctx, Request<Req>) -> ConnectResult<impl Stream<Item = ConnectResult<Res>> + Send + 'static> */
    );
    pub fn client_stream<Req, Res, F>(
        /* async fn(Ctx, impl Stream<Item = ConnectResult<Req>>) -> ConnectResult<Response<Res>> */
    );
    pub fn bidi_stream<Req, Res, F>(
        /* async fn(Ctx, impl Stream<Item = ConnectResult<Req>>)
               -> ConnectResult<impl Stream<Item = ConnectResult<Res>> + Send + 'static> */
    );
}
```

### HandlerOptions

```rust
pub struct HandlerOptions {
    pub interceptors: Vec<Arc<dyn Interceptor>>,
    // No codec field: codecs are installed ONLY through the registration `ProcedureCodecs`
    // parameter (Decision 02); options never install or select codecs server-side —
    // the client picks per request via Content-Type.
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
    pub fn with_compression(self, name: &str, compressor: Arc<dyn Compressor>) -> Self;
    pub fn with_pipe_depth(self, n: usize) -> Self;   // seam pipe depth (Decision 11; default 4)
    pub fn with_recover<F>(self, handler: F) -> Self
    where
        F: Fn(&Ctx, &Spec, &SimpleHeaders, Box<dyn Any + Send>) -> ConnectError + Send + Sync + 'static;
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
   6. Parse codec NAME from Content-Type (string mechanics, Decision 05) → membership
      check via the entry's erased `ProcedureMeta` — miss → 415 (+ Accept-Post from
      `codec_names()`); the TYPED pair is resolved inside the erased wrapper (Decision 02)
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
   │   │   ├── Run handler (async); its response Stream is bridged to the writer queue
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
   │       ├── Run handler (async): request Stream in, response Stream out (bridged via queues)
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
// Generated per service — async fns / Streams (Decision 04/10; default `unimplemented` bodies):
pub trait GreetServiceHandler: Send + Sync + 'static {
    async fn greet(&self, ctx: Ctx, req: Request<GreetRequest>)
        -> ConnectResult<Response<GreetResponse>>;
    // client-streaming: an async Stream of requests in
    async fn greet_group(&self, ctx: Ctx, reqs: impl Stream<Item = ConnectResult<GreetRequest>>)
        -> ConnectResult<Response<GreetGroupResponse>>;
}

// Registration function (generated) — paths carry the leading slash (R1); the generated fn
// also builds each procedure's typed codec table (Decision 02):
pub fn register_greet_service<S: GreetServiceHandler>(router: &mut Router, service: Arc<S>) {
    router.unary(
        "/connectrpc.greet.v1.GreetService/Greet",
        ProcedureCodecs::<GreetRequest, GreetResponse>::defaults(),
        /* wrapper that delegates to service.greet() */,
        HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
    );
    router.client_stream(
        "/connectrpc.greet.v1.GreetService/GreetGroup",
        ProcedureCodecs::<GreetRequest, GreetGroupResponse>::defaults(),
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

## Decided Details

- **R1 — leading slash (decided):** procedure paths and generated constants include the
  leading slash (`/package.Service/Method`); the router matches the full path standard
  clients send. Codegen emits constants with the slash (Decision 10).
- **H5 / H7 — dispatch checks:** the dispatch flow enforces 405 (method not allowed),
  415 (unsupported content-type, and GET-with-body), and 505 (bidi over HTTP/1.1) — the
  last via Decision 11's capability matching.
- **R2 — Unimplemented handler:** generate `Unimplemented<Service>Handler` returning
  `unimplemented` for every method (Decision 10).
- **R6 — conditional options:** support per-procedure option customization via a
  callback that inspects each `Spec`.
- **C7 — default codecs:** codegen emits `ProcedureCodecs::defaults()` (proto + json) per
  procedure; the router itself holds no codec state (Decision 02).
- **Q10 — consumption (decided):** `into_handler(self)` consumes and freezes the router;
  no post-build mutation. Documented.

1. **Path prefix routing (decided):** register the `ConnectRpcHandler` as a **single prefix
   route** in foundation_http and sub-route internally via the `HashMap` (keyed on
   `/package.Service/Method` — leading slash, matching R1). Avoids mutating foundation_http's route tree per procedure and
   matches our native multiplexing.
2. **Middleware ordering (decided):** foundation_http middleware (CORS, TLS, transport
   logging) runs **outside** at the HTTP layer; ConnectRPC **seam interceptors** then
   **facade message-middleware** run **inside** the `ConnectRpcHandler`, closest to the
   handler. Order: `foundation_http mw → ConnectRpcHandler → seam interceptors → facade
   middleware → handler`.
