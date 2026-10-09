# HTTP/1.1 vs HTTP/2 Streaming & Bidirectional Protocols

> Sources: ConnectRPC Go implementation (`connectrpc/connect-go`), gRPC protocol specification, ConnectRPC protocol docs.

---

## Q: What does "bidirectional streaming" actually mean technically?

**A:** Bidirectional streaming means both client and server can send an unbounded sequence of messages to each other simultaneously, interleaved and in any order, over a single logical connection — without waiting for the other side to finish first. It is not a TCP or UDP feature. It is an application-layer multiplexing pattern built on top of the transport.

---

## Q: Is bidirectional streaming a TCP or UDP feature?

**A:** No. TCP is always full-duplex — you can read and write simultaneously on any TCP socket. UDP is connectionless and inherently supports sending/receiving datagrams at any time. Bidirectional streaming in gRPC/Connect is neither of these directly. It is built on top of **HTTP**, which adds a request-response semantic layer on top of TCP. The "magic" is in how HTTP (particularly HTTP/2) structures message framing on top of the raw byte stream.

---

## Q: How does a TCP socket really work? How can it support true bidirectional communication?

**A:** The "socket is a file" abstraction is what causes confusion. That's a Unix convenience layer — underneath, a TCP socket is **two completely separate byte streams** that share connection metadata:

```
          ┌──────────────────────────────────────┐
          │           TCP Socket                  │
          │                                      │
          │  [state: IP, ports, seq nums, etc]   │
          │                                      │
    ┌─────┴──────┐                    ┌──────────┴──────┐
    │  RECV Q    │                    │  SEND Q         │
    │  (read)    │                    │  (write)        │
    │            │                    │                 │
    │  bytes     │                    │  bytes          │
    │  from      │                    │  to             │
    │  network   │                    │  network        │
    └────────────┘                    └─────────────────┘
```

The "read" side and "write" side are **independent buffers**. Writing to the send queue does not affect the recv queue. They don't share byte positions. There is no "cursor" that both sides fight over.

---

## Q: But I was told a socket is a file. If it's a file, how does it not corrupt data when reading and writing simultaneously?

**A:** A file has **one** byte sequence. You seek to a position, read or write at that position. If two processes read and write simultaneously, they're fighting over the same byte positions — chaos.

A TCP socket gives you a file descriptor so you can use `read()`, `write()`, `select()`, `poll()` — the same syscalls as files. But **underneath**, the kernel's `file_operations` for a socket point to completely different functions than a regular file:

```
Regular file:          Socket:
  read()  ──► vfs_read     read()  ──► sock_read   (drains recv queue)
  write() ──► vfs_write    write() ──► sock_write  (fills send queue)
  seek()  ──► vfs_llseek   seek()  ──► -ESPIPE     (operation not supported!)
```

You **cannot** `seek()` on a socket. The kernel returns `-ESPIPE`. This is proof it's not actually a file — it just uses a file descriptor as a handle.

---

## Q: What does the kernel actually do when you send and receive on a TCP socket?

**A:** The kernel allocates a `struct sock` (in Linux) with separate queues for each direction:

```c
// Simplified mental model
struct sock {
    struct sk_buff_head  receive_queue;   // incoming packets
    struct sk_buff_head  write_queue;     // outgoing packets
    // ... connection state: IPs, ports, sequence numbers, window sizes
};
```

`sk_buff` is a kernel buffer holding a chunk of network data. The receive queue and write queue are **separate linked lists**. They never touch each other.

**Sending** (your process → network):

```
Your process                    Kernel                        Network
  │                                │                              │
  │  write(fd, "hello", 5)         │                              │
  │──────────────────────────────► │                              │
  │                                │  copies to write_queue       │
  │                                │  TCP layers on headers       │
  │                                │  NIC driver picks up packet  │
  │                                │────────────────────────────► │
  │  returns 5                     │                              │
  │  (data already copied)         │                              │
```

**Receiving** (network → your process):

```
Your process                    Kernel                        Network
  │                                │                              │
  │                                │  NIC receives packet         │
  │                                │◄──────────────────────────── │
  │                                │  strips headers              │
  │                                │  appends to receive_queue    │
  │  read(fd, buf, 1024)           │                              │
  │◄────────────────────────────── │  copies from receive_queue   │
  │                                │                              │
```

The two flows **never interact**. Send path and receive path run through different code paths, different queues, different locks.

