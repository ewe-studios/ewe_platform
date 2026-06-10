# Spec 41: Review Gaps — Consolidated Findings

Three independent review agents cross-referenced all 10 design decision documents against connect-go source code and the actual foundation crate APIs. This document consolidates every gap, mismatch, blocker, Rust-specific issue, and open question found.

Status legend: **CRITICAL** = blocks correct implementation, **SIGNIFICANT** = breaks interop or causes subtle bugs, **MEDIUM** = missing feature or optimization, **SMALL** = nice-to-have or cosmetic.

---

## 1. Foundation Platform Blockers

These are concrete gaps in foundation_http / foundation_netio that block ConnectRPC implementation. They must be resolved (extended, worked around, or scoped out) before feature specs.

### B1. No mid-response flush for streaming (CRITICAL)
**Affects:** Decision 01, 05, 08
The `Serve` trait returns `ConnectionResult` after the handler completes. `ServeWriter` provides `&mut dyn Write` but no flush-per-message mechanism. `RenderHttp::http_render_to_writer()` calls `writer.flush()` only once at the end. Streaming RPC requires flushing after each envelope frame so clients receive messages incrementally. Without this, streaming responses buffer until the handler finishes, defeating the purpose entirely.

### B2. No incremental/streaming response writing from handlers (CRITICAL)
**Affects:** Decision 01, 05, 08
`SimpleOutgoingResponse` is built as a complete object (status + headers + body) and rendered atomically via `Http11ResponseIterator`. Streaming handlers need to: (1) write headers immediately, (2) write envelope-framed messages one at a time, (3) flush after each. There is no mechanism for a handler to write partial responses while continuing to process.

### B3. No HTTP trailing headers (CRITICAL for gRPC, SIGNIFICANT for gRPC-Web)
**Affects:** Decision 05
`SimpleOutgoingResponse` has no trailers field. `Http11ResponseIterator` does not render trailers. gRPC over HTTP/2 requires trailing HEADERS frames. gRPC-Web uses in-body trailer frames (can work without HTTP trailers). Connect protocol uses `Trailer-` prefixed response headers (works with current model). This blocks gRPC protocol support entirely and complicates gRPC-Web.

### B4. SimpleHeaders uppercases custom header keys (SIGNIFICANT)
**Affects:** Decision 05, 09
`SimpleHeader::from(String)` calls `.to_uppercase()`, so `"grpc-status"` becomes `Custom("GRPC-STATUS")`. When rendering to wire, `Custom` uses its stored value verbatim. Protocol headers like `grpc-status`, `grpc-encoding`, `connect-protocol-version` will appear uppercase on the wire, violating HTTP/2 requirements (lowercase only) and potentially breaking gRPC clients. Every protocol header lookup and emission requires careful case handling.

### B5. No HTTP/2 support (CRITICAL for gRPC, known Phase 2)
**Affects:** Decision 01, 05
`SimpleHttpClient` only supports HTTP/1.1. Response rendering is hardcoded to HTTP/1.1 wire format. gRPC protocol requires HTTP/2. Decision 01 acknowledges this and proposes Phase 2 with the `h2` crate. This also blocks true full-duplex bidi streaming.

#### B5a. What's already protocol-agnostic (works for HTTP/2 as-is)
- `SimpleIncomingRequest` / `SimpleOutgoingResponse` carry a `Proto` field; `Proto::HTTP20` variant exists but is unused.
- `SendSafeBody` enum is protocol-agnostic — works with any HTTP version.
- `Serve` / `ServeWriter` handler traits receive `SimpleIncomingRequest` with no HTTP/1.1 assumption.
- Router and middleware are generic, no protocol constraints.

#### B5b. What's hardcoded to HTTP/1.1 in foundation_netio
1. **Text-based request parsing** (`impls.rs:3156-3417`): `HttpRequestReader` assumes `METHOD URI HTTP/1.1\r\n` text format. HTTP/2 uses binary 9-byte frame headers + HPACK-compressed pseudo-headers (`:method`, `:path`, `:scheme`, `:authority`). A new `Http2FrameDecoder` is needed alongside the text parser.
2. **Text-based response rendering** (`impls.rs:2013-2072`): `Http11RequestDescriptorIterator` hardcodes `"{METHOD} {PATH} HTTP/1.1\r\n"`. HTTP/2 needs binary HEADERS frames with HPACK encoding. Need `Http2FrameEncoder`.
3. **`HTTPStreams<T>` factory** (`impls.rs:5456-5487`): `next_request()` / `next_response()` can only create HTTP/1.1 readers. Must branch on negotiated protocol.
4. **No ALPN in TLS** (`netcap/ssl/mod.rs:23-86`): rustls 0.23 supports ALPN via `set_protocols(&[b"h2", b"http/1.1"])` but it's not wired. Without ALPN, clients can't negotiate `h2` over TLS. Also no h2c detection (the `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n` magic prefix for cleartext HTTP/2).
5. **HTTP client is single-request-per-connection** (`simple_http/client/native/`): No multiplexing. HTTP/2 allows hundreds of concurrent streams on one TCP connection.

#### B5c. What's hardcoded to HTTP/1.1 in foundation_http
1. **Sequential connection handler** (`native/server/connection.rs:56-250`): `ConnectionHandler` state machine is `Idle → CheckExpect → WaitingForBody → Processing → loop`. One request at a time per connection. HTTP/2 needs per-stream state machines with concurrent dispatch to the router.
2. **100-Continue handling** (`connection.rs:34-45, 117-158`): Hardcoded to HTTP/1.1 interim response semantics. HTTP/2 uses RST_STREAM, not 100-continue.
3. **Response rendering** (`shared/serve/mod.rs:146-165`): `render_response()` hardcodes `Http11::response(response).http_render_to_writer(conn)`. Needs a protocol branch.

#### B5d. What HTTP/2 support requires (3 subsystems in netio, 2 in http)

**foundation_netio — 3 new subsystems:**
1. **Binary frame codec**: 9-byte frame headers, HPACK header compression/decompression table, frame type dispatch (DATA, HEADERS, PRIORITY, RST_STREAM, SETTINGS, PUSH_PROMISE, PING, GOAWAY, WINDOW_UPDATE, CONTINUATION). Recommended: use the `h2` crate which handles all of this.
2. **ALPN wiring in TLS**: Configure rustls `ServerConfig` / `ClientConfig` with `set_protocols(&[b"h2", b"http/1.1"])`. Detect negotiated protocol after TLS handshake. Also detect h2c via magic prefix on cleartext connections.
3. **Stream multiplexer**: Track concurrent streams by stream ID, per-stream and per-connection flow control windows, stream priority/dependency graph, RST_STREAM handling, GOAWAY for graceful shutdown.

