# Decision 03: Error Model

## Context

ConnectRPC defines a structured error model shared across all three wire protocols (Connect, gRPC, gRPC-Web). Errors carry:
- A **code** (one of 16 status codes, matching gRPC)
- An optional **message** (human-readable UTF-8 string)
- Optional **details** (strongly-typed protobuf messages wrapped in `Any`)
- Optional **metadata** (key-value headers, for streaming end-of-stream)

connect-go's error types:

```go
type Code uint32  // 16 codes: Canceled(1) through Unauthenticated(16)

type Error struct {
    code    Code
    err     error              // underlying Go error
    details []*ErrorDetail     // protobuf Any messages
    meta    http.Header        // additional metadata
    wireErr bool               // true if error came from the wire (server-sent)
}

type ErrorDetail struct {
    pbAny    *anypb.Any        // protobuf Any wrapper
    pbInner  proto.Message     // cached inner message (if decoded)
    wireJSON string            // original JSON for round-trip fidelity
}
```

Key behaviors:
- `NewError(code, err)` wraps any Go error with a status code
- `NewWireError(code, err)` marks the error as server-sent (for `IsWireError` checks)
- `Error.AddDetail(detail)` attaches strongly-typed details
- `CodeOf(err)` extracts the code from any error (returns `CodeUnknown` if not a connect error)
- Error details use `google.protobuf.Any` for type-safe extensibility
- JSON serialization: `{"code": "unavailable", "message": "...", "details": [{"type": "...", "value": "..."}]}`

### Error ↔ HTTP Status Mapping

| Code | HTTP Status |
|---|---|
| `canceled` | 499 |
| `unknown` | 500 |
| `invalid_argument` | 400 |
| `deadline_exceeded` | 504 |
| `not_found` | 404 |
| `already_exists` | 409 |
| `permission_denied` | 403 |
| `resource_exhausted` | 429 |
| `failed_precondition` | 400 |
| `aborted` | 409 |
| `out_of_range` | 400 |
| `unimplemented` | 501 |
| `internal` | 500 |
| `unavailable` | 503 |
| `data_loss` | 500 |
| `unauthenticated` | 401 |

### HTTP Status → Error Code (reverse mapping for non-Connect responses)

| HTTP Status | Inferred Code |
|---|---|
| 400 | `internal` |
| 401 | `unauthenticated` |
| 403 | `permission_denied` |
| 404 | `unimplemented` |
| 429 | `unavailable` |
| 502 | `unavailable` |
| 503 | `unavailable` |
| 504 | `unavailable` |
| _all others_ | `unknown` |

## Decision

### Error Code Enum

```rust
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    Canceled = 1,
    Unknown = 2,
    InvalidArgument = 3,
    DeadlineExceeded = 4,
    NotFound = 5,
    AlreadyExists = 6,
    PermissionDenied = 7,
    ResourceExhausted = 8,
    FailedPrecondition = 9,
    Aborted = 10,
    OutOfRange = 11,
    Unimplemented = 12,
    Internal = 13,
    Unavailable = 14,
    DataLoss = 15,
    Unauthenticated = 16,
}

impl Code {
    pub fn as_str(&self) -> &'static str { /* "canceled", "unknown", etc. */ }
    pub fn from_str(s: &str) -> Option<Self> { /* parse "canceled" → Canceled */ }
    pub fn from_u32(n: u32) -> Option<Self> { /* 1 → Canceled, etc. */ }
    pub fn http_status(&self) -> u16 { /* Code → HTTP status per table above */ }
    pub fn from_http_status(status: u16) -> Self { /* HTTP status → Code per reverse table */ }
    pub fn grpc_code(&self) -> u32 { /* same numeric value — gRPC codes are identical */ }
}
```

### ConnectError

