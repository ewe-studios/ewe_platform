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
// The message type lives ON THE TRAIT (`CodecFor<M>`), not on generic methods — one Rust
// trait cannot declare `marshal<M: Message>` while impls narrow `M` to `buffa::Message` /
// `ToArrow`, and there is no unified `Message` trait. Split by role:

/// Object-safe metadata supertrait. `name()` is the WIRE TOKEN carried in Content-Type
/// (`application/{name}`, `application/connect+{name}`, GET `?encoding={name}`) — not an
/// internal registry key; it is reached through the table's `CodecFor` entries (supertrait),
/// and there is NO free-standing `Arc<dyn Codec>` registry handle (see ProcedureCodecs).
pub trait Codec: Send + Sync + 'static {
    /// Wire name used in Content-Type headers ("proto"|"json"|"arrow").
    fn name(&self) -> &str;
    /// Binary (base64 in GET query) vs text encoding.
    fn is_binary(&self) -> bool;
}

/// Typed encode/decode for ONE message type `M`. Because the generic is on the trait,
/// each codec blanket-implements it over its family's native message trait (bounds live on
/// the impl block, never the trait), and `dyn CodecFor<M>` is object-safe — the typed facade
/// holds `Arc<dyn CodecFor<Req>>` / `Arc<dyn CodecFor<Res>>` selected by runtime negotiation.
/// No `dyn Any` message ever exists.
pub trait CodecFor<M>: Codec {
    /// Encode a concrete message to wire bytes.
    fn marshal(&self, message: &M) -> Result<Bytes, CodecError>;
    /// Decode wire bytes into a concrete owned message.
    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError>;
    /// Deterministic serialization for HTTP GET caching (stable field ordering).
    fn marshal_stable(&self, message: &M) -> Result<Bytes, CodecError>;
    /// Append-encode into a pooled buffer (RS8) for hot paths.
    fn marshal_append(&self, buf: &mut Vec<u8>, message: &M) -> Result<(), CodecError>;
}

// Blanket impls per codec family — the family bound sits on the impl block:
impl<M: buffa::Message + Default> CodecFor<M> for ProtoCodec { /* … */ }
impl<M: buffa::Message + Default> CodecFor<M> for JsonCodec  { /* canonical proto-JSON */ }
impl<M: ToArrow + FromArrow>      CodecFor<M> for ArrowCodec { /* … */ }
```

> **Why not `trait Codec { type Message: … }`?** (considered, rejected)
> (a) One impl = one `Message` type, but a codec encodes unboundedly many message types — the
> struct would have to go generic (`ProtoCodec<M>`) with per-type instances constructed from
> config for every procedure. (b) A bound on the associated type written in the *trait* would
> apply to every codec family at once (proto messages aren't `ToArrow`). (c) `dyn Codec` with
> an unnamed associated type is not a valid type, so the metadata/typed split is needed
> anyway — and `dyn Codec<Message = M>` is exactly `dyn CodecFor<M>` with more ceremony.
> `CodecFor<M>` keeps one configured codec value serving all message types
> (`Arc<TheCodec>` coerces to `Arc<dyn CodecFor<Req>>` per procedure).

**Zero-copy decode is NOT on `dyn CodecFor<M>`.** `unmarshal_owned_view`'s return type differs
per family (`buffa::OwnedView<V>` / Arc-backed `RecordBatch` / owned `M`), so it stays an
**inherent method on each concrete codec**; the zero-copy handler variant bakes the view type
into its signature (Decision 10), so the generated facade dispatches view decode statically
against the concrete codec. (S4/RS2 unchanged in substance.)

**Single authority (decided): the handler-owned `ProcedureCodecs` table.** The codec is
chosen by the **client, per request, on the wire** (Content-Type) — that is the
ConnectRPC/gRPC contract, so a procedure owns a *set* of codecs keyed by the wire token,
not exactly one. There is **no request-time codec registry**: extracting the name from a
content-type is mechanical string parsing (the `application/(connect+){name}` /
`application/grpc(-web)+{name}` grammar is fixed — Decision 05), and "is this codec
supported *here*" is answered by the procedure's own table. Metadata (`name`/`is_binary`)
rides the `Codec` supertrait of each table entry; typed encode/decode goes through
`Arc<dyn CodecFor<M>>` — **no `dyn Any` message ever exists**, and no name-matching sits on
hot paths (the typed pair is resolved once at dispatch).

### Proto Codec (buffa)

Yes — the ProtoCodec is where `buffa::OwnedView` is produced. The **inherent**
`unmarshal_owned_view` (S4) returns a `buffa::OwnedView<V>` backed by the request `Bytes`,
giving the zero-copy path (RS2); the `CodecFor` blanket impl covers owned decode:

```rust
pub struct ProtoCodec;

