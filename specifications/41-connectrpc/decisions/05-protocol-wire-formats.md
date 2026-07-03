# Decision 05: Protocol Wire Formats

## Context

ConnectRPC supports three wire protocols on the same server/client. Each uses HTTP differently but carries the same RPC semantics. The protocol is detected from the `Content-Type` header on incoming requests. This document captures the exact wire format for each protocol so implementation is unambiguous.

### Protocol Detection (from connect-go)

The server examines `Content-Type` and HTTP method to determine protocol:

| Content-Type | Method | Protocol | Codec | Streaming |
|---|---|---|---|---|
| `application/grpc` | POST | gRPC | proto | depends on method |
| `application/grpc+proto` | POST | gRPC | proto | depends on method |
| `application/grpc+json` | POST | gRPC | json | depends on method |
| `application/grpc-web` | POST | gRPC-Web | proto | depends on method |
| `application/grpc-web+proto` | POST | gRPC-Web | proto | depends on method |
| `application/grpc-web+json` | POST | gRPC-Web | json | depends on method |
| `application/grpc-web-text` | POST | gRPC-Web | proto | text mode (base64) |
| `application/grpc-web-text+proto` | POST | gRPC-Web | proto | text mode (base64) |
| `application/connect+proto` | POST | Connect | proto | yes (streaming) |
| `application/connect+json` | POST | Connect | json | yes (streaming) |
| `application/proto` | POST | Connect | proto | no (unary) |
| `application/json` | POST | Connect | json | no (unary) |
| _(query: encoding=X, message=Y)_ | GET | Connect | X | no (unary GET) |

## Decision

### Shared: Envelope Framing

All three protocols use the same 5-byte envelope framing for streaming messages:

```
+--------+-------------------+---------+
| Flags  | Message Length    | Message |
| 1 byte | 4 bytes (BE u32) | N bytes |
+--------+-------------------+---------+
```

**Flags byte:**
- Bit 0 (0x01): Message is compressed using the negotiated algorithm
- Bit 1 (0x02): **Connect only** — this is an EndStreamResponse (final message)
- Bit 7 (0x80): **gRPC-Web only** — this frame contains trailers, not a message
- Bits 2-6: Reserved

```rust
pub struct Envelope {
    pub flags: u8,
    /// Zero-copy slice of the arriving buffer (`Bytes`, not `Vec<u8>`): the read path is
    /// `Bytes` end-to-end so the facade can build `OwnedView`/`RecordBatch` without a copy
    /// (Decision 11 S5).
    pub data: Bytes,
}

impl Envelope {
    pub const FLAG_COMPRESSED: u8 = 0x01;
    pub const FLAG_END_STREAM: u8 = 0x02;     // Connect protocol
    pub const FLAG_TRAILER: u8 = 0x80;          // gRPC-Web protocol

    pub fn is_compressed(&self) -> bool { self.flags & Self::FLAG_COMPRESSED != 0 }
    pub fn is_end_stream(&self) -> bool { self.flags & Self::FLAG_END_STREAM != 0 }
    pub fn is_trailer(&self) -> bool { self.flags & Self::FLAG_TRAILER != 0 }

    /// Encode into a pooled buffer (write path, RS8): 5-byte header + data. The write path
    /// deliberately stays on pooled `Vec<u8>` scratch — encoding produces new bytes anyway;
    /// zero-copy matters on decode.
    pub fn encode_into(&self, buf: &mut Vec<u8>);

    /// Decode one envelope from the head of `data`, slicing the payload **zero-copy**
    /// (`Bytes::slice` — refcount bump, no memcpy). Partial input is handled by the
    /// `IncrementalDecoder` wrapper (Decision 12 §11): state is retained and a short read
    /// yields `Pending`, never an error.
    pub fn decode(data: &Bytes) -> Result<(Envelope, usize), EnvelopeError>;
}
```

### EnvelopeReader / EnvelopeWriter