```rust
#[derive(Debug)]
pub struct ConnectError {
    code: Code,
    message: String,
    details: Vec<ErrorDetail>,
    metadata: SimpleHeaders,       // foundation_netio headers
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    wire_error: bool,
}

impl ConnectError {
    pub fn new(code: Code, message: impl Into<String>) -> Self;
    pub fn with_source(code: Code, source: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn wire(code: Code, message: impl Into<String>) -> Self; // marks as wire error

    pub fn code(&self) -> Code;
    pub fn message(&self) -> &str;
    pub fn details(&self) -> &[ErrorDetail];
    pub fn add_detail(&mut self, detail: ErrorDetail);
    pub fn metadata(&self) -> &SimpleHeaders;
    pub fn metadata_mut(&mut self) -> &mut SimpleHeaders;
    pub fn is_wire_error(&self) -> bool;

    // Convenience constructors for every code
    pub fn canceled(msg: impl Into<String>) -> Self;
    pub fn unknown(msg: impl Into<String>) -> Self;
    pub fn invalid_argument(msg: impl Into<String>) -> Self;
    pub fn deadline_exceeded(msg: impl Into<String>) -> Self;
    pub fn not_found(msg: impl Into<String>) -> Self;
    pub fn already_exists(msg: impl Into<String>) -> Self;
    pub fn permission_denied(msg: impl Into<String>) -> Self;
    pub fn resource_exhausted(msg: impl Into<String>) -> Self;
    pub fn failed_precondition(msg: impl Into<String>) -> Self;
    pub fn aborted(msg: impl Into<String>) -> Self;
    pub fn out_of_range(msg: impl Into<String>) -> Self;
    pub fn unimplemented(msg: impl Into<String>) -> Self;
    pub fn internal(msg: impl Into<String>) -> Self;
    pub fn unavailable(msg: impl Into<String>) -> Self;
    pub fn data_loss(msg: impl Into<String>) -> Self;
    pub fn unauthenticated(msg: impl Into<String>) -> Self;
}

impl std::fmt::Display for ConnectError { /* "[code] message" */ }
impl std::error::Error for ConnectError { fn source(&self) -> Option<&(dyn Error + 'static)> }
```

### ErrorDetail

```rust
#[derive(Debug, Clone)]
pub struct ErrorDetail {
    type_url: String,              // e.g. "google.rpc.RetryInfo"
    value: Vec<u8>,                // protobuf-serialized bytes
    wire_json: Option<String>,     // original JSON representation for round-trip fidelity
}

impl ErrorDetail {
    /// Create from a protobuf message. Requires buffa's Message trait.
    #[cfg(feature = "proto")]
    pub fn from_message<M: buffa::Message>(msg: &M) -> Result<Self, CodecError>;

    /// Create from raw type URL and bytes (codec-agnostic).
    pub fn from_raw(type_url: impl Into<String>, value: Vec<u8>) -> Self;

    /// The fully-qualified protobuf message type name (without URL prefix).
    pub fn type_name(&self) -> &str;

    /// Raw protobuf bytes.
    pub fn value(&self) -> &[u8];

    /// Attempt to decode into a concrete buffa message type.
    #[cfg(feature = "proto")]
    pub fn decode<M: buffa::Message + Default>(&self) -> Result<M, CodecError>;

    /// Our-stack rich diagnostics: carry an errstacks `StructuredErrorTrace` as a detail
    /// WITHOUT requiring proto. Uses an assigned well-known type_url so it round-trips
    /// between our clients/servers; standard clients simply ignore an unknown type_url.
    pub fn from_errstacks(trace: &StructuredErrorTrace) -> Self; // type_url =
        // "type.googleapis.com/foundation.errstacks.StructuredErrorTrace", value = its bytes/JSON
}
```

### Detail encoding model (decided)

Precise layering — note the two unrelated "Any" types (see below):

1. **Primary error** — `Code` + `message`: **always sent, never uses any `Any`.** Covers the
   majority of errors.
2. **Typed structured details** — *optional*, encoded as **`google.protobuf.Any`**
   (`{type_url, value}`) purely for **standard-client / cross-language interop**
   (connect-go / connect-es / grpc decode by `type_url`). Proto-generated detail types are the
   natural payloads — codegen supplies both the `type_url` (proto FQN) and the serializer.
3. **Our-stack rich diagnostics** — the errstacks `StructuredErrorTrace` rides as **one
   well-known detail entry** (`ErrorDetail::from_errstacks`) with an assigned type_url and no
   proto dependency, so our clients get the full trace while standard clients still get
   code + message (+ any `google.protobuf.Any` details) and ignore the unknown type_url.
4. **Bundling** — the minimal `google.rpc.Status` / `google.protobuf.Any` types are bundled
   **only** to make (2) codec-independent (a JSON/Arrow-only service can still emit interop
   details). If a deployment never emits typed details and never talks to a standard client,
   this machinery is dormant.

Net: **errstacks is the default error model everywhere internally; `google.protobuf.Any` is a
thin, optional wire-interop shim for typed details, not a competing error model.**