impl Codec for ProtoCodec {
    fn name(&self) -> &str { "proto" }
    fn is_binary(&self) -> bool { true }
}

impl<M: buffa::Message + Default> CodecFor<M> for ProtoCodec {
    fn marshal(&self, m: &M) -> Result<Bytes, CodecError> {
        // m.compute_size() + write_to — concrete, no downcast.
    }
    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError> {
        // M::parse_from(data)
    }
    fn marshal_stable(&self, m: &M) -> Result<Bytes, CodecError> {
        // Deterministic field ordering (buffa is deterministic for a schema version;
        // we enforce it explicitly for GET caching).
    }
    fn marshal_append(&self, buf: &mut Vec<u8>, m: &M) -> Result<(), CodecError> {
        // write_to into the pooled buffer (RS8).
    }
}

impl ProtoCodec {
    /// INHERENT (per-family return type — deliberately not on `dyn CodecFor`, see trait
    /// section): zero-copy owning view over the frame bytes.
    pub fn unmarshal_owned_view<V: buffa::MessageView>(&self, bytes: Bytes)
        -> Result<buffa::OwnedView<V>, CodecError> {
        // buffa::view::OwnedView::<V>::decode(bytes) — self-referential Bytes+view,
        // 'static + Send + Sync, Deref<Target = V>, no copy (RS2 / S4).
    }
}
```

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
}

impl<M: buffa::Message + Default> CodecFor<M> for JsonCodec {
    fn marshal(&self, m: &M) -> Result<Bytes, CodecError> {
        // Concrete buffa message → canonical protobuf-JSON (lowerCamelCase, string enums).
        // Monomorphic over M — no `dyn Any`, no downcast, so the S6 "downcast to Serialize"
        // problem never arises. A non-protobuf serde-JSON service is a *separate* codec over
        // its own concrete type under its OWN content-type (Q5).
    }
    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError> {
        // canonical JSON, DiscardUnknown = true; reject zero-length payloads (P16).
    }
    fn marshal_stable(&self, m: &M) -> Result<Bytes, CodecError> {
        // Serialize then compact (strip whitespace), matching connect-go.
    }
    fn marshal_append(&self, buf: &mut Vec<u8>, m: &M) -> Result<(), CodecError> { /* RS8 */ }
}

// JSON has no zero-copy view form — owned decode is its only path (no inherent view method).
```

### Arrow Codec (foundation_arrow)

Resolved: the Arrow codec returns a **view**, not a `Vec`, for zero-copy decode. The inherent
`unmarshal_batch` yields an Arc-backed `RecordBatch` (its buffers are already `Arc`-shared, so
this is zero-copy over the request `Bytes`); `marshal` still produces bytes for the wire. This
is the same S4 owned-view path the ProtoCodec uses.

```rust
pub struct ArrowCodec;

impl Codec for ArrowCodec {
    fn name(&self) -> &str { "arrow" }
    fn is_binary(&self) -> bool { true }
}

impl<M: ToArrow + FromArrow> CodecFor<M> for ArrowCodec {
    fn marshal(&self, m: &M) -> Result<Bytes, CodecError> {
        // m.encode_arrow() → Arrow IPC bytes (concrete `M: ToArrow`, no downcast).
    }
    fn unmarshal(&self, data: Bytes) -> Result<M, CodecError> {
        // M::decode_arrow(data)
    }
    fn marshal_stable(&self, m: &M) -> Result<Bytes, CodecError> { /* deterministic IPC */ }
    fn marshal_append(&self, buf: &mut Vec<u8>, m: &M) -> Result<(), CodecError> { /* RS8 */ }
}

impl ArrowCodec {
    /// INHERENT zero-copy path (per-family return type, not on `dyn CodecFor`): Arc-backed
    /// `RecordBatch` over `data` — already 'static/Send, zero-copy.
    pub fn unmarshal_batch(&self, data: Bytes) -> Result<RecordBatch, CodecError> { /* … */ }
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

### ProcedureCodecs — the single codec authority (normative; replaces the registry)

> connect-go's `readOnlyCodecs` registry is **not ported**. Its two request-time jobs —
> name extraction and supported-lookup — are string mechanics plus this handler-owned table.

```rust
/// Per-procedure, built at registration where `Req`/`Res` are concrete, then FROZEN —
/// nothing installs codecs after registration. Owned by the HandlerEntry (server) /
/// Client (client). Map: wire name → (Arc<dyn CodecFor<Req>>, Arc<dyn CodecFor<Res>>).
pub struct ProcedureCodecs<Req, Res> { /* … */ }

