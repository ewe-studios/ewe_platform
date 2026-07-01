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

> **No message type-erasure (reconciled with Decisions 10/11).** connect-go erases messages to
> `any` and downcasts to `proto.Message`; that relies on Go interface downcasting Rust does not
> have (`dyn Any` downcasts only to a *concrete* type, never to `&dyn buffa::Message` — the S6
> wall). And we don't need it: the framework's generic seam is **byte-level `Bytes`**
> (Decision 11), and typed encode/decode lives **only in the generated per-procedure facade**
> where `Req`/`Res` are concrete (Decision 10). So there is **no `MessageRef`/`MessageMut`, no
> `&dyn` message, no `as_any`** — codecs are used **monomorphically** over the concrete type.

```rust
// Encode/decode are monomorphic over the concrete message `M`, bounded by the codec's
// native message trait (`buffa::Message` for proto/proto-JSON; the Arrow message trait for
// Arrow). Used by codegen, never as a `dyn Codec` marshaling an erased message.
pub trait Codec: Send + Sync + 'static {
    // --- object-safe part (usable through `Arc<dyn Codec>` for negotiation/metadata) ---
    /// Wire name used in Content-Type headers ("proto"|"json"|"arrow").
    fn name(&self) -> &str;
    /// Binary (base64 in GET query) vs text encoding.
    fn is_binary(&self) -> bool;

    // --- typed part: monomorphic, called on the CONCRETE codec (not via `dyn Codec`) ---
    // `where Self: Sized` keeps the trait object-safe while these stay generic.
    /// Encode a concrete message to wire bytes.
    fn marshal<M: Message>(&self, message: &M) -> Result<Bytes, CodecError> where Self: Sized;
    /// Decode wire bytes into a concrete owned message.
    fn unmarshal<M: Message + Default>(&self, data: Bytes) -> Result<M, CodecError> where Self: Sized;
    /// Zero-copy decode (S4/RS2): an owning view over `bytes` — `buffa::OwnedView<M>` (proto)
    /// or Arc-backed `RecordBatch` (Arrow). `M::View = M` for codecs with no view form (JSON).
    fn unmarshal_owned_view<M: Message>(&self, bytes: Bytes) -> Result<M::View, CodecError> where Self: Sized;
    /// Deterministic serialization for HTTP GET caching (stable field ordering).
    fn marshal_stable<M: Message>(&self, message: &M) -> Result<Bytes, CodecError> where Self: Sized;
}
```

`Message` here is **not** a type-erasure trait — it's just the concrete-message bound the codec
family implements against (`buffa::Message`, the Arrow message trait, etc.); it is used only as
a generic bound (`M: Message`), never as `&dyn Message`.

**Two-layer use of `Codec` (this is why `Arc<dyn Codec>` remains valid everywhere):**
- **Negotiation / metadata** goes through `Arc<dyn Codec>` (`name`/`is_binary`/content-type) —
  the registry and configs hold codecs this way. Object-safe because the typed methods are
  `where Self: Sized` (excluded from the vtable).
