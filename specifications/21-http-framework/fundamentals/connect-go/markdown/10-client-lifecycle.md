# connect-go — Client Lifecycle

**Source:** `client.go` (393 LOC), `duplex_http_call.go` (482 LOC), `connect.go` (500 LOC). The `Client[Req, Res]` generic type is the client-side RPC endpoint for a single procedure. It uses `duplexHTTPCall` for full-duplex HTTP streaming, supports all four RPC kinds (unary, client stream, server stream, bidi), and applies interceptors at creation time for zero per-call overhead.

## Client Structure

```go
// client.go:34
type Client[Req, Res any] struct {
    config         *clientConfig
    callUnary      func(context.Context, *Request[Req]) (*Response[Res], error)
    protocolClient protocolClient
    err            error  // deferred initialization error
}
```

The `err` field stores any error from client creation (e.g., invalid URL, unknown compression). This error is checked on every call method, deferring failure to call time rather than construction time.

## Core Types

These four types form the foundational data model for connect-go RPCs. They give interceptors and user code access to protocol-level details without requiring knowledge of which specific protocol (Connect, gRPC, or gRPC-Web) is in use.

### Peer

`Peer` represents the remote endpoint of an RPC connection. It is returned by `Peer()` on both `Request` and `StreamingClientConn` / `StreamingHandlerConn`.

```go
// connect.go:353
type Peer struct {
    Addr     string     // host:port (client-side) or IP:port (server-side)
    Protocol string     // "connect", "grpc", or "grpcweb"
    Query    url.Values // server-only: request query parameters
}
```

| Field | Type | Description |
|-------|------|-------------|
| `Addr` | `string` | Remote address. Client-side: the host or `host:port` from the server URL. Server-side: the client's address in `IP:port` format. |
| `Protocol` | `string` | Protocol name — one of `ProtocolConnect` (`"connect"`), `ProtocolGRPC` (`"grpc"`), or `ProtocolGRPCWeb` (`"grpcweb"`). |
| `Query` | `url.Values` | Query parameters. Only populated server-side; unset on the client. |

`Peer` is used in `CallInfo` for logging and auditing which protocol handled a request, and at what address.

### Spec

`Spec` describes an RPC procedure — the "what" of the call, independent of transport details.

```go
// connect.go:333
type Spec struct {
    Procedure        string
    StreamType       StreamType
    IsClient         bool
    IdempotencyLevel IdempotencyLevel
    Schema           any
}
```

| Field | Type | Description |
|-------|------|-------------|
| `Procedure` | `string` | Fully-qualified procedure name, e.g., `"/acme.foo.v1.FooService/Bar"`. |
| `StreamType` | `StreamType` | One of `StreamTypeUnary` (`0b00`), `StreamTypeClient` (`0b01`), `StreamTypeServer` (`0b10`), or `StreamTypeBidi` (`0b11`). |
| `IsClient` | `bool` | `true` for client-side specs, `false` for server-side (handler) specs. |
| `IdempotencyLevel` | `IdempotencyLevel` | One of `IdempotencyUnknown` (`0`), `IdempotencyNoSideEffects` (`1`), or `IdempotencyIdempotent` (`2`). Affects retry safety and whether GET requests are allowed. |
| `Schema` | `any` | Optional schema — for protobuf RPCs, a `protoreflect.MethodDescriptor`. |

`Spec` is used for routing, validation, and interceptor decisions. Protocol implementations use `IdempotencyLevel` to determine if a unary request can be sent as HTTP GET (`IdempotencyNoSideEffects`), and `StreamType` to configure the transport for streaming vs. unary mode.

### Request[Msg]

`Request[Msg]` is a typed request wrapper that bundles the message with metadata.

```go
// connect.go:165
type Request[T any] struct {
    Msg    *T
    spec   Spec
    peer   Peer
    header http.Header
    method string
}
```