impl<Req, Res> ProcedureCodecs<Req, Res> {
    /// Supply several codecs AT ONCE (decided — resolves the "list of codecs" question).
    /// Deliberately NOT `Vec<Arc<dyn Codec>>`: a dyn list cannot yield the typed
    /// `CodecFor<Req>`/`CodecFor<Res>` entries — that is the exact type-erasure wall the
    /// deleted registry hit. Instead `CodecSet` is implemented for TUPLES of concrete
    /// codecs (macro-generated up to N elements), each element independently bound
    /// `CodecFor<Req> + CodecFor<Res>`, so a heterogeneous list stays fully typed:
    ///     ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec))
    /// `defaults()` ≡ `of((ProtoCodec, JsonCodec))`; `only(c)` ≡ `of((c,))` — both are
    /// thin wrappers over this one constructor.
    pub fn of(set: impl CodecSet<Req, Res>) -> Self;
    /// The interop default (what codegen emits): proto + json, per the Connect spec —
    /// callable by connect-go / connect-es / curl / browsers. METHOD-LEVEL BOUNDS
    /// (fresh-review-2 #7): building the entries coerces `Arc<ProtoCodec>`/`Arc<JsonCodec>`
    /// into `Arc<dyn CodecFor<…>>`, which requires the proto-family blanket impls — an
    /// Arrow-only `ToArrow`/`FromArrow` type CANNOT use `defaults()` (it can't serve the
    /// proto+json conformance it would claim) and uses `only(ArrowCodec)` instead.
    pub fn defaults() -> Self
    where
        Req: buffa::Message + Default,
        Res: buffa::Message + Default;
    /// Single-codec endpoint: exactly one entry; every other content-type → 415.
    /// NOTE: a proto-less/json-less procedure is NOT Connect-conformant — intended for
    /// our-stack extension endpoints (e.g. arrow bulk feeds), never the codegen default.
    pub fn only<C>(codec: C) -> Self
        where C: CodecFor<Req> + CodecFor<Res>;
    /// Add a custom codec — a concrete VALUE with a generic bound (never a name string or
    /// `Arc<dyn Codec>` handle); the wire name comes from `Codec::name()`.
    pub fn with_codec<C>(self, codec: C) -> Self
        where C: CodecFor<Req> + CodecFor<Res>;
    /// Request-time resolution (O(1); the only lookup that exists — a miss is a 415 on the
    /// server / a constructor error on the client, never a silent fallback).
    pub fn for_request(&self, name: &str)  -> ConnectResult<Arc<dyn CodecFor<Req>>>;
    pub fn for_response(&self, name: &str) -> ConnectResult<Arc<dyn CodecFor<Res>>>;
    pub fn names(&self) -> impl Iterator<Item = &str>;   // Accept-Post construction
}
```

**Full flow (construction → request):**

```
registration (codegen; Req/Res concrete)
  ProcedureCodecs::<Req,Res>::defaults()          ← proto + json (interop default)
      [.with_codec(MyCodec)]                      ← via generated *_with_codec entry points
      [ProcedureCodecs::only(ArrowCodec)]         ← single-codec extension endpoints
      [ProcedureCodecs::of((Proto, Json, Arrow))] ← several at once (typed tuple CodecSet)
        │ frozen; owned by HandlerEntry (server) / Client (client)
        ▼
request time (server)
  Content-Type ──canonicalize (Decision 05 P6)──► codec NAME     ← string mechanics, no registry
        ──► THIS procedure's table.for_request(name) / for_response(name)
              ├─ hit  ─► typed pair ─► facade encode/decode
              └─ miss ─► 415 (+ Accept-Post from table.names())       ← defined; no silent fallback
client selection
  ClientOptions::with_codec(name) picks the SEND codec among the client's installed entries
  (it becomes the emitted Content-Type; the response arrives in the same codec per spec);
  unknown name → error at Client::new. Custom codecs install via <Svc>Client::new_with_codec.