---

## Q: Why can't data get corrupted when reading and writing at the same time?

**A:** Data cannot be corrupted because:

1. **Send and recv are different memory** — `write()` copies your bytes into the send queue buffer. `read()` copies bytes out of the recv queue buffer. They never share memory.

2. **The NIC has separate TX/RX rings** — the network card has separate transmit and receive descriptor rings (buffers). TX pushes packets out, RX pulls packets in. Independent hardware queues.

3. **The physical layer is separate** — full-duplex Ethernet uses separate wire pairs for TX and RX. Data literally travels on different copper traces.

4. **TCP sequence numbers prevent reordering** — if packets arrive out of order, the recv queue holds them until gaps are filled. `read()` only gets in-order bytes.

---

## Q: What does the TCP header itself look like to support full-duplex?

**A:** TCP defines independent sequence numbers, window sizes, and acknowledgments for each direction:

```
  0                   1                   2                   3
  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |          Source Port          |       Destination Port        |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                    Sequence Number                            |  ← direction: me→you
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |                Acknowledgment Number                          |  ← direction: you→me
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |  Data |           |U|A|P|R|S|F|                               |
 | Offset| Reserved  |R|C|S|S|Y|I|            Window             |
 |       |           |G|K|H|T|N|N|                               |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

The `Sequence Number` is for **this direction**. The `Acknowledgment Number` is for the **other direction**. Two independent counters, two independent flows, one connection.

---

## Q: What's the right mental model for a TCP socket?

**A:** Think of a TCP socket as **a pipe with two tubes glued together**:

```
  ┌─────────────────────────────────────────┐
  │           TCP Connection                │
  │                                         │
  │   You ──►[tube A→B]───► Network ──► Them│
  │   You ◄──[tube B←A]───◄ Network ◄── Them│
  │                                         │
  │  [shared metadata: who, where, state]   │
  └─────────────────────────────────────────┘
```

Two tubes, one relationship. Data in tube A→B never mixes with data in tube B→A. They're physically separate paths through the kernel, through the NIC, through the cable. The only thing they share is the "address book" — who am I talking to, what are their sequence numbers, what's the window size.

This is why "full-duplex" isn't magic — it's literally two independent byte streams that happen to be created at the same time and bound to the same peer.

---

## Q: How does gRPC implement bidirectional streaming?

**A:** Over **HTTP/2**, gRPC opens a single HTTP/2 stream and sends multiple request messages as `DATA` frames from client to server and multiple response messages as `DATA` frames from server to client. HTTP/2 provides native multiplexing — a single TCP connection carries many independent streams identified by stream IDs. Both sides can send frames independently, interleaved, in any order. The stream stays open until either side sends a trailer frame.

Over **HTTP/1.1**, gRPC does **not** support bidirectional streaming. HTTP/1.1's request-response ordering prevents true interleaving.

---

## Q: How does ConnectRPC implement bidirectional streaming?

**A:** ConnectRPC follows the same principle. For bidirectional streams:

- **Over HTTP/2**: Uses HTTP/2 streams with `DATA` frames, same as gRPC
- **Over HTTP/1.x**: Explicitly **rejected**. The ConnectRPC Go source contains this check:

```go
if (d.streamType&StreamTypeBidi) == StreamTypeBidi && response.ProtoMajor < 2 {
    d.responseErr = errorf(
        CodeUnimplemented,
        "response from %v is HTTP/%d.%d: bidi streams require at least HTTP/2",
        d.request.URL,
        response.ProtoMajor,
        response.ProtoMinor,
    )
}
```

If the server responds with HTTP/1.1, Connect returns `CodeUnimplemented` for bidi streams.

---

## Q: Why can't HTTP/1.1 support bidirectional streaming?

**A:** HTTP/1.1 has a strict **request-response ordering**:

1. Client sends request headers
2. Client sends request body
3. Client signals end of request (EOF / zero-length chunk)
4. Server sends response headers
5. Server sends response body
6. Server signals end of response

The server **cannot** start sending response data until it has received the complete request. The client **cannot** send more data after finishing the request body. This means true bidirectional interleaving is impossible on a single HTTP/1.1 request.

---

## Q: What is `Transfer-Encoding: chunked` and how does it relate to streaming?

**A:** `Transfer-Encoding: chunked` is a standard HTTP/1.1 header that allows the body to be sent in pieces rather than all at once. The body is formatted as:

```
5\r\n          ← chunk size in hex (5 bytes)
msg1\r\n       ← data
5\r\n
msg2\r\n
0\r\n          ← zero-size chunk = end of body
\r\n
```

ConnectRPC uses this mechanism for **client-streaming** and **server-streaming** over HTTP/1.1 (via `io.Pipe()` in Go), but **not** for bidirectional streaming. Chunked encoding allows incremental body writing, but the server still only responds after the final `0\r\n\r\n` chunk closes the request body.

---

## Q: What is ConnectRPC's message envelope format?

**A:** On top of the HTTP body (whether chunked or not), Connect uses its own **5-byte length-prefix framing** for each message:

```
[1 byte flags][4 bytes big-endian message length][N bytes payload]
```

The flags byte supports bitwise flags. `0x01` indicates the message is compressed. `0x02` indicates an end-stream envelope (for server-to-client direction, carries error details and trailers).

The full layering:

```
TCP socket
  └── HTTP/1.1 or HTTP/2 request with Transfer-Encoding: chunked (HTTP/1.1 only)
        └── HTTP body bytes
              └── Connect envelope: [flags (1B)][length (4B)][payload (NB)]
                    └── Protobuf/JSON message