| Method | Return | Description |
|--------|--------|-------------|
| `Msg` | `*Msg` | The typed protobuf/JSON message. |
| `Header()` | `http.Header` | Request headers. Lazily initialized on first access. |
| `Spec()` | `Spec` | The procedure specification (stream type, procedure name, etc.). |
| `Peer()` | `Peer` | The remote peer (address, protocol). |
| `HTTPMethod()` | `string` | The HTTP method used — `POST` or `GET` (for side-effect-free unary RPCs). Set lazily after the request is actually sent. |

`Request` is used in unary and server-stream handlers. The `HTTPMethod()` method returns the empty string until the request is actually transmitted, which means client interceptors may see `""` until they delegate to the inner handler.

### Response[Msg]

`Response[Msg]` is a typed response wrapper with metadata.

```go
// connect.go:255
type Response[T any] struct {
    Msg     *T
    header  http.Header
    trailer http.Header
}
```

| Method | Return | Description |
|--------|--------|-------------|
| `Msg` | `*Msg` | The typed protobuf/JSON message. |
| `Header()` | `http.Header` | Response headers. Lazily initialized on first access. |
| `Trailer()` | `http.Header` | Response trailers — may be sent as HTTP trailers or protocol-specific in-body metadata depending on the RPC protocol. |

`Response` is used in unary and client-stream handlers.

### Type Relationships

These four types work together in every RPC:

```
Spec ───────── describes the procedure (what, how)
  │
Peer ───────── describes the remote (who, which protocol)
  │
Request[Msg] ─ wraps the inbound message with metadata (headers, spec, peer, method)
  │
Response[Msg] ─ wraps the outbound message with metadata (headers, trailers)
```

- `Spec` defines **what** procedure is being called and **how** (stream type, idempotency).
- `Peer` defines **who** the remote party is and **which protocol** (Connect/gRPC/gRPC-Web).
- `Request` wraps the inbound message, giving access to headers, the spec, and peer info.
- `Response` wraps the outbound message, giving access to response headers and trailers.

> **Aha:** These types exist to give interceptors and user code access to protocol-level details (headers, trailers, peer info, procedure spec) without requiring knowledge of which specific protocol is in use. A unary interceptor can read `request.Peer().Protocol` to log whether the call went over Connect or gRPC, or check `request.Spec().IdempotencyLevel` to decide whether to retry — all without any protocol-specific code. The abstraction is protocol-agnostic: the same `Request` and `Response` types work whether the wire format is Connect's JSON-enveloped messages, gRPC's length-prefixed protobuf, or gRPC-Web's multipart responses.

## NewClient Initialization

```go
// client.go:42
func NewClient[Req, Res any](httpClient HTTPClient, url string, options ...ClientOption) *Client[Req, Res] {
    client := &Client[Req, Res]{}
    config, err := newClientConfig(url, options)
    if err != nil {
        client.err = err
        return client
    }
    client.config = config

    protocolClient, protocolErr := client.config.Protocol.NewClient(&protocolClientParams{
        CompressionName:  config.RequestCompressionName,
        CompressionPools: newReadOnlyCompressionPools(config.CompressionPools, config.CompressionNames),
        Codec:            config.Codec,
        Protobuf:         config.protobuf(),
        CompressMinBytes: config.CompressMinBytes,
        HTTPClient:       httpClient,
        URL:              config.URL,
        BufferPool:       config.BufferPool,
        ReadMaxBytes:     config.ReadMaxBytes,
        SendMaxBytes:     config.SendMaxBytes,
        EnableGet:        config.EnableGet,
        GetURLMaxBytes:   config.GetURLMaxBytes,
        GetUseFallback:   config.GetUseFallback,
    })
    if protocolErr != nil {
        client.err = protocolErr
        return client
    }
    client.protocolClient = protocolClient
```

### Unary Function Pre-Computation