```rust
/// Reads envelopes from the body byte stream, handling decompression. **Frame-level only** —
/// yields codec-encoded frame `Bytes`, never decodes messages: typed decode lives in the
/// `MessageSource<Req>` facade (Decision 11), which holds the `Arc<dyn CodecFor<Req>>`
/// (Decision 02).
pub struct EnvelopeReader {
    decompressor: Option<Arc<dyn Compressor>>,   // Decision 06 — one trait, `Compressor`
    read_max_bytes: usize,
    // Accumulation lives in the shared `IncrementalDecoder` state (Decision 12 §11); its
    // buffer hands completed payloads out as `Bytes` slices (freeze/split), so
    // de-enveloping never copies the message body. (Decompressed frames are new
    // allocations by nature; identity frames stay zero-copy.)
}

impl EnvelopeReader {
    /// Next de-enveloped, decompressed frame as `Bytes`, or `None` at end of stream.
    // Source is stored at construction (C8), so `read` takes no source param.
    //
    // On a non-blocking fd the 5-byte prefix + body can span multiple reads, so the reader
    // is implemented over the shared `IncrementalDecoder` primitive (Decision 12 §11) —
    // partial state is retained and a short read yields `Pending`, not an error. The blocking
    // `read` here is the `loop { step }`-until-frame wrapper over it.
    pub fn read(&mut self) -> ConnectResult<Option<Bytes>>;
}

/// Writes envelopes to a byte sink, handling compression. Frame-level only: takes an
/// already-encoded frame (the `MessageSink<Res>` facade did the typed encode).
pub struct EnvelopeWriter {
    compressor: Option<Arc<dyn Compressor>>,
    compress_min_bytes: usize,
    send_max_bytes: usize,
}

impl EnvelopeWriter {
    /// Envelope one already-encoded frame (compress iff negotiated and ≥ compress_min_bytes).
    pub fn write(&self, frame: Bytes) -> ConnectResult<Vec<u8>>;

    /// Write an EndStreamResponse (Connect protocol).
    pub fn write_end_stream(&self, error: Option<&ErrorTrace<ConnectError>>, trailers: &SimpleHeaders) -> ConnectResult<Vec<u8>>;

    /// Write a trailer frame (gRPC-Web protocol).
    pub fn write_trailer_frame(&self, trailers: &SimpleHeaders) -> ConnectResult<Vec<u8>>;
}
```

---

### Protocol 1: Connect

#### Unary POST Request
```
POST /{service}/{method} HTTP/1.1
Content-Type: application/{codec}
Connect-Protocol-Version: 1
Connect-Timeout-Ms: {milliseconds}
Content-Encoding: {compression}
Accept-Encoding: {compression_list}
{custom-header}: {value}

{bare message bytes, possibly compressed}
```

#### Unary POST Response (Success)
```
HTTP/1.1 200 OK
Content-Type: application/{codec}
Content-Encoding: {compression}
Trailer-{key}: {value}

{bare message bytes, possibly compressed}
```

#### Unary POST Response (Error)
```
HTTP/1.1 {error_http_status}
Content-Type: application/json

{"code": "{code}", "message": "{msg}", "details": [...]}
```

Error responses are ALWAYS `application/json` regardless of request codec.

#### Unary GET Request
```
GET /{service}/{method}?encoding={codec}&message={url_encoded_or_base64}&base64=1&compression={algo}&connect=v1 HTTP/1.1
Connect-Timeout-Ms: {milliseconds}
Accept-Encoding: {compression_list}
```

#### Streaming Request
```
POST /{service}/{method} HTTP/1.1
Content-Type: application/connect+{codec}
Connect-Protocol-Version: 1
Connect-Timeout-Ms: {milliseconds}
Connect-Content-Encoding: {compression}
Connect-Accept-Encoding: {compression_list}

[envelope][envelope]...[envelope]
```

Note: Streaming uses `Connect-Content-Encoding` / `Connect-Accept-Encoding` (not standard `Content-Encoding`).

#### Streaming Response
```
HTTP/1.1 200 OK
Content-Type: application/connect+{codec}
Connect-Content-Encoding: {compression}
Connect-Accept-Encoding: {compression_list}

[envelope: message][envelope: message]...[envelope: EndStreamResponse (flags=0x02)]
```

**Always HTTP 200.** Errors are in the EndStreamResponse. The final envelope has flags bit 1 set and contains JSON:

```json
{"error": {"code": "unavailable", "message": "..."}, "metadata": {"key": ["val"]}}
```

Or on success: `{}`

#### Connect Protocol Constants