```

This same envelope format is used in gRPC, gRPC-Web, and the Connect protocol — they all share the 5-byte prefix framing.

---

## Q: What stream types does ConnectRPC support over HTTP/1.1?

**A:**

| Stream Type | Description | HTTP/1.1 Support | Mechanism |
|---|---|---|---|
| **Unary** | Client sends one message, server sends one response | Yes | Normal POST with body |
| **Client-streaming** | Client sends many messages, server sends one response | Yes | `io.Pipe()` + chunked body, must `CloseWrite()` before read |
| **Server-streaming** | Client sends one message, server streams many responses | Yes | `http.Flusher` after each envelope message |
| **Bidirectional** | Both sides stream simultaneously | **No** | Requires HTTP/2+ |

---

## Q: How does ConnectRPC implement client-streaming over HTTP/1.1?

**A:** Using `io.Pipe()` to create a full-duplex channel inside the client:

```go
// In newDuplexHTTPCall():
if spec.StreamType&StreamTypeClient != 0 {
    pipeReader, pipeWriter := io.Pipe()
    duplex.requestBodyWriter = pipeWriter
    duplex.request.Body = pipeReader
    duplex.request.ContentLength = -1  // signals chunked transfer
}
```

The flow:

```
Client                          Server
  │  POST /method                 │
  │  Content-Length: -1           │
  │  ───────────────────────────► │
  │  msg1 (via pipe write) ──────►│  (server reads)
  │  msg2 (via pipe write) ──────►│  (server reads)
  │  close writer                 │  (server sees EOF)
  │                               │  (server processes)
  │  ◄────────────────────────── │  response body
```

The critical constraint from the Connect source:

```go
// HTTP/1.1 doesn't support bidirectional streaming - the write side of the
// stream (aka request body) must be closed before we start reading the
// response or we'll just block forever.
```

So `CloseWrite()` must be called before `Read()`, making this request-response with chunked input, not true bidi.

---

## Q: How does ConnectRPC implement server-streaming over HTTP/1.1?

**A:** The server uses `http.Flusher` to flush each envelope message immediately after writing it, rather than buffering the entire response. The Connect source requires:

```go
func checkServerStreamsCanFlush(spec Spec, responseWriter http.ResponseWriter) *Error {
    requiresFlusher := (spec.StreamType & StreamTypeServer) == StreamTypeServer
    if _, flushable := responseWriter.(http.Flusher); requiresFlusher && !flushable {
        return NewError(CodeInternal, fmt.Errorf("%T does not implement http.Flusher", responseWriter))
    }
}

func flushResponseWriter(w http.ResponseWriter) {
    if f, ok := w.(http.Flusher); ok {
        f.Flush()
    }
}
```

The flow:

```
Client                          Server
  │  POST /method                 │
  │  ───────────────────────────► │
  │                               │  (server processes)
  │  ◄──[envelope][msg1]──────── │  (flush)
  │  ◄──[envelope][msg2]──────── │  (flush)
  │  ◄──[end-stream envelope]─── │  (flush + close)