```go
// client.go:77
unarySpec := config.newSpec(StreamTypeUnary)
unaryFunc := UnaryFunc(func(ctx context.Context, request AnyRequest) (AnyResponse, error) {
    conn := client.protocolClient.NewConn(ctx, unarySpec, request.Header())
    conn.onRequestSend(func(r *http.Request) {
        request.setRequestMethod(r.Method)
        callInfo, ok := clientCallInfoForContext(ctx)
        if ok {
            callInfo.method = r.Method
            callInfo.responseSource = conn
        }
    })
    // Send returns io.EOF only for client-side errors; caller should still Receive
    if err := conn.Send(request.Any()); err != nil && !errors.Is(err, io.EOF) {
        _ = conn.CloseRequest()
        _ = conn.CloseResponse()
        return nil, err
    }
    if err := conn.CloseRequest(); err != nil {
        _ = conn.CloseResponse()
        return nil, err
    }
    response, err := receiveUnaryResponse[Res](conn, config.Initializer)
    if err != nil {
        _ = conn.CloseResponse()
        return nil, err
    }
    return response, conn.CloseResponse()
})
```

**Aha:** The unary function is pre-computed at client creation time, not rebuilt per-call. This means interceptor application, connection setup, and request/response handling are all inlined into a single function at construction. The per-call cost is just the function call itself — no dynamic dispatch or chain rebuilding.

### Interceptor Application

```go
// client.go:107
if interceptor := config.Interceptor; interceptor != nil {
    unaryFunc = interceptor.WrapUnary(unaryFunc)
}
```

Interceptors are applied once at creation time. The `callUnary` method wraps the final unary function with spec, peer, and header setup:

```go
// client.go:111
client.callUnary = func(ctx context.Context, request *Request[Req]) (*Response[Res], error) {
    request.spec = unarySpec
    request.peer = client.protocolClient.Peer()
    protocolClient.WriteRequestHeader(StreamTypeUnary, request.Header())

    callInfo, callInfoOk := clientCallInfoForContext(ctx)
    if callInfoOk {
        callInfo.peer = request.Peer()
        callInfo.spec = request.Spec()
        mergeHeaders(request.Header(), callInfo.requestHeader)
        ctx = context.WithValue(ctx, sentinelContextKey{}, callInfo)
    }

    response, err := unaryFunc(ctx, request)
    // ... type assertion and return
}
```

## Client Configuration

```go
// client.go:304
type clientConfig struct {
    URL                    *url.URL
    Protocol               protocol
    Procedure              string
    Schema                 any
    Initializer            maybeInitializer
    CompressMinBytes       int
    Interceptor            Interceptor
    CompressionPools       map[string]*compressionPool
    CompressionNames       []string
    Codec                  Codec
    RequestCompressionName string
    BufferPool             *bufferPool
    ReadMaxBytes           int
    SendMaxBytes           int
    EnableGet              bool
    GetURLMaxBytes         int
    GetUseFallback         bool
    IdempotencyLevel       IdempotencyLevel
}
```

Default configuration (`client.go:325`):
- Protocol: `&protocolConnect{}` (Connect protocol)
- Codec: proto binary
- Compression: gzip (standard library)
- Buffer pool: 512-byte initial, 8MiB max recycle

To use gRPC or gRPC-Web, the client must explicitly opt in:
```go
client := NewClient[Req, Res](httpClient, url, connect.WithGRPC())
// or
client := NewClient[Req, Res](httpClient, url, connect.WithGRPCWeb())
```

## Call Methods

### CallUnary

```go
// client.go:149
func (c *Client[Req, Res]) CallUnary(ctx context.Context, request *Request[Req]) (*Response[Res], error) {
    if c.err != nil { return nil, c.err }
    return c.callUnary(ctx, request)
}
```

The pre-computed `callUnary` function handles the full request-response cycle: create connection, send request, close request write side, receive response, close response.

### CallClientStream

```go
// client.go:161
func (c *Client[Req, Res]) CallClientStream(ctx context.Context) *ClientStreamForClient[Req, Res] {
    if c.err != nil {
        return &ClientStreamForClient[Req, Res]{err: c.err}
    }
    return &ClientStreamForClient[Req, Res]{
        conn:        c.newConn(ctx, StreamTypeClient, nil),
        initializer: c.config.Initializer,
    }
}
```

