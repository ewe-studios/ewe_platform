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
}
```

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

### Error → foundation_errstacks Integration

`ConnectError` is the RPC-level error type. It integrates with foundation_errstacks for internal error tracing:

```rust
impl From<ErrorTrace<ConnectError>> for ConnectError {
    fn from(trace: ErrorTrace<ConnectError>) -> Self {
        // Extract the ConnectError from the trace
    }
}

// Handlers can use ? with ErrorTrace and it converts to ConnectError
```

### ErrorWriter (Protocol-Aware Error Responses)

Mirrors connect-go's `ErrorWriter` — writes errors in the correct protocol format from middleware (before handler dispatch):

```rust
pub struct ErrorWriter {
    codecs: Arc<CodecRegistry>,
    // Options for gRPC error encoding
}

impl ErrorWriter {
    pub fn new(codecs: Arc<CodecRegistry>) -> Self;

    /// Can this writer handle errors for the given request?
    pub fn is_supported(&self, request: &SimpleIncomingRequest) -> bool;

    /// Write an error response in the appropriate protocol format.
    pub fn write(
        &self,
        response: &mut SimpleOutgoingResponse,
        request: &SimpleIncomingRequest,
        error: &ConnectError,
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
  for conditional GET.
- **H11 — error wrapping:** add `wrap_if_uncoded`, `wrap_if_context`, `wrap_if_rst`,
  `wrap_if_h2c` helpers that map arbitrary errors to a `Code`.
- **H12 — `code_of`:** free fn `code_of(&dyn Error) -> Code` (downcast to `ConnectError`,
  else `Unknown`).
- **H13 — chain traversal:** document the downcast pattern — walk `Error::source()` and
  `downcast_ref::<ConnectError>()` — for interceptors checking nested errors.
- **H17 — `ErrorWriter`:** holds only a buffer pool + a single protobuf codec (for gRPC
  status details), not the full `CodecRegistry`.

## Open Questions

1. **Error detail without protobuf**: If a service uses only JSON or Arrow codec (no buffa dependency), error details still need protobuf `Any` for interoperability. Should we require buffa for error details, or support JSON-only error details as a platform extension?
2. **Debug field in error details**: connect-go optionally serializes error details to JSON under the `"debug"` key for readability. Should we always include debug JSON, never include it, or make it configurable?
3. **foundation_errstacks integration depth**: Should `ConnectError` wrap `ErrorTrace` internally for rich tracing, or keep them separate (simple error + separate trace in logs)?