> **Two different "Any" — do not conflate.** `google.protobuf.Any` (here) is a *protobuf wire
> message* (`{type_url, value}`, serializable, cross-language, self-describing). Rust's
> `std::any::Any` (which appears in this design only in panic payloads and the `Extensions`
> type-map — the Decision 02 codec path has **no** `dyn Any`) is *in-process*
> type erasure via `TypeId` — process-local, not stable, not serializable. **You cannot build
> a `google.protobuf.Any` from a `dyn Any`** (`TypeId` is not a `type_url` and can't
> serialize); wire details therefore require proto type identity (or the assigned errstacks
> type_url in (3)), which is exactly why (2)/(3) exist. Docs write the fully-qualified name
> (`google.protobuf.Any` vs `std::any::Any`) wherever ambiguous.

### JSON Serialization (Connect Protocol)

Connect protocol errors are always JSON on the wire, regardless of the RPC codec:

```json
{
  "code": "unavailable",
  "message": "overloaded: back off and retry",
  "details": [
    {
      "type": "google.rpc.RetryInfo",
      "value": "CgIIPA",
      "debug": {"retryDelay": "30s"}
    }
  ]
}
```

Serialization:
```rust
#[derive(Serialize, Deserialize)]
struct WireError {
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Vec<WireErrorDetail>>,
}

#[derive(Serialize, Deserialize)]
struct WireErrorDetail {
    #[serde(rename = "type")]
    type_url: String,
    value: String,           // base64-encoded protobuf bytes (unpadded)
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<serde_json::Value>,
}
```

### EndStreamResponse (Streaming)

The final message in a Connect streaming response carries error + trailing metadata:

```rust
#[derive(Serialize, Deserialize)]
struct EndStreamResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<WireError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, Vec<String>>>,
}
```

### gRPC Error Encoding

gRPC uses different wire encoding:
- `grpc-status` trailer: numeric code (0-16)
- `grpc-message` trailer: percent-encoded error message
- `grpc-status-details-bin` trailer: base64-encoded `google.rpc.Status` protobuf

### Error representation = foundation_errstacks (`ErrorTrace<C>`)

There is **no separate "ErrorTrace vs errstacks"** — `ErrorTrace<C>` *is* the foundation_errstacks
type (its typed context + frame trace). So we don't invent a parallel tracing layer:

- **`ConnectError` is a context type.** The canonical error is `ErrorTrace<ConnectError>` — the
  errstacks trace carrying `ConnectError` as its context `C`, with attached frames for tracing.
- **Expected/domain errors are custom context types.** A service defines its own error enum and
  carries it as `ErrorTrace<MyDomainError>`; a `From<MyDomainError> for ConnectError` (or
  `change_context`) maps it to the RPC error at the boundary. This is exactly "represent
  expected errors via custom errors through errstacks" — no bespoke machinery.
- **JSON/`debug` serialization is already provided** by errstacks' `StructuredErrorTrace` /
  `StructuredFrame` — so there is **no separate `debug`-field toggle** to design; structured
  JSON of the trace is the debug representation, emitted where diagnostics are wanted (kept off
  the wire by default; the on-wire detail is separate, P4).

```rust
// The trace's context is ConnectError; `?` works through errstacks.
type ConnectResult<T> = Result<T, ErrorTrace<ConnectError>>;
// Domain errors flow in as custom contexts and change_context to ConnectError at the seam.
```

**Normative (decided — swept through all docs; scope precise per fresh-review A11):**
`ConnectResult<T>` is the error type of every public **RPC-surface** signature — handlers,
interceptor fn-types, the seam traits (`HandlerConn`/`ClientConn` and their halves), the
client surface, and generated traits/clients (Decisions 04/05/07/08/10/11 use it).
**Domain layers below the RPC surface keep their own typed errors** and feed in via
`From`/`change_context` at the boundary: `CodecError` (codecs, `ErrorWriter`),
`CompressionError` (compressors), `EnvelopeError` (wire framing, Decision 05),
`TransportError` (transports — defined with its `Code` mapping in Decision 11). Bare `Result<_, ConnectError>` appears nowhere in the public API;
`ConnectError → ErrorTrace<ConnectError>` has a `From` impl (same pattern as foundation_http's
`ServeError`) so constructors compose with `?`/`.into()`. Pure flow-control signals (bounded
pipe `Full`, `Pending`) are not errors and stay outside this type (Decision 11).

### ErrorWriter (Protocol-Aware Error Responses)

Mirrors connect-go's `ErrorWriter` — writes errors in the correct protocol format from middleware (before handler dispatch):