Returns a `ClientStreamForClient` that wraps the `StreamingClientConn`. The caller sends multiple messages, then closes the stream to receive the response. Request headers are sent via `ClientStreamForClient.RequestHeader()` — not automatically on method invocation.

### CallClientStreamSimple

```go
// client.go:179
func (c *Client[Req, Res]) CallClientStreamSimple(ctx context.Context) (*ClientStreamForClientSimple[Req, Res], error) {
    stream := &ClientStreamForClientSimple[Req, Res]{
        stream: &ClientStreamForClient[Req, Res]{
            conn:        c.newConn(ctx, StreamTypeClient, nil),
            initializer: c.config.Initializer,
        },
    }
    if err := stream.Send(nil); err != nil { return nil, err }
    return stream, nil
}
```

The "simple" variant automatically sends a nil `Send()` to transmit request headers. Response headers and trailers are read from the `CallInfo` in context, not from a `Response` wrapper.

### CallServerStream

```go
// client.go:199
func (c *Client[Req, Res]) CallServerStream(ctx context.Context, request *Request[Req]) (*ServerStreamForClient[Res], error) {
    if c.err != nil { return nil, c.err }
    conn := c.newConn(ctx, StreamTypeServer, func(r *http.Request) {
        request.method = r.Method
    })
    request.peer = conn.Peer()
    request.spec = conn.Spec()
    mergeHeaders(conn.RequestHeader(), request.header)

    if err := conn.Send(request.Msg); err != nil && !errors.Is(err, io.EOF) {
        _ = conn.CloseRequest()
        _ = conn.CloseResponse()
        return nil, err
    }
    if err := conn.CloseRequest(); err != nil { return nil, err }

    return &ServerStreamForClient[Res]{
        conn:        conn,
        initializer: c.config.Initializer,
    }, nil
}
```

Sends a single request message, then returns a stream for receiving multiple response messages. The request is sent and the write side is closed before returning.

### CallBidiStream

```go
// client.go:233
func (c *Client[Req, Res]) CallBidiStream(ctx context.Context) *BidiStreamForClient[Req, Res] {
    if c.err != nil {
        return &BidiStreamForClient[Req, Res]{err: c.err}
    }
    return &BidiStreamForClient[Req, Res]{
        conn:        c.newConn(ctx, StreamTypeBidi, nil),
        initializer: c.config.Initializer,
    }
}
```

Returns a bidirectional stream — the caller can interleave `Send()` and `Receive()` calls. Requires HTTP/2 for true full-duplex communication.

## newConn — Streaming Connection Factory

```go
// client.go:269
func (c *Client[Req, Res]) newConn(ctx context.Context, streamType StreamType, onRequestSend func(r *http.Request)) StreamingClientConn {
    callInfo, callInfoOk := clientCallInfoForContext(ctx)
    if callInfoOk {
        ctx = context.WithValue(ctx, sentinelContextKey{}, callInfo)
    }

    newConn := func(ctx context.Context, spec Spec) StreamingClientConn {
        header := make(http.Header, 8)  // arbitrary power of two
        c.protocolClient.WriteRequestHeader(streamType, header)
        conn := c.protocolClient.NewConn(ctx, spec, header)
        conn.onRequestSend(onRequestSend)
        return conn
    }

    if interceptor := c.config.Interceptor; interceptor != nil {
        newConn = interceptor.WrapStreamingClient(newConn)
    }

    conn := newConn(ctx, c.config.newSpec(streamType))

    if callInfoOk {
        callInfo.peer = conn.Peer()
        callInfo.spec = conn.Spec()
        callInfo.responseSource = conn
        mergeHeaders(conn.RequestHeader(), callInfo.RequestHeader())
    }

    return conn
}
```

The connection factory applies interceptors via `WrapStreamingClient`, writes protocol headers, and merges call info headers. The `http.Header` is initialized with capacity 8 — a power of two that prevents immediate resizing for the typical number of headers.

## duplexHTTPCall — Full-Duplex HTTP Transport