```

The server must flush after each message, otherwise the HTTP server buffers the entire response and sends it all at once (defeating the purpose of streaming).

---

## Q: How does duplexHTTPCall work in ConnectRPC?

**A:** `duplexHTTPCall` is the core abstraction that provides a full-duplex interface over HTTP. It wraps Go's `net/http` client and uses:

1. **`io.Pipe()`** for the request body — client writes to `PipeWriter`, `net/http` reads from `PipeReader` concurrently
2. **Goroutine for request** — `makeRequest()` runs in a background goroutine so writes and reads can happen concurrently
3. **`responseReady` channel** — coordinates between the goroutine sending the request and the reader waiting for the response
4. **`StreamType` flags** — determines which mechanism to use (`StreamTypeClient`, `StreamTypeServer`, `StreamTypeBidi`, `StreamTypeUnary`)

Key implementation detail from the source:

```go
// Be warned: we need to use some lesser-known APIs to do this with net/http.
type duplexHTTPCall struct {
    ctx              context.Context
    httpClient       HTTPClient
    requestBodyWriter *io.PipeWriter   // assigned once, safe to read without sync
    requestSent      atomic.Bool
    responseReady    chan struct{}     // closed when response is ready
    response         *http.Response
    responseErr      error
}
```

The `Send()` method for streaming RPCs:

```go
func (d *duplexHTTPCall) Send(payload messagePayload) (int64, error) {
    isFirst := d.requestSent.CompareAndSwap(false, true)
    if isFirst {
        go d.makeRequest() // concurrent request
    }
    bytesWritten, err := payload.WriteTo(d.requestBodyWriter)
    return bytesWritten, err
}
```

This allows writing to the request body pipe while `net/http` concurrently reads from the other end and sends data over the network.

---

## Q: What are the practical workarounds for bidirectional streaming over HTTP/1.1?

**A:** Since HTTP/1.1 cannot support true bidi on a single request, the approaches are:

### 1. WebSocket (Recommended)

WebSocket starts as an HTTP/1.1 request with `Upgrade: websocket`. The server responds with `101 Switching Protocols` and downgrades the connection from HTTP to a raw TCP socket with lightweight framing:

```
Client: GET /ws  Upgrade: websocket  ───────────► Server
Server: 101 Switching Protocols                   │
         ◄────────────────────────────────────────│
         [TCP socket, full-duplex, raw framing]
         msg1  ──────────────────►
               ◄──────────── reply1
         msg2  ──────────────────►
               ◄──────────── reply2
```

This is the closest you get to "true" bidirectional streaming over HTTP/1.1 infrastructure. Used by gRPC-Web as a fallback.

### 2. Two Correlated HTTP Connections

Open two HTTP/1.1 connections:

```
Connection A (Client → Server):      Connection B (Server → Client):
  POST /stream/upload                  GET /stream/download?id=abc
  chunk: msg1  ──────────────────►       ◄──────────── chunk: reply1
  chunk: msg2  ──────────────────►       ◄──────────── chunk: reply2