```rust
/// Per H17: holds only the single protobuf codec needed to encode `google.rpc.Status`
/// details for gRPC trailers (Connect errors are always JSON; gRPC-Web trailers are text;
/// there is no request-path codec registry — Decision 02). It does NOT own a `BufferPool`:
/// it is one shared instance across worker threads, so scratch comes from the current
/// worker's thread-local pool via the `with_worker_buffer(|buf| …)` accessor
/// (Decision 06 §Buffer Pool — owning a `&mut`-API pool here would force a Mutex).
pub struct ErrorWriter {
    status_codec: ProtoCodec,  // encodes google.rpc.Status for grpc-status-details-bin
}

impl ErrorWriter {
    pub fn new() -> Self;

    /// Can this writer handle errors for the given request?
    pub fn is_supported(&self, request: &SimpleIncomingRequest) -> bool;

    /// Write an error response in the appropriate protocol format. Takes the full trace
    /// (ConnectResult norm): code/message/details come from the ConnectError context;
    /// `ErrorDetail::from_errstacks` can ride the trace as a wire detail.
    pub fn write(
        &self,
        response: &mut SimpleOutgoingResponse,
        request: &SimpleIncomingRequest,
        error: &ErrorTrace<ConnectError>,
    ) -> Result<(), CodecError>;
}
```

## Consequences

- Error model is fully protocol-agnostic — one `ConnectError` type works across Connect, gRPC, gRPC-Web
- `Code` enum uses same numeric values as gRPC status codes — direct mapping
- Error details use protobuf `Any` semantics even when the RPC codec is JSON or Arrow
- `ErrorWriter` enables middleware (like auth) to write protocol-correct errors before handler dispatch
- `wire_error` flag lets clients distinguish server-sent errors from transport/client errors
- JSON error serialization matches the Connect protocol spec exactly
- **Error details always available (decided):** `google.protobuf.Any` is **reused from
  `buffa-types`** (existing WKT) and `google.rpc.Status` is **generated from its `.proto`** and
  bundled (Decision 05 §Decided Details — gRPC Status protobuf), so structured error details work **regardless of the service's
  codec** (JSON/Arrow-only services still emit protobuf-`Any` details, connect-go interop).
- **Errors are foundation_errstacks `ErrorTrace<C>` (decided):** `ConnectError` is the context
  type; domain errors are custom contexts mapped in via `From`/`change_context`. errstacks'
  structured JSON is the debug representation — no separate `debug`-field flag.

## Review-Gap Coverage

- **P2 / P3 — status mappings:** the `Code → HTTP` and `HTTP → Code` tables are already
  in this doc; implement as `Code::http_status()` / `Code::from_http_status()`. Decision
  05 references them rather than redefining.
- **P4 — detail wire encoding:** `ErrorDetail` derives serde `Serialize`/`Deserialize`;
  the wire form is `{type, value (base64 RawStdEncoding), debug?}`. Internal tracing uses
  foundation_errstacks and is independent of the on-wire detail.
- **P5 — EndStream details:** `WireError.details` is the same `Vec<WireErrorDetail>` used
  for unary errors; `EndStreamResponse.error` embeds it (documented in the JSON shapes
  above).
- **P15 — 304 Not Modified:** add `ConnectError::not_modified()` / `is_not_modified()`
  for conditional GET. Representation (fresh-review-2 #9): the 16-code enum has no 304
  member, so — connect-go parity — it carries `Code::Unknown` plus a **private
  `not_modified` sentinel flag** that `is_not_modified()` checks. It is a *signaling*
  error, GET-only: `ErrorWriter` renders it as HTTP **304 with headers (ETag) and no
  body**, never as a wire error payload; it is invalid outside conditional GET handling.
- **H11 — error wrapping:** add `wrap_if_uncoded`, `wrap_if_context`, `wrap_if_rst`,
  `wrap_if_h2c` helpers that map arbitrary errors to a `Code`.
- **H12 — `code_of`:** free fn `code_of(&dyn Error) -> Code` (downcast to `ConnectError`,
  else `Unknown`).
- **H13 — chain traversal:** document the downcast pattern — walk `Error::source()` and
  `downcast_ref::<ConnectError>()` — for interceptors checking nested errors.
- **H17 — `ErrorWriter`:** holds only the single protobuf codec for gRPC status details;
  scratch buffers are borrowed from the worker thread's pool via `with_worker_buffer`
  (Decision 06 RS6) — it owns no pool and no codec table.


## Cross-References

- Wire encodings: Decision 05. Auth error writing: Decision 09. Codec tables
  (`ProcedureCodecs` — there is no codec registry): Decision 02. `TransportError` → `Code`
  mapping: Decision 11.