- **Typed encode/decode** is called on the **concrete** codec (`ProtoCodec`/`JsonCodec`/
  `ArrowCodec`), monomorphic over `Req`/`Res`. The generated facade resolves the negotiated
  codec to its concrete type (match on the codec's identity) and calls `marshal::<Res>` /
  `unmarshal::<Req>` directly.

So content-type negotiation stays runtime (via `dyn Codec`), while message
serialization is fully typed — **no `dyn Any` message ever exists**, and the `Arc<dyn Codec>`
fields in Decisions 05/07/08/11 are the negotiation handle, not a typed-marshal path.

### Proto Codec (buffa)

Yes — the ProtoCodec is where `buffa::OwnedView` is produced. `unmarshal_owned_view` (S4)
returns a `buffa::OwnedView<V>` backed by the request `Bytes`, giving the zero-copy path
(RS2); plain `unmarshal` remains for owned-decode callers. Both are implemented:

```rust
pub struct ProtoCodec;

impl Codec for ProtoCodec {
    fn name(&self) -> &str { "proto" }
    fn is_binary(&self) -> bool { true }

    fn marshal<M: buffa::Message>(&self, m: &M) -> Result<Bytes, CodecError> {
        // m.compute_size() + write_to — concrete, no downcast.
    }
    fn unmarshal<M: buffa::Message + Default>(&self, data: Bytes) -> Result<M, CodecError> {
        // M::parse_from(data)
    }
    fn unmarshal_owned_view<M: buffa::Message>(&self, bytes: Bytes) -> Result<M::View, CodecError> {
        // buffa::view::OwnedView::<M::View>::decode(bytes) — self-referential Bytes+view,
        // 'static + Send + Sync, Deref<Target = View>, no copy (RS2 / S4). The type parameter
        // is the *view* type (e.g. `PersonView`), and `M::View = OwnedView<PersonView>`.
    }
    fn marshal_stable<M: buffa::Message>(&self, m: &M) -> Result<Bytes, CodecError> {
        // Deterministic field ordering (buffa is deterministic for a schema version;
        // we enforce it explicitly for GET caching).
    }
}
```

(`Message` for the proto codec family = `buffa::Message`.)

buffa dependency is behind a `proto` feature flag (default on). The buffa crate is `no_std + alloc` capable.

> **Reference source:** buffa lives at
> `/home/darkvoid/Boxxed/@formulas/src.rust/src.RPC/src.connect-protocol/buffa/` (workspace:
> `buffa`, `buffa-codegen`, `buffa-descriptor`, `buffa-types`, `protoc-gen-buffa`,
> `conformance`). It is a mature pure-Rust, editions-first protobuf impl with canonical JSON
> (`json` feature, conformance-tested), zero-copy `OwnedView<V>` (`view` feature), and text
> format — the runtime + codegen basis for Decisions 02/05/10.

### JSON Codec (serde_json)

```rust
pub struct JsonCodec;

impl Codec for JsonCodec {
    fn name(&self) -> &str { "json" }
    fn is_binary(&self) -> bool { false }

    fn marshal<M: buffa::Message>(&self, m: &M) -> Result<Bytes, CodecError> {
        // Concrete buffa message → canonical protobuf-JSON (lowerCamelCase, string enums).
        // Monomorphic over M — no `dyn Any`, no downcast, so the S6 "downcast to Serialize"
        // problem never arises. A non-protobuf serde-JSON service is a *separate* codec over
        // its own concrete type under its OWN content-type (Q5).
    }
    fn unmarshal<M: buffa::Message + Default>(&self, data: Bytes) -> Result<M, CodecError> {
        // canonical JSON, DiscardUnknown = true; reject zero-length payloads (P16).
    }
    fn unmarshal_owned_view<M: buffa::Message>(&self, data: Bytes) -> Result<M::View, CodecError> {
        // JSON has no zero-copy view: M::View = M, so this just calls unmarshal.
    }
    fn marshal_stable<M: buffa::Message>(&self, m: &M) -> Result<Bytes, CodecError> {
        // Serialize then compact (strip whitespace), matching connect-go.
    }
}
```

### Arrow Codec (foundation_arrow)

Resolved: the Arrow codec returns a **view**, not a `Vec`, for decode. `unmarshal_owned_view`
yields an Arc-backed `RecordBatch` (its buffers are already `Arc`-shared, so this is
zero-copy over the request `Bytes`); `marshal` still produces bytes for the wire. This is the
same S4 owned-view path the ProtoCodec uses, routed through the codec registry.

```rust
pub struct ArrowCodec;

impl Codec for ArrowCodec {
    fn name(&self) -> &str { "arrow" }
    fn is_binary(&self) -> bool { true }

    fn marshal<M: ToArrow>(&self, m: &M) -> Result<Bytes, CodecError> {
        // m.encode_arrow() → Arrow IPC bytes (concrete `M: ToArrow`, no downcast).
    }
    fn unmarshal<M: FromArrow + Default>(&self, data: Bytes) -> Result<M, CodecError> {
        // M::decode_arrow(data)
    }
    fn unmarshal_owned_view<M: FromArrow>(&self, data: Bytes) -> Result<M::View, CodecError> {
        // Arc-backed RecordBatch over `data` — already 'static/Send, zero-copy.
    }
    fn marshal_stable<M: ToArrow>(&self, m: &M) -> Result<Bytes, CodecError> { /* deterministic IPC */ }
}
```
(For Arrow, `Message` = `ToArrow`/`FromArrow`; the message type is a `RecordBatch`-backed type.)

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
- **No message type-erasure**: codecs are monomorphic over the concrete message (bounded by
  the codec's native trait — `buffa::Message`, `ToArrow`/`FromArrow`); the framework's generic
  seam is byte-level `Bytes` (Decision 11) and typing lives in the generated facade (Decision
  10). No `MessageRef`/`MessageMut`/`dyn Any` messages.
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
- **RS8 — `MarshalAppend`:** add `marshal_append<M: Message>(&self, buf: &mut Vec<u8>, m: &M)`
  to `Codec` for pooled-buffer reuse on hot paths.
- **RS9 — Send/Sync:** moot — with monomorphic codecs there is no erased `MessageRef`/
  `MessageMut` pair, so no `Send`-only vs `Send+Sync` split to reconcile; the concrete `Req`/
  `Res` carry their own auto-trait bounds.
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

1. **buffa JSON support — resolved (verified).** buffa's `json` feature emits **canonical
   protobuf-JSON** (verified in source: `buffa/Cargo.toml` `json` feature + `DESIGN.md`
   §454–468 + the `conformance/` crate): snake_case→camelCase, `int64`/`uint64`/`sint64` as
   strings, proto3 default-value omission, and bespoke well-known-type JSON (Timestamp→RFC3339,
   Duration→`"1.5s"`, `Any`→`{"@type":…}`, wrappers, Value/Struct). So `JsonCodec` uses buffa's
   `json` **directly** — we do not hand-roll the canonical mapping.
2. **Arrow batch semantics — decided: leave to the application.** Unary = a single-row batch;
   for streaming the handler/client choose rows-per-batch and the framework carries each
   `RecordBatch` through as **one enveloped message, unchanged** — this preserves Arrow's
   bulk/columnar strength (no framework-imposed batch size). DoS safety still comes from
   `read_max_bytes` (Decision 06) bounding total decoded size, not from a row cap.

<!-- Resolved and removed:
 • Type-erasure cost — MOOT and now eliminated: codecs are monomorphic over concrete Req/Res
   (facade owns the concrete codec, Decision 10/11); no dyn Any messages, no MessageRef/MessageMut.
 • Zero-copy deserialization — resolved via buffa::OwnedView<V> / Arc-backed RecordBatch (RS2). -->