```rust
pub mod connect_protocol {
    pub const HEADER_PROTOCOL_VERSION: &str = "connect-protocol-version";
    pub const PROTOCOL_VERSION: &str = "1";
    pub const HEADER_TIMEOUT: &str = "connect-timeout-ms";
    pub const HEADER_STREAMING_CONTENT_ENCODING: &str = "connect-content-encoding";
    pub const HEADER_STREAMING_ACCEPT_ENCODING: &str = "connect-accept-encoding";

    pub const QUERY_ENCODING: &str = "encoding";
    pub const QUERY_MESSAGE: &str = "message";
    pub const QUERY_BASE64: &str = "base64";
    pub const QUERY_COMPRESSION: &str = "compression";
    pub const QUERY_CONNECT_VERSION: &str = "connect";
    pub const QUERY_CONNECT_VERSION_VALUE: &str = "v1";

    pub const TRAILER_HEADER_PREFIX: &str = "trailer-";
}
```

---

### Protocol 2: gRPC

#### Request
```
POST /{service}/{method} HTTP/2
Content-Type: application/grpc+{codec}
Grpc-Timeout: {value}{unit}
Grpc-Encoding: {compression}
Grpc-Accept-Encoding: {compression_list}
Te: trailers

[envelope][envelope]...
```

ALL gRPC requests use envelope framing, even unary (single envelope).

#### Timeout encoding
| Duration | Wire format |
|---|---|
| 100ns | `100n` |
| 1μs | `1u` |
| 1ms | `1m` |
| 1s | `1S` |
| 1min | `1M` |
| 1hr | `1H` |

```rust
pub fn encode_grpc_timeout(duration: Duration) -> String;
pub fn decode_grpc_timeout(header: &str) -> Result<Duration, ParseError>;
```

#### Response (Success)
```
HTTP/2 200 OK
Content-Type: application/grpc+{codec}
Grpc-Encoding: {compression}
Grpc-Accept-Encoding: {compression_list}

[envelope][envelope]...

--- HTTP/2 trailers ---
Grpc-Status: 0
{custom-trailer}: {value}
```

#### Response (Error)
```
HTTP/2 200 OK
Content-Type: application/grpc+{codec}

--- HTTP/2 trailers ---
Grpc-Status: {numeric_code}
Grpc-Message: {percent_encoded_message}
Grpc-Status-Details-Bin: {base64_encoded_status_proto}
```

HTTP status is ALWAYS 200. Status is in trailers.

#### gRPC Trailer Encoding

`Grpc-Message` is percent-encoded (RFC 3986).
`Grpc-Status-Details-Bin` is base64-encoded `google.rpc.Status` protobuf:

```protobuf
message Status {
    int32 code = 1;
    string message = 2;
    repeated google.protobuf.Any details = 3;
}
```

#### gRPC Protocol Constants

```rust
pub mod grpc_protocol {
    pub const HEADER_TIMEOUT: &str = "grpc-timeout";
    pub const HEADER_ENCODING: &str = "grpc-encoding";
    pub const HEADER_ACCEPT_ENCODING: &str = "grpc-accept-encoding";
    pub const TRAILER_STATUS: &str = "grpc-status";
    pub const TRAILER_MESSAGE: &str = "grpc-message";
    pub const TRAILER_STATUS_DETAILS: &str = "grpc-status-details-bin";
    pub const HEADER_TE: &str = "te";
    pub const TE_TRAILERS: &str = "trailers";
}
```

---

### Protocol 3: gRPC-Web

Identical to gRPC except:

1. **Content-Type**: `application/grpc-web+{codec}` instead of `application/grpc+{codec}`
2. **No HTTP/2 requirement**: Works over HTTP/1.1
3. **No `Te: trailers` header**
4. **Trailers in body**: Instead of HTTP/2 trailing HEADERS, trailers are sent as a final envelope with flag byte `0x80`:

```
[envelope: message (flags=0x00)]...[envelope: trailers (flags=0x80)]
```

The trailer frame body is HTTP header format:
```
grpc-status: 0\r\n
grpc-message: \r\n
custom-trailer: value\r\n
```

5. **Text mode**: `application/grpc-web-text` variant base64-encodes the entire body (envelope framing + messages). Used by browsers that can't handle binary response bodies.

