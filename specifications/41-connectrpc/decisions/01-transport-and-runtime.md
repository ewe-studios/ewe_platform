# Decision 01: Transport Layer & Runtime Model

## Context

ConnectRPC is an HTTP-based RPC framework supporting three wire protocols (Connect, gRPC, gRPC-Web) over HTTP/1.1 and HTTP/2. The reference connect-go implementation builds directly on Go's `net/http`, which provides a unified HTTP/1.1+HTTP/2 server and client with goroutine-per-request concurrency.

Our platform uses:
- **foundation_http** — Connection-owned, worker-pooled HTTP server with synchronous handler dispatch
- **foundation_netio** — Custom HTTP types (`SimpleIncomingRequest`, `SimpleOutgoingResponse`, `SimpleMethod`, `SimpleHeaders`, `SimpleBody`/`SendSafeBody`), TCP/TLS, WebSocket, SSE
- **foundation_core** — Valtron: a progress-driven async model using `Stream<D, P>` enum (Init/Ignore/Delayed/Pending/Next) with `ConcurrentQueueStreamIterator`, single/multi executor pools, WASM-compatible

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

- **Request body reading**: `SimpleBody::Stream` provides `Iterator<Item = Result<Vec<u8>, BoxedError>>`. The envelope decoder wraps this iterator, yielding decoded messages.
- **Response body writing (streaming)**: Handler returns an iterator/stream of response messages. The envelope encoder wraps each message in the 5-byte framing header and yields chunks via `SendSafeBody::ChunkedStream`.
- **Bidi streaming**: Requires concurrent read (from request body iterator) and write (to response body stream). Valtron executor manages both sides. On HTTP/1.1, this is half-duplex (request fully consumed before response begins). On HTTP/2, this is full-duplex.

### HTTP/2 Support

gRPC protocol requires HTTP/2. Current foundation_netio supports HTTP/1.1 natively. For HTTP/2:

**Option A — h2 crate behind feature gate**: Add optional `h2` dependency for native HTTP/2 server/client. The protocol layer doesn't change; only the transport framing changes.

**Option B — Delegate HTTP/2 to external proxy**: Support Connect protocol (HTTP/1.1 capable) and gRPC-Web (HTTP/1.1 capable) natively. gRPC over HTTP/2 handled by fronting with an Envoy/nginx proxy that translates.

**Chosen: Option A with incremental delivery.** Phase 1 delivers Connect + gRPC-Web over HTTP/1.1 (covers browsers, curl, most clients). Phase 2 adds HTTP/2 for native gRPC. The protocol abstraction layer is protocol-agnostic from day one so adding HTTP/2 transport doesn't change handler code.

### WASM Compatibility

The core protocol logic (codec, compression, envelope framing, error types, interceptors) must compile to `wasm32-unknown-unknown`. Transport-specific code (TCP sockets, TLS, HTTP/2 frames) is `#[cfg(not(target_arch = "wasm32"))]` gated. WASM clients use browser Fetch API via foundation_wasm bridges.

## Consequences

- Handlers receive `SimpleIncomingRequest` / produce `SimpleOutgoingResponse` — no new HTTP type system
- Streaming uses foundation's iterator model, not async streams — simpler but requires careful buffer management
- gRPC (HTTP/2) is a Phase 2 transport addition, not a protocol-layer concern
- Connect + gRPC-Web work on HTTP/1.1 from day one
- Valtron executor handles concurrent stream processing for bidi RPCs
- WASM client support is possible because core protocol logic has no OS dependencies

## Open Questions

1. **foundation_netio HTTP/2**: Does foundation_netio have any HTTP/2 frame-level support today, or is it purely HTTP/1.1? If none, we need to assess the `h2` crate integration effort.
2. **Chunked transfer encoding**: Connect streaming over HTTP/1.1 requires chunked transfer encoding. Does foundation_http's response writer handle chunked encoding automatically, or do we need to manage `Transfer-Encoding: chunked` headers and chunk boundaries manually?
3. **Backpressure**: connect-go uses `io.Pipe` which naturally provides backpressure (writer blocks when reader is slow). Foundation's iterator model doesn't block — do we need a bounded channel between encoder and transport writer?
4. **Half-duplex bidi over HTTP/1.1**: connect-go supports this by fully consuming the request body before sending the response. Foundation_http's worker model already does this (read request, produce response). Confirm this is the case.
