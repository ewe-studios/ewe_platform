# Decision 02: Codec System & Serialization

## Context

ConnectRPC's wire protocol supports pluggable message serialization via content-type negotiation. The connect-go implementation defines:

```go
type Codec interface {
    Name() string                       // "proto", "json", etc.
    Marshal(any) ([]byte, error)        // serialize message → bytes
    Unmarshal([]byte, any) error        // deserialize bytes → message
}
```

With optional extensions:
- `marshalAppender` — append to existing buffer (`MarshalAppend`)
- `stableCodec` — deterministic serialization for HTTP GET caching (`MarshalStable`, `IsBinary`)

connect-go ships two codecs:
1. **Proto** (`protoBinaryCodec`) — binary protobuf via `proto.Marshal`/`proto.Unmarshal`
2. **JSON** (`protoJSONCodec`) — protobuf's canonical JSON mapping via `protojson`

Our plan requires three codecs:
1. **Proto** — via buffa (pure Rust, editions-first, zero-copy views)
2. **JSON** — via serde_json (standard Rust JSON)
3. **Arrow** — via foundation_arrow (Arrow IPC for columnar data)

## Decision

### Codec Trait Design

Define a `Codec` trait in foundation_connectrpc that mirrors connect-go's interface, adapted for Rust's type system:

```rust
pub trait Codec: Send + Sync + 'static {
    /// Wire name used in Content-Type headers.
    /// "proto" → application/proto, "json" → application/json, "arrow" → application/arrow
    fn name(&self) -> &str;

    /// Serialize a message to bytes.
    /// The message type is erased — implementations downcast internally.
    fn marshal(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError>;

    /// Deserialize bytes into a message.
    /// Returns a boxed message that the caller downcasts.
    fn unmarshal(&self, data: &[u8], target: &mut dyn MessageMut) -> Result<(), CodecError>;

    /// Zero-copy decode (S4): produce an **owning view** backed by `bytes` — e.g.
    /// `buffa::OwnedView<V>` (proto) or an Arc-backed `RecordBatch` (Arrow) — as a
    /// `'static + Send` boxed value the facade downcasts. Default `None` for codecs with no
    /// view representation (JSON), so owned-decode is used. Keeps zero-copy routed through
    /// the codec registry rather than hard-wired to one codec.
    fn unmarshal_owned_view(&self, bytes: Bytes) -> Result<Option<Box<dyn Any + Send>>, CodecError> {
        Ok(None)
    }
}

pub trait StableCodec: Codec {
    /// Deterministic serialization for HTTP GET caching.
    fn marshal_stable(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError>;

    /// Whether the serialized output is binary (true) or text (false).
    /// Binary codecs use base64 encoding in GET request query parameters.
    fn is_binary(&self) -> bool;
}
```

### Message Abstraction

**TODO**: Is this still needed?

connect-go uses Go's `any` interface for type erasure. We need a Rust equivalent that works across buffa messages, serde types, and Arrow types:

```rust
/// Read-only access to a message for serialization.
pub trait MessageRef: Send + Sync {
    /// Downcast to a concrete type for codec-specific serialization.
    fn as_any(&self) -> &dyn Any;
}

/// Mutable access to a message for deserialization.
pub trait MessageMut: Send {
    /// Downcast to a concrete type for codec-specific deserialization.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
```

This is the minimal type-erasure boundary. Each codec implementation knows what concrete types it expects and downcasts internally. This matches connect-go's pattern where `protoBinaryCodec.Marshal` casts `any` to `proto.Message`.

### Proto Codec (buffa)

**TODO**: Are we not using the OwnedView from Buffa?

```rust
pub struct ProtoCodec;

impl Codec for ProtoCodec {
    fn name(&self) -> &str { "proto" }

    fn marshal(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError> {
        // Downcast to buffa::Message, call compute_size + write_to
    }

    fn unmarshal(&self, data: &[u8], target: &mut dyn MessageMut) -> Result<(), CodecError> {
        // Downcast to buffa::Message, call merge_from
    }
}

impl StableCodec for ProtoCodec {
    fn marshal_stable(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError> {
        // Deterministic field ordering (buffa's default is already deterministic
        // for a given schema version, but we enforce it explicitly)
    }
    fn is_binary(&self) -> bool { true }
}
```

buffa dependency is behind a `proto` feature flag (default on). The buffa crate is `no_std + alloc` capable.

### JSON Codec (serde_json)

```rust
pub struct JsonCodec;

impl Codec for JsonCodec {
    fn name(&self) -> &str { "json" }