```

Registration reads as directly as it sounds — the wrapping handler construct owns exactly
the codecs it supports, and nothing else exists:

```rust
router.register("/v1/analytics.Feed/Batches", ProcedureCodecs::only(ArrowCodec), handler);
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
- **Extensible**: custom codecs enter as concrete values via `ProcedureCodecs::with_codec` /
  `::only` (generated `*_with_codec` entry points hide the plumbing); there is **no
  request-time codec registry**
- **No message type-erasure**: typed dispatch is `dyn CodecFor<M>` (message type on the
  trait; blanket impls per family over `buffa::Message` / `ToArrow`+`FromArrow`); the
  framework's generic seam is byte-level `Bytes` (Decision 11) and the typed handles live in
  the per-procedure `ProcedureCodecs` table built by generated code (Decision 10). No
  `MessageRef`/`MessageMut`/`dyn Any` messages, no unified `Message` trait.
- **Feature-gated**: `proto` (default), `json` (default), `arrow` (optional)
- **Proto JSON uses protobuf canonical mapping**: lowerCamelCase field names, string enums, omitted zero values — matching connect-go's `protojson`

## Decided Details

- **P1 — JSON charset acceptance:** `application/json; charset=utf-8` is accepted rather
  than 415'd via content-type canonicalization (Decision 05 P6): the `charset` parameter is
  stripped **before** the name is resolved in the procedure's table, which keys on bare wire
  names — no dual registration (that was a registry-era workaround; the registry is gone).
- **P16 — zero-length JSON:** reject zero-length JSON payloads with `invalid_argument`
  ("zero-length payload is not a valid JSON object").
- **RS7 — frozen registries:** `ProcedureCodecs` / `CompressionRegistry` are built then
  frozen (builder → `Arc`); no `&mut register` after handlers hold references. (The former
  global `CodecRegistry` is deleted — see §ProcedureCodecs.)
- **RS8 — `MarshalAppend`:** `marshal_append(&self, buf: &mut Vec<u8>, m: &M)` is on
  `CodecFor<M>` (see trait section) for pooled-buffer reuse on hot paths.
- **RS2 — zero-copy views (supported, via owning views):** a *naked* `buffa::MessageView<'a>`
  can't work under async handlers (not `'static`; can't be held across `.await`). The
  supported form is **`buffa::OwnedView<V>`** — a self-referential `Bytes`+view bundle that
  is **`'static + Send + Sync`** ("suitable for async and RPC frameworks", buffa DESIGN.md),
  so it survives `.await` and crosses the pool, with no per-field copy (only an `Arc`
  refcount on the frame `Bytes`). For the **Arrow** codec the message is an Arc-backed
  `RecordBatch`, inherently `'static`/`Send`/zero-copy. So zero-copy is a supported codegen
  variant (owned-decode is the default); it just isn't a borrowed `MessageView<'a>`.
  **View-typed procedures are single-codec by construction** (fresh-review A5):
  `OwnedView<V>`/`RecordBatch` satisfy no `CodecFor` family bound, so no generic
  `ProcedureCodecs` table exists for them — codegen emits the zero-copy variant with one
  fixed concrete codec (proto → `OwnedView`, arrow → `RecordBatch`), statically dispatched
  via the inherent view method; any other content-type → 415. Same non-conformance caveat
  class as `only(...)` endpoints.
- **Q5 — JSON semantics (decided):** protobuf messages serialize via canonical
  protobuf-JSON (lowerCamelCase, string enums, omit-zero). Arbitrary serde types are a
  documented **platform extension** that is *not* protobuf-JSON-canonical and is not
  guaranteed cross-language-interoperable. **Decision: ship it**, but every
  platform-extension codec registers under its **own** content-type / codec name (e.g.
  `arrow` → `application/arrow` + `application/connect+arrow`; a non-canonical serde-JSON
  extension under a distinct name) — never silently under `application/json` for protobuf
  services. The codec name on the wire is what selects canonical vs extension behaviour.

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
   `read_max_bytes` (Decision 06) bounding total decoded size, not from a row cap — noting
   it is opt-in protection (the default is connect-go-parity unlimited, Decision 06 P17). The size
   caps also govern the WS batch framing: per-envelope `read_max_bytes`/`send_max_bytes`
   apply unchanged inside a batch, and the assembler's `max_message_size` bounds the whole
   WS message (rule recorded in Decision 13 §Framing).