#### gRPC-Web Protocol Constants

```rust
pub mod grpc_web_protocol {
    // Uses same headers as grpc_protocol for encoding/timeout
    // Different content-type prefix
    pub const CONTENT_TYPE_PREFIX: &str = "application/grpc-web";
    pub const CONTENT_TYPE_TEXT_PREFIX: &str = "application/grpc-web-text";
    pub const TRAILER_FLAG: u8 = 0x80;
}
```

---

### Protocol Implementation Trait

```rust
/// Internal protocol abstraction. Each protocol (Connect, gRPC, gRPC-Web)
/// implements this to handle protocol-specific HTTP semantics.
pub(crate) trait ProtocolHandler: Send + Sync {
    /// HTTP methods this protocol accepts.
    fn allowed_methods(&self) -> &[SimpleMethod];

    /// Content-Types this handler can process.
    fn content_types(&self) -> Vec<String>;

    /// Parse timeout from request headers.
    fn parse_timeout(&self, headers: &SimpleHeaders) -> Option<Duration>;

    /// Can this handler process the given request?
    fn can_handle(&self, request: &SimpleIncomingRequest) -> bool;

    /// Create the per-call exchange. The transport hands over BYTE-level body pipes (it
    /// knows nothing of envelopes — Decision 11); this constructor is where `compression`
    /// is consumed: it builds the protocol's de-envelope/decompress READER task + its
    /// envelope/compress WRITER task AND the conn over the internal `FramePipe`s between
    /// them. The dispatcher spawns the returned tasks on valtron.
    // No codec parameter: the codec authority is the procedure's ProcedureCodecs table,
    // resolved by the dispatcher (Decision 02) — there is no request-path codec registry.
    fn new_conn(
        &self,
        request: &SimpleIncomingRequest,
        body: ByteSource,          // wire request-body bytes (from the transport)
        responder: ByteSink,       // wire response-body bytes (to the transport)
        compression: &CompressionRegistry,
    ) -> ConnectResult<HandlerExchange>;
}

/// What a protocol constructs per call (Decision 11 §who-owns-enveloping). The client
/// mirror is `ClientExchange` (same shape over `ClientConn`).
pub(crate) struct HandlerExchange {
    pub conn: Box<dyn HandlerConn>,
    pub reader_task: BoxedTask,  // bytes → de-envelope/decompress → request FramePipe
    pub writer_task: BoxedTask,  // response FramePipe → envelope/compress → bytes, flush per frame
}

pub(crate) trait ProtocolClient: Send + Sync {
    /// Build request headers for an outgoing RPC.
    fn write_request_headers(
        &self,
        stream_type: StreamType,
        headers: &mut SimpleHeaders,
        codec_name: &str,          // the wire token (Decision 02); the typed handle stays in the facade
        compression: Option<&str>,
    );

    /// Create the per-call exchange over a live transport stream (`Transport::open`,
    /// Decision 11) — `stream` carries BYTE-level pipes; this constructor builds the
    /// protocol's reader/writer task pair + the conn (client mirror of `HandlerExchange`);
    /// the framework spawns the tasks.
    fn new_conn(
        &self,
        spec: &Spec,
        headers: SimpleHeaders,
        stream: TransportStream,
    ) -> ClientExchange;   // { conn: Box<dyn ClientConn>, reader_task, writer_task }
}
```

### Compression Negotiation

Shared across all protocols — only the header names differ per protocol
(`Content-Encoding` / `Connect-Content-Encoding` / `Grpc-Encoding`, and the matching
accept-encoding headers). The **single normative definition** — `negotiate_compression`
returning `NegotiatedCompression` — lives in **Decision 06**; each `ProtocolHandler` here
only maps its protocol's header names onto that one function. (An earlier duplicate
`CompressionNegotiation` sketch in this doc was removed.)

## Consequences

- Three protocol implementations share the same handler code — protocol differences are isolated
- `ProtocolHandler` / `ProtocolClient` traits are internal (not public API)
- Envelope framing is shared code; only flag interpretation differs per protocol
- Connect + gRPC-Web work on HTTP/1.1; gRPC requires HTTP/2
- Protocol detection is O(1) from Content-Type header parsing

## Review-Gap Coverage

Behaviours to implement (own the code; folded in from the review):