    fn marshal(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError> {
        // buffa messages → buffa's canonical protobuf-JSON (lowerCamelCase, string enums).
        // NOTE: there is no "is it serde::Serialize?" branch — `dyn Any` can only downcast
        // to a *concrete type*, not to a trait bound (S6). A non-protobuf serde-JSON
        // service is a separate codec instantiated over its concrete type and registered
        // under its OWN content-type (Q5), not smuggled through this proto-JSON codec.
    }

    fn unmarshal(&self, data: &[u8], target: &mut dyn MessageMut) -> Result<(), CodecError> {
        // buffa canonical JSON, DiscardUnknown = true (forward compatibility).
        // Reject zero-length payloads (P16).
    }
}

impl StableCodec for JsonCodec {
    fn marshal_stable(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError> {
        // Serialize then compact (strip whitespace), matching connect-go's approach
    }
    fn is_binary(&self) -> bool { false }
}
```

### Arrow Codec (foundation_arrow)

**Same, should we be returning Vec or a arrow view ?

```rust
pub struct ArrowCodec;

impl Codec for ArrowCodec {
    fn name(&self) -> &str { "arrow" }

    fn marshal(&self, message: &dyn MessageRef) -> Result<Vec<u8>, CodecError> {
        // Downcast to ArrowMessageBox, call encode_arrow()
        // Wire format: Arrow IPC bytes
    }