```

The server correlates them with a session ID. This is gRPC-Web's default approach without WebSocket. Requires managing correlation state between two separate requests.

### 3. Long Polling

Repeated requests that block until the server has data. Not real streaming — high latency per message.

### 4. Chunked Transfer Encoding (Limited)

HTTP/1.1 chunked bodies work on the wire, but many servers and reverse proxies buffer the entire request before passing it to the handler. Even though chunked encoding is technically being used, the server doesn't see messages until the client closes the body.

---

## Q: What is the difference between what TCP provides vs what HTTP/2 adds for streaming?

**A:**

| Layer | Provides | Relevant to bidi streaming? |
|-------|----------|----------------------------|
| **TCP** | Full-duplex byte stream (can read+write simultaneously) | Yes, but this is just the raw capability |
| **HTTP/1.1** | Request-response; server responds after request | No — fundamentally half-duplex per request |
| **HTTP/2** | Multiplexed binary frames on one connection | **Yes** — native stream multiplexing with `DATA` frames |
| **HTTP/3 (QUIC)** | Multiplexed streams over UDP | Same concept, over UDP with QUIC streams |
| **gRPC/Connect** | Protocol that maps RPC calls to HTTP streams | Application layer orchestrating the above |

The critical insight: TCP gives you the raw full-duplex pipe, but HTTP/1.1 layers a half-duplex protocol on top. HTTP/2 restores multiplexing with structured frame types. gRPC and Connect are application protocols that use HTTP/2's frame types to implement their streaming semantics.

---

## Q: What is the HTTP/1.1 request using `Proto: "HTTP/1.1"` in the duplexHTTPCall source?

**A:** In `newDuplexHTTPCall()`, the request is constructed as:

```go
request := (&http.Request{
    Method:     http.MethodPost,
    URL:        url,
    Header:     header,
    Proto:      "HTTP/1.1",
    ProtoMajor: 1,
    ProtoMinor: 1,
    Body:       http.NoBody,
    GetBody:    getNoBody,
    Host:       url.Host,
}).WithContext(ctx)
```

This does **not** force HTTP/1.1 on the wire. It's initializing the request struct with HTTP/1.1 as the default proto because `http.NewRequestContext` does the same. The actual wire protocol is determined by:

1. What the `HTTPClient` (transport) negotiates with the server
2. Whether the server supports HTTP/2 (ALPN negotiation for HTTPS, or h2c for cleartext)
3. The server's response `ProtoMajor`/`ProtoMinor`

The bidi check in `makeRequest()` examines the **actual** response protocol:

```go
if (d.streamType&StreamTypeBidi) == StreamTypeBidi && response.ProtoMajor < 2 {
    // fail
}
```

So the `Proto: "HTTP/1.1"` in the constructor is just initialization — the real protocol version comes from the server's response.

---

## Q: What compression support exists in Connect's streaming envelope?

**A:** The envelope's flags byte (`0x01`) indicates whether the payload is compressed. Connect uses separate compression pools for streaming vs non-streaming:

- `connectStreamingHeaderCompression` = `"Connect-Content-Encoding"` — the header indicating compression used for the streaming body
- `connectStreamingHeaderAcceptCompression` = `"Connect-Accept-Encoding"` — the header advertising what compressions the sender can decompress

Each message in a stream can be individually compressed or not (the envelope flags byte indicates which). This is different from unary where the entire body is compressed as one unit.

---

## Q: What is the `StreamType` flag system in ConnectRPC?

**A:** ConnectRPC uses bitwise flags to classify stream types:

```
StreamTypeUnary    = (1 << 0)
StreamTypeClient   = (1 << 1)   // client sends multiple messages
StreamTypeServer   = (1 << 2)   // server sends multiple messages
StreamTypeBidi     = StreamTypeClient | StreamTypeServer  // both
```

The code uses bitwise operations to check capabilities:

```go
// Is this a client-streaming or bidi RPC?
if spec.StreamType&StreamTypeClient != 0 {
    // set up io.Pipe for request body
}

// Is this a server-streaming or bidi RPC?
if spec.StreamType&StreamTypeServer != 0 {
    // require http.Flusher
}

// Is this specifically bidi?
if (d.streamType&StreamTypeBidi) == StreamTypeBidi {
    // require HTTP/2
}
```

---

## Q: How are end-of-stream and trailers handled in the Connect protocol?

**A:** For server-to-client direction, the final message uses a special envelope flag:

```go
connectFlagEnvelopeEndStream = 0b00000010
```

When the `0x02` flag is set, the envelope contains a serialized end-stream message carrying error details and HTTP trailers instead of a normal protobuf message. This is how the server communicates RPC completion and error status within the body stream itself, rather than relying on HTTP status codes alone.

The client reads envelopes in a loop until it encounters an envelope with the `EndStream` flag or gets `io.EOF` from the body reader.

---

## Q: How does ConnectRPC support gRPC, gRPC-Web, and its own protocol simultaneously without a proxy?

**A:** ConnectRPC uses a **content-type-based dispatch** pattern. When an HTTP request arrives at a `Handler.ServeHTTP()`, it:

1. Extracts the `Content-Type` header from the request
2. Iterates through its registered protocol handlers
3. Each protocol handler declares which content-types it can handle via `CanHandlePayload()`
4. The first matching handler gets the request

```go
// In ServeHTTP():
contentType := canonicalizeContentType(getHeaderCanonical(request.Header, headerContentType))