```go
// duplex_http_call.go:32
type duplexHTTPCall struct {
    ctx              context.Context
    httpClient       HTTPClient
    streamType       StreamType
    onRequestSend    func(*http.Request)
    validateResponse func(*http.Response) *Error
    requestBodyWriter *io.PipeWriter   // for client-streaming and bidi
    requestSent      atomic.Bool
    request          *http.Request
    responseReady    chan struct{}
    response         *http.Response
    responseErr      error
}
```

### Construction

```go
// duplex_http_call.go:56
func newDuplexHTTPCall(ctx context.Context, httpClient HTTPClient, url *url.URL, spec Spec, header http.Header) *duplexHTTPCall {
    url = cloneURL(url)  // prevent transport from mutating our URL
    request := (&http.Request{
        Method:     http.MethodPost,
        URL:        url,
        Header:     header,
        Proto:      "HTTP/1.1",
        ProtoMajor: 1,
        ProtoMinor: 1,
        Body:       http.NoBody,
        Host:       url.Host,
    }).WithContext(ctx)

    duplex := &duplexHTTPCall{
        ctx:           ctx,
        httpClient:    httpClient,
        streamType:    spec.StreamType,
        request:       request,
        responseReady: make(chan struct{}),
    }

    // Client-streaming and bidi: set up io.Pipe for streaming request body
    if spec.StreamType&StreamTypeClient != 0 {
        pipeReader, pipeWriter := io.Pipe()
        duplex.requestBodyWriter = pipeWriter
        duplex.request.Body = pipeReader
        duplex.request.GetBody = nil
        duplex.request.ContentLength = -1
    }
    return duplex
}
```

**Aha:** The URL is cloned before passing to `http.Request` to prevent the transport from mutating it. Some HTTP transports modify `req.URL` during the request (e.g., following redirects). For streaming RPCs, the client may need to reuse the URL for subsequent operations, so cloning prevents unexpected mutations.

### Send — Dual Mode

```go
// duplex_http_call.go:106
func (d *duplexHTTPCall) Send(payload messagePayload) (int64, error) {
    if d.streamType&StreamTypeClient == 0 {
        return d.sendUnary(payload)
    }
    // Client-streaming / bidi mode
    isFirst := d.requestSent.CompareAndSwap(false, true)
    if isFirst {
        go d.makeRequest()  // concurrent request
    }
    if err := d.ctx.Err(); err != nil {
        return 0, wrapIfContextError(err)
    }
    if isFirst && payload.Len() == 0 {
        return 0, nil  // nil Send used to send headers only
    }
    bytesWritten, err := payload.WriteTo(d.requestBodyWriter)
    if err != nil && errors.Is(err, io.ErrClosedPipe) {
        err = io.EOF  // match grpc-go behavior
    }
    return bytesWritten, err
}
```

**Unary mode** (`sendUnary`):
```go
// duplex_http_call.go:137
func (d *duplexHTTPCall) sendUnary(payload messagePayload) (int64, error) {
    if !d.requestSent.CompareAndSwap(false, true) {
        return 0, errors.New("request already sent")
    }
    payloadLength := int64(payload.Len())
    if payloadLength > 0 {
        payloadBody := newPayloadCloser(payload)
        d.request.Body = payloadBody
        d.request.ContentLength = payloadLength
        d.request.GetBody = func() (io.ReadCloser, error) {
            if !payloadBody.Rewind() {
                return nil, errors.New("payload cannot be retried")
            }
            return payloadBody, nil
        }
        defer payloadBody.Release()
    }
    d.makeRequest()  // synchronous — blocks until response
    // ...
    return payloadLength, nil
}
```

**Aha:** Unary sends are synchronous — `makeRequest()` blocks until the response headers arrive. Streaming sends are asynchronous — `makeRequest()` runs in a goroutine, and writes to the `io.PipeWriter` concurrently while `net/http` reads from the `io.PipeReader`. The `payloadCloser` wrapper enables request body retry via `GetBody` — if the HTTP transport needs to rewind the body (e.g., after a connection retry), `Rewind()` seeks back to the start.