    fn unmarshal(&self, data: &[u8], target: &mut dyn MessageMut) -> Result<(), CodecError> {
        // Downcast to ArrowMessageBox, call decode_arrow()
    }
}
```

Arrow codec is behind an `arrow` feature flag. This is a platform extension — not part of the ConnectRPC spec. Content-Type: `application/arrow` for unary, `application/connect+arrow` for streaming.

**First-class zero-serialization codec (decided).** Arrow is the strongest zero-copy path:
`foundation_arrow` wraps the split `arrow-*` crates, and Arrow `Buffer`/`RecordBatch` are
**Arc-backed and columnar** — a batch is `'static + Send + Sync`, cheaply cloneable, and read
column-wise with **no per-field decode**. The Arrow "message" type is a `RecordBatch` (or a
`ToArrow`/`FromArrow` type), so it sidesteps the `MessageView<'a>` lifetime problem entirely.
Two caveats: (1) Arrow is **columnar** — a message is a batch of N rows (great for bulk /
analytical RPC, awkward for one-object-per-call; per-method choice), and (2) it is a
**platform extension** under `application/(connect+)arrow` — only our clients speak it, not
stock gRPC. **Enabler for *true* zero-copy:** wire `foundation_arrow`'s IPC reader to read
from the Arc-backed frame `Bytes` (`arrow_buffer::Buffer`) rather than `&[u8]`+copy, and let
write reuse the batch's buffers — otherwise the current `encode_ipc`/`decode_ipc(&[u8])`
path still does an IPC framing copy.

### Codec Registry

Mirrors connect-go's `readOnlyCodecs`:

```rust
pub struct CodecRegistry {
    codecs: HashMap<String, Arc<dyn Codec>>,
}

impl CodecRegistry {
    pub fn new() -> Self { /* proto + json by default */ }
    pub fn register(&mut self, codec: Arc<dyn Codec>) { /* add by name */ }
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Codec>> { /* lookup */ }
    pub fn protobuf(&self) -> &Arc<dyn Codec> { /* fallback to ProtoCodec */ }
    pub fn names(&self) -> Vec<&str> { /* registered codec names */ }
}
```

### Content-Type Mapping

| Codec Name | Unary Content-Type | Streaming Content-Type |
|---|---|---|
| `proto` | `application/proto` | `application/connect+proto` |
| `json` | `application/json` | `application/connect+json` |
| `arrow` | `application/arrow` | `application/connect+arrow` |
| `{custom}` | `application/{custom}` | `application/connect+{custom}` |

For gRPC/gRPC-Web protocols:
| Codec Name | gRPC Content-Type | gRPC-Web Content-Type |
|---|---|---|
| `proto` | `application/grpc+proto` or `application/grpc` | `application/grpc-web+proto` or `application/grpc-web` |
| `json` | `application/grpc+json` | `application/grpc-web+json` |

## Consequences

- **Three first-class codecs**: proto (buffa), json (serde_json), arrow (foundation_arrow)
- **Extensible**: Custom codecs register via `CodecRegistry::register`
- **Type erasure via `MessageRef`/`MessageMut`**: Each codec downcasts to its expected type — no universal message trait required
- **Feature-gated**: `proto` (default), `json` (default), `arrow` (optional)
- **Proto JSON uses protobuf canonical mapping**: lowerCamelCase field names, string enums, omitted zero values — matching connect-go's `protojson`

## Review-Gap Coverage

Decided items folded in from the review (we own the code; implement directly):

- **P1 — dual JSON registration:** register the JSON codec under both `json` and
  `json; charset=utf-8`, and canonicalize content-types (Decision 05 P6) so
  `application/json; charset=utf-8` is accepted rather than 415'd.
- **P16 — zero-length JSON:** reject zero-length JSON payloads with `invalid_argument`
  ("zero-length payload is not a valid JSON object").
- **RS7 — frozen registries:** `CodecRegistry` / `CompressionRegistry` are built then
  frozen (builder → `Arc`); no `&mut register` after handlers hold references.
- **RS8 — `MarshalAppend`:** add `marshal_append(&self, buf: &mut Vec<u8>, msg)` to
  `Codec` for pooled-buffer reuse on hot paths.
- **RS9 — Send/Sync:** per-call message construction resolves the `Send`-only vs
  `Send+Sync` distinction between `MessageRef`/`MessageMut`.
- **RS2 — zero-copy views (supported, via owning views):** a *naked* `buffa::MessageView<'a>`
  can't work under async handlers (not `'static`; can't be held across `.await`). The
  supported form is **`buffa::OwnedView<V>`** — a self-referential `Bytes`+view bundle that
  is **`'static + Send + Sync`** ("suitable for async and RPC frameworks", buffa DESIGN.md),
  so it survives `.await` and crosses the pool, with no per-field copy (only an `Arc`
  refcount on the frame `Bytes`). For the **Arrow** codec the message is an Arc-backed
  `RecordBatch`, inherently `'static`/`Send`/zero-copy. So zero-copy is a supported codegen
  variant (owned-decode is the default); it just isn't a borrowed `MessageView<'a>`.
- **Q5 — JSON semantics (decided):** protobuf messages serialize via canonical
  protobuf-JSON (lowerCamelCase, string enums, omit-zero). Arbitrary serde types are a
  documented **platform extension** that is *not* protobuf-JSON-canonical and is not
  guaranteed cross-language-interoperable. **Decision: ship it**, but every
  platform-extension codec registers under its **own** content-type / codec name (e.g.
  `arrow` → `application/arrow` + `application/connect+arrow`; a non-canonical serde-JSON
  extension under a distinct name) — never silently under `application/json` for protobuf
  services. The codec name on the wire is what selects canonical vs extension behaviour.

## Open Questions

1. **buffa JSON support**: Does buffa have built-in JSON serialization that follows the protobuf canonical JSON mapping? Or do we need to implement that ourselves? The connect-go implementation uses `protojson.Marshal`/`protojson.Unmarshal` from the official Go protobuf library. buffa has a `json` feature — need to verify it produces canonical protobuf JSON.
2. **Type erasure cost**: `dyn MessageRef` requires heap allocation and virtual dispatch. For high-throughput services, is this acceptable? Alternative: make `Codec` generic over message type, but this prevents storing mixed codecs in a registry. connect-go pays the same cost with `any` interface.
3. **Arrow batch semantics**: Arrow IPC naturally represents record batches (multiple rows). For unary RPCs, a single-row batch is sent. For streaming, each envelope could carry a multi-row batch. Should we define batch size policy, or leave it to the application?
4. **Zero-copy deserialization** — *resolved* (see RS2 above): supported via `buffa::OwnedView<V>` (`'static + Send + Sync`) for proto and Arc-backed `RecordBatch` for Arrow; owned-decode is the default. A naked borrowed `MessageView<'a>` is not usable under async handlers.