var protocolHandler protocolHandler
for _, handler := range protocolHandlers {
    if handler.CanHandlePayload(request, contentType) {
        protocolHandler = handler
        break
    }
}
```

Each `Handler` (each RPC endpoint) creates **three** protocol handlers at construction time:

```go
func (c *handlerConfig) newProtocolHandlers() []protocolHandler {
    protocols := []protocol{
        &protocolConnect{},         // Connect protocol
        &protocolGRPC{web: false},  // gRPC protocol
        &protocolGRPC{web: true},   // gRPC-Web protocol
    }
    // ... creates a handler for each
}
```

No proxy needed because the server **natively understands** all three protocols. It's not translating between them — it's just dispatching to the right handler based on what the client asked for.

---

## Q: How do the three protocols differ from each other?

**A:** They differ in four key areas:

### 1. Content-Type Headers

| Protocol | Content-Type |
|---|---|
| **Connect** (streaming) | `application/connect+proto`, `application/connect+json` |
| **Connect** (unary) | `application/json`, `application/proto` |
| **gRPC** | `application/grpc`, `application/grpc+proto` |
| **gRPC-Web** | `application/grpc-web`, `application/grpc-web+proto` |

This is how the dispatch works — the client identifies which protocol it's speaking by its content-type.

### 2. Timeout Headers

| Protocol | Header | Format |
|---|---|---|
| **Connect** | `Connect-Timeout-Ms` | Milliseconds as integer (`"5000"`) |
| **gRPC/gRPC-Web** | `Grpc-Timeout` | Unit-based (`"5000m"`, `"1H"`, `"10S"`) |

### 3. Trailer Handling (The Biggest Difference)

**gRPC** — HTTP trailers (HTTP/2 `TRAILER` frames):
- Server writes trailers using `http.TrailerPrefix` (e.g., `Trailer-Grpc-Status`)
- Client reads HTTP trailers from the response after the body
- Requires `Te: trailers` request header to signal trailer support
- `grpcFlagEnvelopeTrailer = 0b10000000` — trailer envelope flag

**gRPC-Web** — Trailers encoded in the HTTP body:
- Server writes trailers as an envelope with `grpcFlagEnvelopeTrailer` flag
- The envelope body contains an HTTP/1 header block (without terminating newline):
  ```
  [0x80][length][Grpc-Status: 0\r\nGrpc-Message: OK\r\n]
  ```
  The `0x80` flag byte signals "this envelope contains trailers, not a message"
- Client parses this by adding a newline and using `textproto.NewReader` to parse MIME headers
- No HTTP/2 trailers needed — works over HTTP/1.1

**Connect** — End-stream envelope in the body:
- Server writes a JSON end-stream message wrapped in an envelope:
  ```
  [0x02][length]{"error":{"code":"...","message":"..."},"trailer":{"X-Custom":"val"}}
  ```
  The `0x02` flag (`connectFlagEnvelopeEndStream`) signals end-of-stream
- Error details and trailers are serialized as JSON inside the body

### 4. Compression Headers

| Protocol | Compression Header | Accept-Encoding Header |
|---|---|---|
| **Connect** (streaming) | `Connect-Content-Encoding` | `Connect-Accept-Encoding` |
| **Connect** (unary) | `Content-Encoding` | `Accept-Encoding` |
| **gRPC/gRPC-Web** | `Grpc-Encoding` | `Grpc-Accept-Encoding` |

### 5. Protocol-Specific Error Headers

| Protocol | Status Header | Message Header |
|---|---|---|
| **gRPC/gRPC-Web** | `Grpc-Status` | `Grpc-Message` |
| **Connect** | JSON in end-stream envelope | JSON in end-stream envelope |

---

## Q: What does the shared `protocolHandler` interface look like?

**A:** All three protocols implement the same interface:

```go
type protocolHandler interface {
    Methods() map[string]struct{}              // Which HTTP methods (POST, GET)
    ContentTypes() map[string]struct{}         // Which Content-Types this protocol handles
    SetTimeout(*http.Request) (context.Context, context.CancelFunc, error)
    CanHandlePayload(*http.Request, string) bool
    NewConn(http.ResponseWriter, *http.Request) (handlerConnCloser, bool)
}
```

The implementation differs per protocol but the interface is identical. This is how ConnectRPC achieves **protocol polymorphism** — the business logic (the RPC handler) doesn't know or care which protocol the client uses.

---

## Q: How does the gRPC vs gRPC-Web distinction work internally?

**A:** They share the same `protocolGRPC` struct, differentiated by a single `web bool` field:

```go
type protocolGRPC struct {
    web bool
}
```

This single boolean cascades through the entire implementation:

```go
// Content-Type selection
func (g *protocolGRPC) NewHandler(params *protocolHandlerParams) protocolHandler {
    bare, prefix := grpcContentTypeDefault, grpcContentTypePrefix
    if g.web {
        bare, prefix = grpcWebContentTypeDefault, grpcWebContentTypePrefix
    }
    // ... register content types
}
```

The same `grpcHandlerConn`, `grpcMarshaler`, `grpcUnmarshaler` structs handle both — they just have a `web bool` field that changes behavior:

```go
type grpcHandlerConn struct {
    web            bool  // ← changes trailer sending behavior
    // ...
}