- **P6 — content-type canonicalization:** strip parameters for matching but preserve
  `charset`; `application/json; charset=utf-8` matches `application/json`. Applies to
  every Content-Type comparison.
- **P7 — `RequireConnectProtocolHeader`:** handler option requiring
  `Connect-Protocol-Version: 1` on unary POST.
- **P8 — gRPC-Web trailers-only:** when no body and no custom headers, send trailers as
  HTTP headers; the client handles this case.
- **P9 — status-details-bin preference:** prefer the `Grpc-Status-Details-Bin` protobuf
  `Status` over `Grpc-Status` / `Grpc-Message` when both are present.
- **P10 — streaming `Accept-Encoding: identity`:** set on streaming requests to disable
  HTTP-level compression of already-per-message-compressed streams.
- **P11 — `Vary: Accept-Encoding`** on cacheable GET responses.
- **P12 — User-Agent:** set `User-Agent` (Connect) and both `User-Agent` + `X-User-Agent`
  (gRPC-Web).
- **P13 — gRPC timeout:** enforce max 8 digits on parse.
- **P14 — GET idempotency:** stable-codec serialization, `get_url_max_bytes` (default
  8 KiB), POST fallback, and 304 support.
- **H8 — ProtocolHandler split:** expose `methods()`, `content_types()`, and
  `can_handle_payload()` separately so the dispatcher can distinguish 405 (method) from
  415 (content-type).
- **C8 — `EnvelopeReader`:** store the source iterator at construction (aligns with
  Decision 11's `MessageSource`).
- **T4 — Phase-1 scope (decided):** Connect + gRPC-Web are fully supported on HTTP/1.1;
  gRPC requires HTTP/2 and is therefore Phase 2. Enforced at runtime by Decision 11's
  capability matching.
- **T8 — pseudo-headers:** `:method`/`:path`/`:scheme`/`:authority`/`:status` are mapped
  to request/response fields inside the `http2/` module (Decision 12 §6); handlers never
  see them.

## Decided Details

1. **gRPC-Web text mode — decided: Phase 1 (implement).** Support `application/grpc-web-text`
   (whole-body base64) alongside binary gRPC-Web from the start, for maximum browser reach
   incl. legacy XHR-only clients. Implementation note: base64 operates on 3-byte groups, so for
   **streaming** the encoder/decoder must buffer partial groups across frame/chunk boundaries
   (don't assume an envelope boundary aligns to a base64 boundary) — a small stateful
   base64 stream adapter sits outermost on the body, wrapping the normal envelope framing.
2. **gRPC Status protobuf — decided.** `google.protobuf.Any` is **reused from `buffa-types`**
   (already a WKT there, with `pack`/`unpack` + canonical JSON + type-URL registry — verified).
   `google.rpc.Status` is **not** a WKT, so it is **generated from `google/rpc/status.proto`**
   via our buffa-codegen-based generator (Decision 10) and bundled in `foundation_connectrpc`.
   No hand-written wire types; matches Decision 03's "details available regardless of codec."
3. **Content-Type charset parameter — decided: normalize.** A `canonicalize_content_type`
   step strips the `charset` parameter and lowercases the media type **before** protocol/codec
   detection, so `application/json; charset=utf-8` is treated as `application/json` (connect-go
   parity). Required for browser/proxy interop — without it those requests would 415. Pairs
   with Decision 02 P1 (charset handled entirely by this canonicalization; the codec tables
   key on bare wire names — no dual registration exists).
4. **415 / 405 — decided: matched inside `ConnectRpcHandler` by parsed header.** foundation_http
   only prefix-routes to the handler; content-type negotiation is a plain header read + lookup
   because we already parse headers into `SimpleHeaders`. Inside the handler, after matching the
   procedure path: read `Content-Type`, normalize it (charset canonicalization above), and match against the procedure's
   registered protocol handlers' accepted content-types — the codec-name segment is checked
   against the procedure's `ProcedureCodecs.names()` (Decision 02; there is no global codec
   registry). **No match → 415**; method not in the
   procedure's allowed set → **405** (+ `Accept-Post`). foundation_http never inspects RPC
   content-types (it can't distinguish 415 vs 405 vs a valid Connect GET). This is the same
   dispatch path as Decision 08.