**foundation_http — 2 major changes:**
1. **Multi-stream connection handler**: After ALPN selects `h2`, hand the socket to the `h2` crate's `server::handshake()`. Bridge `h2::RecvStream` / `h2::SendStream` into `SimpleIncomingRequest` / `SimpleOutgoingResponse`. Each HTTP/2 stream dispatches independently to the router. The key integration point is `ConnectionHandler` — it needs a branch: h2 crate for HTTP/2, existing state machine for HTTP/1.1.
2. **SETTINGS exchange & flow control**: HTTP/2 connections start with a SETTINGS frame handshake. Flow control is per-stream and per-connection. The `h2` crate manages this, but foundation_http must expose configuration (max concurrent streams, initial window size, max frame size).

#### B5e. Recommended approach: Replicate h2's design, tokio-free
The `h2` crate is the most mature HTTP/2 implementation in Rust, but it depends on tokio (`AsyncRead`/`AsyncWrite`, tokio runtime context). This conflicts with Valtron's progress-driven async model — bringing in tokio means two competing executors.

**Approach:** Use `h2` as the reference implementation — study its architecture, data structures, and state machines — then replicate the parts we need as a tokio-free implementation in foundation_netio's `http2/` module. This is not a fork or vendored copy; it's a clean reimplementation that follows h2's proven design decisions while writing against `std::io::Read`/`Write` and Valtron's iterator-based streaming model.

**What to replicate from h2:**
- HPACK encoder/decoder (header compression table, Huffman coding)
- Frame codec (9-byte frame headers, frame type dispatch, continuation handling)
- Flow control arithmetic (per-stream and per-connection window tracking)
- Stream state machine (idle → open → half-closed → closed)
- Settings negotiation logic
- Priority/dependency tree (optional, can defer)

**Key differences from h2:**
- `tokio::io::AsyncRead` / `AsyncWrite` → `std::io::Read` / `Write` on `SharedByteBufferStream<RawStream>`
- `tokio::sync` channels → Valtron's `ConcurrentQueueStreamIterator` or crossbeam channels
- `Future`/`Poll`-based state machines → iterator-based state machines matching `HttpRequestReader` pattern
- Connection handshake → synchronous SETTINGS exchange in `ConnectionHandler`

**Module structure:** HTTP/2 gets its own module in foundation_netio, separate from `simple_http/`. The HTTP/1.1 text-based parser in `simple_http/shared/impls.rs` is fundamentally different from HTTP/2's binary framing — they should not share a parser. The new module reuses foundation_netio's shared types (`SimpleIncomingRequest`, `SimpleOutgoingResponse`, `SimpleHeaders`, `SendSafeBody`, `Proto`, `SharedByteBufferStream<RawStream>`, TLS/SSL wrappers) but implements its own frame reader/writer, stream state machine, and connection handler. Think of it as a sibling to `simple_http/`, not an extension of it.

```
backends/foundation_netio/src/
├── simple_http/          # existing HTTP/1.1 text-based parser
│   └── shared/impls.rs   # HttpRequestReader, Http11RequestDescriptorIterator, etc.
├── http2/                # new HTTP/2 binary frame parser (forked from h2)
│   ├── mod.rs
│   ├── frame/            # 9-byte frame codec, frame types
│   ├── hpack/            # HPACK header compression/decompression
│   ├── stream/           # per-stream state machine, multiplexer
│   ├── flow_control.rs   # window tracking (per-stream + per-connection)
│   ├── settings.rs       # SETTINGS negotiation
│   └── connection.rs     # connection-level state, GOAWAY, PING
└── netcap/               # shared: TCP, TLS/SSL, RawStream (used by both)
```

**Integration point:** `ConnectionHandler` in foundation_http branches after protocol detection (ALPN or h2c magic prefix): HTTP/1.1 uses `simple_http/` parser, HTTP/2 uses `http2/` parser. Both paths produce `SimpleIncomingRequest` and consume `SimpleOutgoingResponse` — handlers see the same types regardless of protocol version.

**Estimated scope:** 5-8 features (larger than wrapping h2 as-is, but avoids the tokio dependency and gives full control). Can be phased: (1) frame codec + HPACK, (2) server-side stream multiplexer, (3) client-side multiplexer, (4) flow control tuning.

### B9. No HTTP/3 support (Phase 3, replicates h3 design with a tokio-free QUIC backend)
**Affects:** Decision 01
HTTP/3 replaces TCP+TLS with QUIC — multiplexed streams without head-of-line blocking, 0-RTT connection establishment, built-in encryption. For ConnectRPC this means all three wire protocols (Connect, gRPC, gRPC-Web) can run over QUIC with better latency and resilience than HTTP/2 over TCP.