### makeRequest — Background HTTP Call

```go
// duplex_http_call.go:296
func (d *duplexHTTPCall) makeRequest() {
    defer close(d.responseReady)

    if host := getHeaderCanonical(d.request.Header, headerHost); len(host) > 0 {
        d.request.Host = host
    }
    if d.onRequestSend != nil {
        d.onRequestSend(d.request)
    }

    response, err := d.httpClient.Do(d.request)
    if err != nil {
        if errors.Is(err, io.EOF) {
            err = io.ErrUnexpectedEOF  // don't confuse with other io.EOF uses
        }
        err = wrapIfContextError(err)
        err = wrapIfLikelyH2CNotConfiguredError(d.request, err)
        err = wrapIfLikelyWithGRPCNotUsedError(err)
        err = wrapIfRSTError(d.ctx, err)
        if _, ok := asError(err); !ok {
            err = NewError(CodeUnavailable, err)
        }
        d.responseErr = err
        _ = d.CloseWrite()
        return
    }

    d.response = response
    if err := d.validateResponse(response); err != nil {
        d.responseErr = err
        _ = d.CloseWrite()
        return
    }

    // Bidi requires HTTP/2
    if (d.streamType&StreamTypeBidi) == StreamTypeBidi && response.ProtoMajor < 2 {
        d.responseErr = errorf(CodeUnimplemented,
            "response from %v is HTTP/%d.%d: bidi streams require at least HTTP/2",
            d.request.URL, response.ProtoMajor, response.ProtoMinor)
        _ = d.CloseWrite()
    }
}
```

The `responseReady` channel synchronizes readers with the HTTP response. `BlockUntilResponseReady()` waits on this channel — readers block until the server responds or an error occurs.

### CloseWrite — Half-Close

```go
// duplex_http_call.go:173
func (d *duplexHTTPCall) CloseWrite() error {
    if d.requestSent.CompareAndSwap(false, true) {
        go d.makeRequest()  // trigger request even if no messages sent
    }
    if d.requestBodyWriter != nil {
        return d.requestBodyWriter.Close()
    }
    return d.request.Body.Close()
}
```

**Aha:** For HTTP/1.1 compatibility, the write side must be closed before reading the response. HTTP/1.1 is half-duplex — the server won't start responding until the client finishes sending. The `CloseWrite` method handles this: for streaming RPCs, it closes the `io.PipeWriter`; for unary/server-streaming, it closes the request body.

### payloadCloser — Retry-Safe Request Body

```go
// duplex_http_call.go:425
type payloadCloser struct {
    mu      sync.Mutex
    payload messagePayload  // nil after Release
}

func (p *payloadCloser) Read(dst []byte) (int, error) {
    p.mu.Lock()
    defer p.mu.Unlock()
    if p.payload == nil { return 0, io.EOF }
    return p.payload.Read(dst)
}

func (p *payloadCloser) Rewind() bool {
    p.mu.Lock()
    defer p.mu.Unlock()
    if p.payload == nil { return false }
    if _, err := p.payload.Seek(0, io.SeekStart); err != nil { return false }
    return true
}

func (p *payloadCloser) Release() {
    p.mu.Lock()
    p.payload = nil
    p.mu.Unlock()
}
```

The `payloadCloser` wraps the unary request body, enabling retry via `Rewind()`. After the response is received, `Release()` nils the payload reference, making the body safe for reuse (e.g., buffer pool recycling).

## Response Validation

Each protocol client sets a `validateResponse` function on the `duplexHTTPCall`:

```go
// Set in protocol_connect.go for Connect unary client
duplex.SetValidateResponse(func(resp *http.Response) *Error {
    // Validate content-type, status code, etc.
    return connectValidateUnaryResponseContentType(...)
})
```

The validation runs in `makeRequest()` after receiving the HTTP response. If validation fails, the error is stored in `responseErr` and the write side is closed.

## Next

The document series is complete. The remaining files to create are `README.md` and `build.py` for HTML generation.