func (hc *grpcHandlerConn) Close(err error) error {
    // ...
    if hc.web && !hc.wroteToBody && len(hc.responseHeader) == 0 {
        // gRPC-Web, trailers-only: send as HTTP headers
        mergeHeaders(hc.responseWriter.Header(), mergedTrailers)
        return nil
    }
    if hc.web {
        // gRPC-Web, body already started: write trailers INTO the body
        return hc.marshaler.MarshalWebTrailers(mergedTrailers)
    }
    // Standard gRPC: write trailers as HTTP/2 trailer frames
    for key, values := range mergedTrailers {
        hc.responseWriter.Header().Add(http.TrailerPrefix+key, value)
    }
}
```

---

## Q: Why doesn't ConnectRPC need an Envoy proxy for gRPC-Web?

**A:** The traditional gRPC-Web setup requires Envoy as a sidecar proxy because:

1. Browsers can't read HTTP/2 trailers (gRPC's normal trailer mechanism)
2. Envoy translates: HTTP/2 trailers → body-embedded trailers for the browser
3. Envoy sits between the browser and the gRPC server, doing protocol translation

ConnectRPC eliminates this need because **the server itself natively speaks gRPC-Web**. When a gRPC-Web client connects:

1. The `Content-Type: application/grpc-web` header triggers the gRPC-Web handler
2. The handler writes trailers directly into the HTTP body (not HTTP/2 trailers)
3. The client reads trailers from the body envelope

```
Traditional gRPC-Web:
  Browser ──gRPC-Web──► Envoy ──gRPC──► gRPC Server
  (body trailers)       (translate)     (HTTP/2 trailers)

ConnectRPC gRPC-Web:
  Browser ──gRPC-Web──► ConnectRPC Server
  (body trailers)       (native, no translation)
```

No proxy needed because the ConnectRPC server was designed from the start to handle all three protocols natively. The `protocolGRPC{web: true}` handler writes body-embedded trailers directly — there's nothing for a proxy to translate.

---

## Q: How does the shared envelope format work across all three protocols?

**A:** All three use the same 5-byte envelope prefix:

```
[1 byte flags][4 bytes big-endian message length][N bytes payload]
```

The flag byte has protocol-specific meanings:

| Flag | gRPC/gRPC-Web | Connect |
|---|---|---|
| `0x01` | Compressed | Compressed |
| `0x02` | (unused) | End-stream envelope |
| `0x80` | Trailer envelope | (unused) |

This shared framing is why the `envelopeWriter` and `envelopeReader` types are shared across all protocol implementations — they live in `envelope.go` as a common utility.

---

## Q: What is the full request flow when a ConnectRPC server receives a request?

**A:**

```
1. HTTP request arrives at Handler.ServeHTTP()

2. Check bidi over HTTP/1.1 → reject if bidi && ProtoMajor < 2

3. Extract Content-Type header

4. Dispatch to protocol handler:
   for _, handler := range protocolHandlers {
       if handler.CanHandlePayload(request, contentType) {
           protocolHandler = handler
           break
       }
   }

5. Protocol-specific timeout parsing:
   - Connect: parse "Connect-Timeout-Ms" → context.WithTimeout
   - gRPC: parse "Grpc-Timeout" (e.g., "5000m") → context.WithTimeout

6. Create handler connection:
   conn := protocolHandler.NewConn(responseWriter, request)
   // This constructs the right *HandlerConn type:
   //   connectHandlerConn, grpcHandlerConn, etc.

7. Run the RPC implementation:
   connCloser.Close(h.implementation(ctx, connCloser))

8. On Close(), the protocol handler:
   - Writes error/trailers in protocol-specific format
   - Flushes the response
   - Closes the request body
```

The RPC implementation (`h.implementation`) never knows which protocol was used. It just calls `conn.Send()` and `conn.Receive()` — the protocol handler handles the translation.