**Reference implementation:** The `h3` crate by hyperium (`/home/darkvoid/Boxxed/@formulas/src.rust/src.tokio/h3/`). It has a clean architecture worth replicating:
- `h3/` — core HTTP/3 framing, QPACK, connection/stream management. Only depends on tokio for `tokio::sync` (channels), not the runtime. Uses `Poll`-based QUIC trait abstraction.
- `h3/src/quic.rs` — defines QUIC transport traits (`Connection`, `OpenStreams`, `SendStream`, `RecvStream`, `BidiStream`) that are backend-agnostic.
- `h3/src/qpack/` — QPACK header compression (HTTP/3's replacement for HPACK).
- `h3/src/frame.rs`, `h3/src/proto/` — HTTP/3 frame types and stream ID management.
- `h3-quinn/` — one QUIC backend (quinn). The trait abstraction means any QUIC implementation can plug in.

**Approach:** Same as HTTP/2 — replicate h3's design tokio-free, don't fork or vendor. Study h3's QUIC trait abstraction, QPACK implementation, frame codec, and connection state machine. Rewrite against synchronous/iterator-based APIs matching Valtron's model. Replace `tokio::sync` with crossbeam or Valtron's `ConcurrentQueueStreamIterator`. Replace `Poll`-based QUIC traits with synchronous equivalents.

For the QUIC transport layer itself: find or build a tokio-free QUIC implementation. Options include adapting quinn-proto (the protocol-only layer of quinn, which is transport-agnostic) or evaluating other pure Rust QUIC libraries. The QUIC backend plugs into the HTTP/3 module via the same trait abstraction pattern h3 uses.

**Module structure:**
```
backends/foundation_netio/src/
├── simple_http/          # HTTP/1.1 text-based parser
├── http2/                # HTTP/2 binary frame parser (replicated from h2)
├── http3/                # HTTP/3 over QUIC (replicated from h3)
│   ├── mod.rs
│   ├── qpack/            # QPACK header compression/decompression
│   ├── stream/           # QUIC stream ↔ HTTP/3 stream mapping
│   ├── frame/            # HTTP/3 frame types (DATA, HEADERS, etc.)
│   ├── connection.rs     # QUIC connection lifecycle, control streams
│   ├── settings.rs       # HTTP/3 SETTINGS negotiation
│   └── quic.rs           # QUIC transport trait abstraction (backend-agnostic)
├── quic/                 # QUIC transport backend (replicated from quinn-proto or similar)
│   ├── mod.rs
│   ├── connection.rs     # QUIC connection state machine
│   ├── stream.rs         # QUIC stream management
│   ├── crypto.rs         # TLS 1.3 integration
│   └── packet.rs         # QUIC packet codec
└── netcap/               # shared: TCP, TLS/SSL, RawStream (used by all)
```

Same integration pattern as HTTP/2: produces `SimpleIncomingRequest` / consumes `SimpleOutgoingResponse`. Handlers see the same types. `ConnectionHandler` in foundation_http adds a third branch for HTTP/3 alongside HTTP/1.1 and HTTP/2. `Proto::HTTP30` variant already exists in the `Proto` enum.

**Estimated scope:** Phase 3, after HTTP/2 is stable. 6-10 features (larger than HTTP/2 because it includes the QUIC transport layer). Can be phased: (1) QUIC transport backend, (2) HTTP/3 frame codec + QPACK, (3) server-side integration, (4) client-side integration.

### B10. iroh peer-to-peer connectivity module (Phase 3+)
**Affects:** Decision 01
[iroh](https://crates.io/crates/iroh) is a peer-to-peer QUIC connectivity library — direct connections dialed by public key with automatic hole punching and relay server fallback. It depends on tokio, but parts of it can be used directly or wrapped with Valtron's async bridging where needed (wrapping futures with Valtron's progress-driven model, or using `block_on` at the boundary).

**Source:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.WebTransport/src.n0-computer/iroh/`

**Approach:** Add a separate `iroh/` module in foundation_netio that integrates iroh's peer-to-peer connectivity. This is distinct from the `quic/` module (which is a raw QUIC transport replicated from quinn-proto) — iroh adds peer discovery, NAT traversal, relay fallback, and public-key-based authentication on top of QUIC. Where iroh's tokio futures surface, wrap them with Valtron adapters or bridge at the connection boundary.

**iroh stands on its own as a first-class transport.** It is not just a QUIC backend for HTTP/3 — it provides direct peer-to-peer encrypted connections, bidirectional and unidirectional QUIC streams, and relay-based connectivity that are independently useful. Applications can use iroh connections for raw stream communication, custom protocols, data synchronization, or any P2P messaging — without HTTP/3 framing on top. The `iroh/` module exposes these capabilities through foundation_netio's abstractions so the rest of the platform can use iroh connections the same way it uses TCP or TLS connections.

**Two integration paths:**
1. **Standalone P2P transport:** iroh `Endpoint` → bidirectional/unidirectional QUIC streams → application reads/writes directly. Used for custom protocols, data sync, P2P messaging, or any scenario where HTTP framing is unnecessary overhead.
2. **HTTP/3 backend:** iroh connection plugs into `http3/`'s QUIC trait abstraction → HTTP/3 framing runs over the P2P connection → ConnectRPC dispatches RPCs. Full stack: `iroh (P2P QUIC) → http3/ (HTTP/3 framing) → foundation_connectrpc (RPC dispatch)`.

**Module structure:**
```
backends/foundation_netio/src/
├── simple_http/          # HTTP/1.1
├── http2/                # HTTP/2 (replicated from h2)
├── http3/                # HTTP/3 framing (replicated from h3)
├── quic/                 # Raw QUIC transport (replicated from quinn-proto)
├── iroh/                 # P2P connectivity (wraps iroh crate)
│   ├── mod.rs
│   ├── endpoint.rs       # iroh Endpoint ↔ foundation_netio bridge
│   ├── connection.rs     # iroh Connection → QUIC trait impl for http3/
│   └── discovery.rs      # peer discovery, relay configuration
└── netcap/               # shared: TCP, TLS/SSL, RawStream
```

**Key integration:** The `iroh/` module implements the same QUIC transport traits that `http3/` expects (from `http3/quic.rs`). This means `http3/` doesn't care whether the QUIC connection came from raw `quic/` (direct UDP) or `iroh/` (P2P with hole punching + relay). Same `SimpleIncomingRequest` / `SimpleOutgoingResponse` at the top.

**Estimated scope:** 2-3 features on top of the http3 work. iroh handles the hard P2P parts; the work is bridging its tokio-based API into foundation_netio's types and implementing the QUIC trait abstraction.

### B6. Client-side body streaming is request-at-once (SIGNIFICANT)
**Affects:** Decision 07
connect-go's `duplexHTTPCall` uses `io.Pipe` to stream request body concurrently with reading the response. Foundation's HTTP client builds the full request, sends it, then reads the response. There is no pipe equivalent. Client streaming and bidi RPCs require sending request body chunks while simultaneously receiving response chunks. `SendSafeBody::Stream` iterator is consumed during request rendering with no mechanism to push new data after the request starts sending.

### B7. No ALPN negotiation in TLS layer (CRITICAL for HTTP/2)
**Affects:** Decision 01, 05
TLS backend selection (`netcap/ssl/mod.rs:23-86`) supports rustls 0.23, openssl, and native-tls, but none are configured with ALPN protocol lists. Without ALPN, the server cannot advertise `h2` support during TLS handshake. This is the standard mechanism for HTTP/2 over TLS (RFC 7301). rustls 0.23 supports it natively — it just needs wiring.

### B8. `HTTPStreams<T>` factory only creates HTTP/1.1 readers (SIGNIFICANT)
**Affects:** Decision 01
`HTTPStreams<T>` (`impls.rs:5456-5487`) is the generic factory for creating request/response readers from a connection. `next_request()` and `next_response()` always create `HttpRequestReader` / `HttpResponseReader` (HTTP/1.1 text parsers). After protocol negotiation, this factory must branch to create the appropriate parser for the negotiated protocol.

---

## 2. Protocol & Wire Format Gaps

Missing connect-go behaviors that affect wire protocol correctness and interoperability.

### P1. Missing `json; charset=utf-8` dual codec registration (CRITICAL)
**Affects:** Decision 02
connect-go registers TWO JSON codecs — `"json"` and `"json; charset=utf-8"` (`option.go:628-633`). This is handler-only. The design doc only describes a single `JsonCodec` with name `"json"`. Clients sending `Content-Type: application/json; charset=utf-8` get 415 Unsupported Media Type.

### P2. Missing `connectCodeToHTTP` mapping table (CRITICAL)
**Affects:** Decision 03, 05
Doc 05 says errors use `{error_http_status}` but never specifies the mapping. connect-go's `connectCodeToHTTP` maps e.g. `CodeCanceled` → 499, `CodeResourceExhausted` → 429, `CodeFailedPrecondition` → 400. This mapping is part of the wire protocol.

### P3. Missing `httpToCode` reverse mapping (CRITICAL)
**Affects:** Decision 05, 07
When receiving Connect unary error responses, the client maps HTTP status codes back to ConnectRPC codes. connect-go has `httpToCode()`. Neither decision doc documents this.

### P4. Missing error detail wire encoding (SIGNIFICANT)
**Affects:** Decision 03
`connectWireDetail` (`protocol_connect.go:1142-1193`) encodes details with `base64.RawStdEncoding` and includes optional `debug` field. The design doc's `ConnectError` mentions details but doesn't specify the JSON encoding format (`type`, `value` as base64, `debug` as optional JSON).

### P5. Missing `ConnectWireError` / `connectEndStreamMessage` detail structure (SIGNIFICANT)
**Affects:** Decision 03, 05
The EndStreamResponse JSON shape is shown but doesn't document the `details` array inside the error object or the wire encoding of error details.

### P6. Missing content-type canonicalization (SIGNIFICANT)
**Affects:** Decision 05
connect-go's `canonicalizeContentType` strips parameters for matching but preserves `charset`. A request with `application/json; charset=utf-8` fails to match `application/json` unless canonicalization is implemented. Affects every Content-Type comparison.

### P7. Missing `RequireConnectProtocolHeader` option (MEDIUM)
**Affects:** Decision 05
connect-go has configurable `RequireConnectProtocolHeader` that mandates `Connect-Protocol-Version: 1` on unary POST requests. Not documented.

### P8. Missing gRPC "trailers-only" response optimization (MEDIUM)
**Affects:** Decision 05
gRPC-Web has special handling for "trailers-only" responses: when no body has been written and there are no custom headers, trailers are sent as HTTP headers instead of a body envelope. Client must also handle this case. Not documented.

### P9. Missing `gRPC-Status-Details-Bin` preference over trailer headers (MEDIUM)
**Affects:** Decision 05
connect-go prefers the protobuf-encoded `Status` from `Grpc-Status-Details-Bin` over `Grpc-Status` / `Grpc-Message` when both are present. Not documented.

### P10. Missing `Accept-Encoding: identity` override for streaming (MEDIUM)
**Affects:** Decision 05
connect-go sets `Accept-Encoding: identity` on streaming request headers to prevent HTTP-level compression of already-per-message-compressed streams. Not documented.

### P11. Missing `Vary` header for cacheable GET responses (SMALL)
**Affects:** Decision 05
connect-go sets `Vary: Accept-Encoding` on GET responses. Not documented.

### P12. Missing `User-Agent` / `X-User-Agent` header setting (SMALL)
**Affects:** Decision 05, 07
connect-go sets `User-Agent` for Connect protocol and both `User-Agent` and `X-User-Agent` for gRPC-Web. Not documented.

### P13. Missing gRPC timeout max-8-digits constraint (SMALL)
**Affects:** Decision 05
`grpcParseTimeout` enforces max 8 digits. Not documented.

### P14. Missing GET idempotency + 304 Not Modified support (MEDIUM)
**Affects:** Decision 05, 07
connect-go supports `stableCodec` for deterministic serialization, URL size limits (`getURLMaxBytes`), fallback to POST, and 304 Not Modified responses. Doc 05 mentions GET superficially but nothing about these details.

### P15. Missing `NewNotModifiedError` / HTTP 304 support (MEDIUM)
**Affects:** Decision 03
connect-go defines `NewNotModifiedError` and `IsNotModifiedError` for conditional HTTP GET (304 Not Modified). No equivalent in the design.

### P16. Missing zero-length JSON payload rejection (SMALL)
**Affects:** Decision 02
connect-go explicitly rejects zero-length JSON payloads: `"zero-length payload is not a valid JSON object"`. Not mentioned in the design.

### P17. Missing `ReadMaxBytes` default divergence from connect-go (MEDIUM)
**Affects:** Decision 06
Design doc sets `read_max_bytes: 4 MiB` as default. connect-go's default is 0 (unlimited). Setting a default of 4 MiB will reject messages valid under connect-go defaults.

---

## 3. Handler & Interceptor Gaps

### H1. Server streaming handler is pull-based, connect-go is push-based (CRITICAL)
**Affects:** Decision 04, 10
connect-go's server streaming handler receives BOTH the request AND a writable `*ServerStream`, and returns `error`. The handler pushes via `stream.Send()`. Decision 10's generated trait returns `Result<(SimpleHeaders, Box<dyn StreamIterator<GreetResponse>>), ConnectError>` — the handler returns a pull iterator. The push model is more natural: the handler controls when it sends, can set headers/trailers at any point, and can abort mid-stream.

### H2. Bidi handler signature mismatch — single BidiStream vs separate iterators (CRITICAL)
**Affects:** Decision 04, 10
connect-go's bidi handler gets a single `*BidiStream` with both `Receive()` and `Send()`. Design gives separate input iterator and output iterator. connect-go allows interleaved reads and writes; the iterator model forces the framework to pull from the output iterator, meaning the handler cannot interleave reads and writes in the same function body.

### H3. Client streaming handler passes headers as separate parameter (MEDIUM)
**Affects:** Decision 10
connect-go exposes headers via `ClientStream.RequestHeader()` — they're not a separate parameter. Design passes `headers: &SimpleHeaders` alongside the stream.

### H4. Missing cardinality validation for unary RPCs (SIGNIFICANT)
**Affects:** Decision 04
connect-go's `receiveUnaryMessage` reads a second message to verify exactly one message, returning `CodeUnimplemented` for zero or multiple. The design doc's `UnaryHandler` shows no mention of this validation layer.

### H5. Missing HTTP method and content-type routing in handler dispatch (SIGNIFICANT)
**Affects:** Decision 04
connect-go's `ServeHTTP` checks bidi requires HTTP/2 (returns 505), routes by method (405), routes by Content-Type to protocol handler (415), validates GET has no body. Design defines handler traits but not how these become HTTP handlers.

### H6. Missing HTTP/1.1 bidi rejection (SIGNIFICANT)
**Affects:** Decision 08
connect-go explicitly rejects bidi streaming over HTTP/1.1 with `505 HTTP Version Not Supported` and `Connection: close`. Not mentioned in the dispatch flow.

### H7. Missing GET body rejection on server (MEDIUM)
**Affects:** Decision 08
connect-go checks that GET requests have no body and rejects with 415 if they do. Not mentioned in dispatch flow.

### H8. Missing `ProtocolHandler` split (method vs content-type checking) (MEDIUM)
**Affects:** Decision 05
connect-go separates `Methods()`, `ContentTypes()`, and `CanHandlePayload()`. Design collapses into single `can_handle()`, losing the ability to separately report allowed methods (405) vs supported content types (415).

### H9. `Stream::Init` used for EOF — contradicts actual Stream semantics (CRITICAL)
**Affects:** Decision 04
Design says `StreamIterator::next` returns `Stream::Init` to signal stream completion. But `foundation_core`'s `Stream::Init` means "stream is initializing" (the opposite). Stream completion is signaled by `Iterator::next()` returning `None`. This will confuse implementors.

### H10. Missing `compress_min_bytes` per-message check in streaming (SIGNIFICANT)
**Affects:** Decision 06
connect-go checks `compressMinBytes` per envelope: if message is too small, sends uncompressed even when compression is negotiated. Also checks `sendMaxBytes` AFTER compression. Design forward-references Decision 05 but this must be verified.

### H11. Missing error wrapping utilities (SIGNIFICANT)
**Affects:** Decision 03
connect-go has `wrapIfUncoded`, `wrapIfContextError`, `wrapIfContextDone`, `wrapIfRSTError`, `wrapIfLikelyH2CNotConfiguredError`. These map arbitrary errors to ConnectRPC codes. Design defines `ConnectError` but says nothing about how arbitrary Rust errors get mapped to codes.

### H12. Missing `CodeOf` free function (SMALL)
**Affects:** Decision 03
connect-go defines `CodeOf(err error) Code` that extracts code from any error, returning `CodeUnknown` for non-Connect errors. Design has no standalone equivalent for arbitrary errors.

### H13. Missing `Unwrap` / error chain traversal pattern (MEDIUM)
**Affects:** Decision 03
connect-go's `Error.Unwrap()` enables `errors.Is` and `errors.As`. Design defines `impl Error for ConnectError` with `source()` but doesn't address how interceptors check for `ConnectError` inside nested error chains. Explicit downcasting pattern needed.

### H14. Missing `Schema` field in Spec (SMALL)
**Affects:** Decision 04
connect-go's `Spec` has `Schema any` for protoreflect.MethodDescriptor. Design omits this.

### H15. Missing `Peer.Query` field (SMALL)
**Affects:** Decision 04
connect-go's `Peer` has `Query url.Values` (server-only). Needed for HTTP GET idempotent RPCs where request message is in query params.

### H16. Missing `Request.HTTPMethod()` (SMALL)
**Affects:** Decision 04
connect-go's `Request` has `HTTPMethod()` to distinguish GET vs POST. Design has no equivalent.

### H17. `ErrorWriter` stores single protobuf codec, not full `CodecRegistry` (MEDIUM)
**Affects:** Decision 03
connect-go's `ErrorWriter` stores only a `bufferPool` and one `protobuf` codec for gRPC status details. Design's `ErrorWriter` takes `Arc<CodecRegistry>` which is misleading and over-specified.

### H18. `RecoverInterceptor` must NOT wrap `WrapStreamingClient` (SMALL)
**Affects:** Decision 04
connect-go's `recoverHandlerInterceptor` only overrides `WrapUnary` and `WrapStreamingHandler`. It explicitly does NOT wrap streaming client. Design doesn't mention this constraint.

### H19. Interceptor chain thunk sentinel is client-side only (SMALL)
**Affects:** Decision 04
connect-go inserts thunks with `checkSentinel` for `WrapUnary` and `WrapStreamingClient`, but NOT for `WrapStreamingHandler`. Design misses this asymmetry.

### H20. `StreamingHandlerFunc` takes `StreamingHandlerConn` by value (MEDIUM)
**Affects:** Decision 04
connect-go passes `StreamingHandlerConn` by value (interface). Design uses `&mut dyn StreamingHandlerConn` — a mutable reference. Go interceptors routinely wrap the conn by embedding it. With `&mut dyn`, Rust interceptors cannot inject a wrapper without boxing.

---

## 4. Client Architecture Gaps

### C1. Missing `CallClientStreamSimple` / `CallBidiStreamSimple` APIs (MEDIUM)
**Affects:** Decision 07
connect-go provides "simple" variants that send headers immediately and return unwrapped types. Useful for simple codegen mode. Not mentioned.

### C2. Missing `CallInfo` / `NewClientContext` mechanism (MEDIUM)
**Affects:** Decision 07
connect-go's `NewClientContext` allows clients to set request headers and read response headers/trailers via context instead of `Request`/`Response` wrappers. Essential for simple generation mode and interceptors that observe metadata.

### C3. Missing `Spec` and `Peer` on client stream types (SMALL)
**Affects:** Decision 07
connect-go exposes `.Spec()` and `.Peer()` on every client stream type. Design's stream types have none.

### C4. Missing `Close()` / `CloseResponse()` on `ServerStream` (MEDIUM)
**Affects:** Decision 07
connect-go's `ServerStreamForClient.Close()` is non-blocking and closes the receive side. Matters for connection reuse. Design has only `receive()` and accessors.

### C5. Missing `Send(nil)` for header-only sends (MEDIUM)
**Affects:** Decision 07
connect-go's `BidiStreamForClient.Send(nil)` sends just headers without a body. Design's `send(&mut self, msg: &Req)` takes a reference, making nil-send impossible.

### C6. Default gzip acceptance missing from client defaults (MEDIUM)
**Affects:** Decision 07
connect-go's `newClientConfig` calls `withGzip()` by default. Design doesn't specify gzip acceptance as a default.

### C7. Handlers support multiple codecs by default, design doesn't specify (MEDIUM)
**Affects:** Decision 08
connect-go handlers register both `protoBinaryCodec` and `protoJSONCodec` by default. Design doesn't specify JSON should be registered by default.

### C8. `EnvelopeReader` should store source at construction (MEDIUM)
**Affects:** Decision 05
Design's `EnvelopeReader.read()` takes iterator by parameter on every call. connect-go stores the reader at construction time. API should store source iterator in `EnvelopeReader` at construction.

---

## 5. Router, Auth & Codegen Gaps

### R1. Procedure path leading slash mismatch (CRITICAL)
**Affects:** Decision 08, 10
connect-go's generated constants include a leading slash: `/connect.ping.v1.PingService/Ping`. Design omits it: `"connectrpc.greet.v1.GreetService/Greet"`. connect-go's handler registration returns path prefix with leading slash. Routing will fail when generated code interoperates with standard clients sending `POST /package.Service/Method`.

### R2. Missing `UnimplementedServiceHandler` generation (MEDIUM)
**Affects:** Decision 10
connect-go always generates `UnimplementedPingServiceHandler` returning `CodeUnimplemented` for every method. Acknowledged as open question but not included in generated output.

### R3. Missing `PingServiceName` constant (SMALL)
**Affects:** Decision 10
connect-go generates a service name constant (fully-qualified). Design only generates per-method procedure constants.

### R4. Missing client trait/interface generation for mocking (MEDIUM)
**Affects:** Decision 10
connect-go generates `PingServiceClient` as an interface alongside the struct for test mocking. Design only generates the struct.

### R5. Missing `WithSchema` option and schema propagation (SMALL)
**Affects:** Decision 07, 08, 10
connect-go passes `WithSchema(methodDescriptor)` to every handler and client constructor. Used by interceptors and dynamic message construction. Not in design.

### R6. Missing `WithConditionalHandlerOptions` (SMALL)
**Affects:** Decision 08
connect-go provides per-procedure option customization via a callback that inspects each `Spec`. Missing from design.

### R7. `extract_bearer_token` in foundation_auth is NOT fully case-insensitive (SIGNIFICANT)
**Affects:** Decision 09
foundation_auth only matches `"Bearer "` or `"bearer "`, not RFC 9110-required case-insensitive match. Design's `bearer_token` helper correctly uses `eq_ignore_ascii_case` but duplicates rather than reuses foundation_auth.

### R8. `has_scope` signature mismatch with foundation_auth (SIGNIFICANT)
**Affects:** Decision 09
Design shows `has_scope(claims, scope)` taking `VerifiedClaims` + single string. Real API takes `(&AuthContext, &[&str])`.

### R9. `SessionManager` is generic, not a concrete type (MEDIUM)
**Affects:** Decision 09
`SessionManager<S: CredentialStore>` requires `SessionAuthenticator` to be generic too, or use type-erased variant that doesn't exist.

### R10. `extract_session_token` takes cookies slice, not request (MEDIUM)
**Affects:** Decision 09
Real API: `extract_session_token(cookies: &[&str], cookie_name: &str)`. Design calls it with `SimpleIncomingRequest`.

### R11. `InferProtocol` returns `"grpcweb"` but connect-go uses `"grpc-web"` (SMALL)
**Affects:** Decision 09
String value mismatch. Connect-go uses hyphenated `"grpc-web"`.

### R12. foundation_connectrpc hard-depends on foundation_auth (MEDIUM)
**Affects:** Decision 10
Putting auth middleware inside `foundation_connectrpc/src/auth/` creates dependency chain `foundation_connectrpc → foundation_auth → foundation_db`. Should be feature-gated or separate crate.

### R13. Missing JWKS constructors on JwtVerifier (MEDIUM)
**Affects:** Decision 09
Design proposes `from_jwks_url` and `from_oidc_discovery` on `JwtVerifier`. Real `JwtVerifier` only has `from_config`. `JwksManager` is async-only, but ConnectRPC middleware is synchronous.

### R14. `ClientOptions.clone()` called in generated code but not derivable (SMALL)
**Affects:** Decision 10
Generated client constructor calls `options.clone()` four times. `ClientOptions` contains `Vec<Arc<dyn Interceptor>>` — needs explicit `Clone` impl.

---

## 6. Rust-Specific Type System Issues

### RS1. `Box<dyn StreamIterator<T>>` cannot work as written (CRITICAL)
**Affects:** Decision 04
The real `StreamIterator` in `foundation_core` is:
```rust
pub trait StreamIterator: Iterator<Item = Stream<Self::D, Self::P>> {
    type D;
    type P;
}
```
It has two associated types and is a supertrait of `Iterator`. It cannot be parameterized as `StreamIterator<T>`. The design invents `StreamIterator<T>` with `fn next(&mut self) -> Stream<Result<T, ConnectError>, ()>` that doesn't exist. Either: (1) define a new ConnectRPC-specific trait, breaking the "compatible with valtron" claim, or (2) use `Box<dyn Iterator<Item = Stream<Result<T, ConnectError>, ()>> + Send>` directly.

### RS2. `MessageRef`/`MessageMut` via `dyn Any` blocks zero-copy views (SIGNIFICANT)
**Affects:** Decision 02
`std::any::Any` requires `'static`. buffa's `MessageView<'a>` is not `'static`. You cannot downcast `&dyn Any` to `&MessageView<'a>`. This is a hard constraint — zero-copy view handlers are fundamentally incompatible with the `dyn Any` approach. Should be stated as a known limitation, not an open question.

### RS3. `UnaryFunc` type signatures are not valid Rust (CRITICAL)
**Affects:** Decision 04
```rust
pub type UnaryFunc = Box<dyn Fn(&RequestContext, AnyRequest) -> Result<AnyResponse, ConnectError> + Send + Sync>;
```
`AnyRequest` and `AnyResponse` are traits, not concrete types. Bare trait function parameters are not valid. Must be `Box<dyn AnyRequest>` or `&dyn AnyRequest`. Return must be `Result<Box<dyn AnyResponse>, ConnectError>`.

### RS4. `catch_unwind` requires `UnwindSafe` bound (MEDIUM)
**Affects:** Decision 04
Handler closures capturing `&RequestContext`, `&mut dyn StreamingHandlerConn` are NOT `UnwindSafe`. Need `AssertUnwindSafe` wrapper. After panic recovery, connection should be considered poisoned.

### RS5. `AnyRequest`/`AnyResponse` sealed trait pattern not addressed (SMALL)
**Affects:** Decision 04
connect-go seals `AnyRequest` with `internalOnly()`. Design makes them public traits. Should either document as intentional divergence or use sealed trait pattern.

### RS6. `BufferPool` as `Vec<Vec<u8>>` is not equivalent to `sync.Pool` (MEDIUM)
**Affects:** Decision 06
Go's `sync.Pool` is lock-free, thread-safe, auto-shrinks. `Vec<Vec<u8>>` in `Mutex` has contention. For valtron's worker model, thread-local pools avoid contention but buffers can't migrate between workers. Needs a concrete decision.

### RS7. `HashMap` registries mutable after construction — data race risk (SIGNIFICANT)
**Affects:** Decision 02, 06
Both `CodecRegistry` and `CompressionRegistry` have `pub fn register(&mut self, ...)` suggesting post-construction mutation. connect-go uses read-only copies (`readOnlyCodecs`). If mutated after handlers are built (which hold `Arc` references), there will be data races. Should be frozen after construction (builder pattern).

### RS8. `MarshalAppend` optimization dropped (MEDIUM)
**Affects:** Decision 02
connect-go's `marshalAppender` interface avoids allocation by appending into pooled buffers. Design's `Codec` trait only has `fn marshal(...) -> Result<Vec<u8>, ...>` which always allocates. Matters for high-throughput paths.

### RS9. `dyn Any + Send + Sync` vs `dyn Any + Send` inconsistency (SMALL)
**Affects:** Decision 02, 04
`MessageRef: Send + Sync` vs `MessageMut: Send` (no Sync). `StreamingHandlerConn` send/receive use `Box<dyn Any + Send>`. `AnyRequest`/`AnyResponse` are `Send` only. Should clarify that per-call construction resolves the thread-safety concern.

### RS10. Compression uses batch API, not streaming API (MEDIUM)
**Affects:** Decision 06
connect-go uses streaming `io.Reader`/`io.Writer` compressor/decompressor with `sync.Pool` via `Reset`. Design uses batch `fn compress(&self, input: &[u8]) -> Result<Vec<u8>>`. No reuse of compressor state, no streaming compression, no pooling.

---

## 7. Open Questions Requiring Decisions

### Q1. How do streaming handlers write partial responses?
The `Serve` trait returns `ConnectionResult` after completion. Streaming handlers need to write headers then messages incrementally. Options: new `StreamingServe` trait with `&mut dyn Write`? Use `ConnectionResult::Take` for full connection ownership? This is the core architectural question.

### Q2. How does bidi streaming work on a single valtron worker thread?
connect-go's bidi allows `Send()` and `Receive()` from different goroutines concurrently. In a single-threaded model, you can't simultaneously consume the request stream and produce responses. Handler must alternate reads and writes — fundamentally different programming model. This shapes the entire API.

### Q3. What is the concrete `AnyRequest`/`AnyResponse` implementation?
Design needs to show `impl AnyRequest for Request<T>` and confirm the `Any` downcasting round-trip works. Does `any_ref()` return `&T` (message) or `&Request<T>` (wrapper)? connect-go returns the message.

### Q4. How does `ErrorWriter` determine protocol from `SimpleIncomingRequest`?
connect-go inspects Content-Type, HTTP method, and Connect protocol version header. Does `SimpleIncomingRequest` expose all needed accessors?

### Q5. JSON codec: protobuf JSON vs serde JSON ambiguity
connect-go's `protoJSONCodec` ONLY works with `proto.Message`. Design introduces dual-path `JsonCodec` for both buffa messages and arbitrary serde types. For protocol compliance, JSON must follow protobuf canonical mapping (lowerCamelCase fields, string enums). Non-buffa serde types don't follow these rules. This is a platform extension that may break interop.

### Q6. What is the streaming transport abstraction for clients?
`Transport::round_trip` returns the full response. For server streaming / bidi, client needs incremental response body reading. connect-go uses `duplexHTTPCall` with `io.Pipe`. What is the Valtron equivalent?

### Q7. Should `ReadMaxBytes` default match connect-go (unlimited) or be safer (4 MiB)?
Design diverges from connect-go's default. Must be an explicit, documented choice.

### Q8. Push vs pull streaming model — which do we adopt?
connect-go uses push (handler calls `stream.Send()`). Design uses pull (handler returns iterator). Push is more natural for server streaming (handler controls timing, can set headers/trailers at any point, can abort mid-stream). Pull is simpler to implement. This is the single most important API design decision remaining.

### Q9. Foundation_auth gaps — fix upstream or bridge in connectrpc?
Multiple mismatches: `extract_bearer_token` not case-insensitive, `has_scope` signature different, `SessionManager` is generic, `extract_session_token` takes cookies not request. Fix in foundation_auth or write bridge code?

### Q10. Router consumption semantics
`into_handler(self)` consumes the Router. Is modification after construction explicitly prevented? Should be documented.

---

## 8. Multi-Protocol Transport Gaps

New gaps introduced by the HTTP/2, HTTP/3, QUIC, and iroh additions. These affect both the ConnectRPC design decisions and foundation_netio/foundation_http architecture.

### T1. Decision 01 only covers HTTP/1.1 transport mapping (CRITICAL)
**Affects:** Decision 01
Decision 01 maps connect-go's `net/http` onto `SimpleIncomingRequest`/`SimpleOutgoingResponse` assuming HTTP/1.1 throughout. With three protocol modules (`simple_http/`, `http2/`, `http3/`), Decision 01 must be updated to describe how the transport layer selects and bridges each protocol version. The `Transport` trait (client-side) and `Serve` trait (server-side) need protocol-version-aware variants or a unified abstraction that works across all three.

### T2. No protocol negotiation strategy documented (CRITICAL)
**Affects:** Decision 01, 05
The server must select HTTP version per-connection. Three negotiation mechanisms needed:
1. **ALPN** for HTTP/2 over TLS (`h2` vs `http/1.1`)
2. **h2c upgrade** for HTTP/2 over cleartext (HTTP Upgrade or prior knowledge via magic prefix)
3. **Alt-Svc** header for HTTP/3 discovery (server advertises `h3` availability, client upgrades on next connection)

None of these are documented. `ConnectionHandler` in foundation_http needs a protocol detection step before dispatching to the appropriate parser module.

### T3. `ConnectionHandler` needs protocol-branching architecture (SIGNIFICANT)
**Affects:** Decision 08
Currently `ConnectionHandler` is a single sequential state machine. With three protocol modules it needs:
```
Connection arrives →
  ├── TLS with ALPN "h2" → http2/ module → per-stream dispatch to Router
  ├── TLS with ALPN "http/1.1" (or no ALPN) → simple_http/ module → sequential dispatch
  ├── Cleartext with h2c magic prefix → http2/ module
  ├── Cleartext without magic → simple_http/ module
  └── QUIC (UDP) → http3/ module → per-stream dispatch to Router
```
iroh connections arrive via QUIC but with peer-identity context (public key) that must be propagated to handlers/auth.

### T4. gRPC wire protocol requires HTTP/2 trailing HEADERS — design must specify the Phase 1 workaround (SIGNIFICANT)
**Affects:** Decision 05
With HTTP/2 as Phase 2, the design must be explicit about what gRPC support looks like in Phase 1 (HTTP/1.1 only):
- **gRPC protocol**: not supported in Phase 1 (requires HTTP/2 trailing HEADERS frames). Must be documented as a known limitation, not left ambiguous.
- **gRPC-Web protocol**: works over HTTP/1.1 (trailers encoded in body). Full support in Phase 1.
- **Connect protocol**: works over HTTP/1.1. Full support in Phase 1.

### T5. HTTP/2 enables true full-duplex bidi — handler model must account for this (SIGNIFICANT)
**Affects:** Decision 04, Q2
Over HTTP/1.1, bidi is half-duplex (send all, then receive all). Over HTTP/2, bidi is full-duplex (concurrent send/receive on the same stream). The handler model (Q8 — push vs pull) must work for both modes. If we choose push (connect-go style with `stream.Send()`/`stream.Receive()`), the handler can be protocol-agnostic. If we choose pull (separate iterators), the framework must somehow interleave iteration of input and output — which is impossible in a single-threaded context without cooperative scheduling.

### T6. HTTP/2 server push not addressed (MEDIUM)
**Affects:** Decision 05
HTTP/2 supports server push (PUSH_PROMISE frames). ConnectRPC doesn't use this, but foundation_http's HTTP/2 module should handle PUSH_PROMISE frames gracefully (reject or ignore) even if not exposed. The design should explicitly state server push is not supported for ConnectRPC.

### T7. Client `Transport` trait must support multiple HTTP versions (SIGNIFICANT)
**Affects:** Decision 07
The `Transport` trait currently defines `round_trip(SimpleOutgoingRequest) -> Result<SimpleIncomingResponse>`. This assumes a single request-response exchange. For HTTP/2 and HTTP/3:
- Multiplexed connections: multiple concurrent RPCs share one connection
- Client needs connection pooling that is HTTP-version-aware
- Stream-level operations (reset, flow control) need to be exposed for streaming RPCs
- The client must know which HTTP version to use (Connect+gRPC-Web work on any; gRPC requires HTTP/2+)

The `Transport` trait may need a `StreamingTransport` extension or the streaming client types (`ServerStream`, `BidiStream`) need protocol-aware internals.

### T8. `SimpleHeaders` must handle HTTP/2 pseudo-headers (SIGNIFICANT)
**Affects:** Decision 05
HTTP/2 uses pseudo-headers (`:method`, `:path`, `:scheme`, `:authority`, `:status`) that are distinct from regular headers. `SimpleHeaders` currently stores headers in a `BTreeMap<SimpleHeader, Vec<String>>` with known variants (`CONTENT_TYPE`, `HOST`, etc.) and `Custom(String)`. The http2 module must:
1. Map `:method` → `SimpleIncomingRequest.method`, not a header
2. Map `:path` → `SimpleIncomingRequest.url`, not a header
3. Map `:authority` → `Host` header equivalent
4. Map `:status` → `SimpleOutgoingResponse.status`, not a header
5. Reject pseudo-headers in regular header positions and vice versa

This mapping happens inside the http2 module before producing `SimpleIncomingRequest`, so handlers never see pseudo-headers — but the mapping logic must be correct.

### T9. QUIC transport traits need synchronous equivalents (MEDIUM)
**Affects:** B9
h3's QUIC trait abstraction (`Connection`, `OpenStreams`, `SendStream`, `RecvStream`, `BidiStream`) uses `Poll`-based async APIs. The tokio-free reimplementation needs synchronous equivalents:
- `poll_accept_recv` → blocking or iterator-based `accept_recv`
- `poll_data` → blocking `read` or `Iterator::next`
- `poll_ready` / `send_data` → blocking `write`
- Flow control backpressure via blocking instead of `Poll::Pending`

The trait design must work for both the raw `quic/` backend and the `iroh/` wrapper.

### T10. iroh public-key identity must flow into auth context (MEDIUM)
**Affects:** Decision 09
iroh connections are authenticated by public key (Ed25519), not certificates or tokens. When ConnectRPC runs over iroh (either standalone or via HTTP/3), the peer's public key should be available in `RequestContext` / auth extensions. This is a new auth mechanism not covered by Decision 09's JWT/session/OAuth authenticators. A `PublicKeyAuthenticator` or similar should be possible, and `Peer` should carry the public key when the connection is iroh-based.

### T11. No design for multi-transport server (listening on TCP + QUIC + iroh simultaneously) (MEDIUM)
**Affects:** Decision 08
A production ConnectRPC server may listen on:
- TCP port (HTTP/1.1 + HTTP/2 via ALPN)
- UDP port (HTTP/3 via QUIC)
- iroh endpoint (P2P via public key)

All three should dispatch to the same `Router`. foundation_http's `HttpServer` currently only accepts TCP. The server architecture needs to support multiple listeners dispatching to a shared router, or separate servers sharing a router via `Arc`.

### T12. Client protocol selection strategy not documented (MEDIUM)
**Affects:** Decision 07
The client needs a strategy for choosing HTTP version:
- **Connect protocol**: works on HTTP/1.1, HTTP/2, HTTP/3 — prefer highest available
- **gRPC-Web**: works on HTTP/1.1, HTTP/2, HTTP/3 — prefer highest available
- **gRPC**: requires HTTP/2 or HTTP/3 — error if only HTTP/1.1 available
- **iroh**: always QUIC, can layer HTTP/3 or use raw streams

`ClientOptions` needs `with_preferred_http_version()` or automatic negotiation. connect-go doesn't handle this (Go's `net/http` auto-negotiates), but we must be explicit since we have separate protocol modules.

### T13. h2 source reference path not recorded (SMALL)
**Affects:** B5
The h2 crate source being used as the reference implementation for the HTTP/2 module — record its location if available locally, similar to how h3 and iroh sources are recorded.

### T14. quinn-proto as potential QUIC backend not evaluated (MEDIUM)
**Affects:** B9
quinn-proto is the protocol-only layer of quinn — it handles QUIC state machines without any async runtime dependency. It's transport-agnostic: you feed it bytes and timers, it tells you what bytes to send. This could be a better fit than a full rewrite for the `quic/` module. Needs evaluation: does quinn-proto's API map cleanly onto foundation_netio's `SharedByteBufferStream<RawStream>` model?

---

## 9. Open Questions — Transport & Multi-Protocol

### Q11. Protocol phasing strategy — what ships when?
Proposed phasing from the transport analysis:
- **Phase 1**: HTTP/1.1 only. Connect protocol + gRPC-Web. No gRPC (requires HTTP/2). Half-duplex bidi only.
- **Phase 2**: HTTP/2 (replicated from h2). Full gRPC support. Full-duplex bidi. HTTP/2 client with multiplexing.
- **Phase 3**: HTTP/3 (replicated from h3) + QUIC backend. All protocols over QUIC.
- **Phase 3+**: iroh integration. P2P ConnectRPC. Public-key auth.

Is this phasing correct? Should gRPC-Web be deferred to Phase 2 as well, or is it safe for Phase 1?

### Q12. Does foundation_http need a `Listener` abstraction?
Currently `HttpServer` binds a TCP listener. With QUIC (UDP) and iroh (P2P), the server needs multiple listener types. Options:
1. Separate server instances per transport, sharing a `Router` via `Arc`
2. A `Listener` trait abstracting over TCP, QUIC, iroh — server accepts from any
3. A multi-listener server that polls all sources

### Q13. How does connection-level state (HTTP version, TLS info, peer identity) reach handlers?
Handlers receive `SimpleIncomingRequest` which has `proto: Proto`. But they may also need:
- TLS certificate info (for mTLS)
- HTTP/2 stream ID (for logging/tracing)
- iroh public key (for P2P auth)
- QUIC connection ID
- Whether 0-RTT was used (security implications)

Should these go in `RequestContext.extensions`, `Peer`, or `SimpleIncomingRequest` fields?

### Q14. Should the `http2/` and `http3/` modules be in foundation_netio or separate crates?
They're large subsystems (5-8 and 6-10 features respectively). Putting them in foundation_netio keeps the transport layer unified but makes the crate large. Separate crates (`foundation_http2`, `foundation_http3`) keep compilation fast but fragment the network stack. Feature flags on foundation_netio could be a middle ground.

---

## Summary Counts

| Category | Critical | Significant | Medium | Small | Total |
|---|---|---|---|---|---|
| Platform Blockers | 4 | 3 | 2 | 0 | 10 |
| Protocol/Wire Gaps | 3 | 3 | 5 | 4 | 17 |
| Handler/Interceptor | 3 | 4 | 5 | 5 | 20 |
| Client Architecture | 0 | 0 | 6 | 1 | 8 |
| Router/Auth/Codegen | 1 | 2 | 6 | 4 | 14 |
| Rust Type System | 2 | 2 | 4 | 2 | 10 |
| Multi-Protocol Transport | 2 | 4 | 5 | 1 | 14 |
| **Total** | **15** | **18** | **33** | **17** | **93** |

The 15 critical items must be resolved before feature specs can be written. The most impactful are:
1. **B1/B2**: Foundation streaming response model (blocks all streaming RPCs)
2. **B5/B7**: HTTP/2 support — no binary framing, no ALPN, no stream multiplexing (blocks gRPC, full-duplex bidi)
3. **T1/T2**: Transport decisions don't cover multi-protocol — design must specify negotiation and bridging
4. **H1/H2/Q8**: Push vs pull handler model (shapes the entire API)
5. **RS1/RS3**: Invalid Rust type signatures (design doesn't compile)
6. **H9**: Wrong EOF signaling (contradicts actual foundation_core semantics)
7. **R1**: Leading slash mismatch (breaks routing)
8. **P1/P2/P3**: Missing wire format mappings (breaks interop)
