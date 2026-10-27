# Idea 2 and 3: FlatBuffers wire envelope with a payload union

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); Document Object Model (DOM);
WebAssembly (WASM); inter-process communication (IPC); JavaScript Object Notation (JSON);
Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE); hash-based message
authentication code (HMAC); AeroSpace and Defence Industries Association of Europe (ASD),
the owner of the STE standard ASD-STE100.

## 1. What the document proposes

Source: `foundation_ui_rearchitecture.md` §2, lines 23-136.

- One FlatBuffers table, `WireEnvelope`, wraps every server-to-client frame. It has
  `version`, `sequence_id` and a `payload` union.
- The `PayloadData` union selects the body: `Columnar`, `Arrow`, `MessagePack` or
  `MobileLayout`. The first three are opaque `[ubyte]` vectors. `MobileLayout` is a typed
  vector of `DomOpRow` (four operations: `CreateElement`, `SetText`, `SetAttribute`,
  `AppendChild`).
- One FlatBuffers table, `UserInteractionRequest`, carries every client-to-server action:
  `sequence_id`, `node_id`, `event_name`, `input_value` and `active_state`.
- `active_state` is an `InlineNodeState`: a `StateFormatPayload` union with eight formats
  (integer, float, text, JSON, MessagePack, Arrow, FlatBuffer with an optional `.bfbs`
  schema, custom binary) and a `security_hmac`.
- The stated goals are type safety and schema evolution that does not break installed
  clients.

## 2. What we have now

### 2.1 Outbound framing

| Item | Location |
|---|---|
| 6-byte header `[protocol:1][version:1][length:4 LE]` | `backends/foundation_ui_traits/src/envelope.rs:16-27`, parse at `:60`, write helper `encode_with_envelope` at `:94` |
| Protocol bytes: 0 custom binary, 1 Arrow/columnar, 2 JSON | `backends/foundation_ui_traits/src/encoder.rs:29-36` |
| Protocol byte 1 has two bodies, chosen by the version byte: version 1 is the compact columnar form, version 2 is real Arrow IPC | `backends/foundation_ui_traits/src/columnar_encoder.rs:8-11`, `backends/foundation_arrow/src/dom_ops.rs:54`, `:164` |
| 14-byte `WasmEnvelope` for the WASM-to-JS boundary (adds an arena memory id) | `backends/foundation_wasm/src/protocol.rs:96` |
| `ProtocolEncoder` trait that every body implements | `backends/foundation_ui_traits/src/encoder.rs:140` |
| HTTP Content-Type per protocol | `backends/foundation_ui_traits/src/platform_types.rs:313-346` |
| Server fan-out of encoded frames to live clients (SSE, WebSocket) with a replayed backlog | `backends/foundation_wasm_ui/src/server/mod.rs:38` (`FrameTransport`), `:71` (`Broadcaster`), `:151` (`BroadcastSink`) |

### 2.2 Inbound actions

- There is no typed inbound action envelope.
- Spec 39 decision 025 sketches a JSON action request (`action`, `target`, `payload`,
  `component_id`) at
  `specifications/completed/39-foundation-wasm-ui/decisions/025-server-client-state-sync.md:44-52`.
  No code implements it.
- Local events go from JS into WASM as JSON `EventData` through `invoke_signal_callback`
  (`backends/foundation_wasm_ui/src/events.rs:65`). This path stays inside one process. It
  does not reach a server.
- `<mount-data>` and `<mount-stream>` send plain `fetch` requests
  (`backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js:2353`, `:2374`). The request
  body is not typed.

### 2.3 FlatBuffers and MessagePack in the repository

- No crate uses FlatBuffers directly. Arrow IPC uses FlatBuffers internally for its message
  headers.
- Spec 36 decision 10 dropped a FlatBuffers fallback for agentic API records
  (`specifications/completed/36-agentic-api/decisions/10-serialization-format.md:116-118`).
- MessagePack (`rmp-serde`, `rmpv`) is used only by `foundation_keychain`
  (`backends/foundation_keychain/Cargo.toml:27-28`). The UI stack does not use it.

## 3. Gap

1. **No sequence id.** The 6-byte header has no frame order number. A client cannot detect a
   lost or reordered frame. Reconnect with a backlog
   (`foundation_wasm_ui/src/server/mod.rs`) works around this for one server, but not across
   replicas.
2. **No typed union.** The protocol byte plus the version byte select the body. Byte 1 means
   two different formats. This is fragile. A FlatBuffers union gives a typed tag and lets old
   clients skip unknown members.
3. **No inbound schema.** Actions have no typed request. This is the largest gap, because
   the signed state token (see [`stateless-signed-state.md`](stateless-signed-state.md))
   must ride in it.
4. **No native layout body.** All bodies carry `DomOp`, which is HTML-shaped. See
   [`native-mobile-rendering.md`](native-mobile-rendering.md).
5. **Schema evolution.** We only have a version byte. FlatBuffers gives field-level
   evolution (add fields at the end, deprecate fields, never reuse ids).

## 4. Open risks

1. **Alignment inside the envelope.** The columnar body needs an 8-byte aligned start for
   zero-copy typed arrays (`columnar_encoder.rs:26-32`). Arrow IPC needs 8-byte alignment too.
   A FlatBuffers `[ubyte]` vector is 4-byte aligned by default. We must confirm that the
   chosen FlatBuffers generator supports `force_align: 8` on vectors, or put the body after
   the FlatBuffers header as a trailing aligned block. If not, the JS parser falls back to a
   copy (`foundation-wasm-ui.js:129-150`). The fallback is correct, but the zero-copy gain is
   lost.
2. **Generator choice.** Rule 5 in `CLAUDE.md` says to use generated types. Options are
   `flatc` (a C++ binary in the build) or a pure Rust generator such as `planus`. A pure Rust
   generator is easier for our build. We must check `no_std` support, because
   `foundation_ui_traits` is `no_std` with zero dependencies (`foundation_ui_traits/src/lib.rs:1-5`).
   The envelope types may need their own crate so that `foundation_ui_traits` stays
   dependency-free.
3. **Double framing.** HTTP, SSE and WebSocket already frame messages. The FlatBuffers
   envelope adds a second frame. This is acceptable, but frame size limits must be
   configuration with documented defaults (rule 8). No hard-coded caps.
4. **Streaming.** A FlatBuffers table is read from one complete buffer. That is correct for
   one frame. Frames must stay small and bounded. A large data push must be a stream of
   frames, not one large frame (rule 4).
5. **The state format union is too wide.** Eight formats for `active_state` means eight
   decoders on the server, and a `.bfbs` schema shipped by the client. The client never reads
   the state (owner decision D5: the server signs it). The client only returns it. So the
   format is a server concern. On the wire, the state can be opaque bytes plus a token.

## 5. Conflict with earlier decisions

- Spec 36 decision 10 dropped FlatBuffers for agentic records. Per owner decision D3, the new
  direction overrides it for the UI wire. The two do not truly overlap: decision 10 is about
  stored AI records. A note in decision 10 that points to spec 69 is enough.
- The 6-byte `Envelope` and the protocol byte table are replaced, not extended.

## 6. Recommendation: **adapt**

- **Adopt** a FlatBuffers outer envelope for outbound frames, with `version`, `sequence_id`,
  a stream or component id, and a typed `payload` union.
- **Adopt** a FlatBuffers `ActionRequest` for inbound actions. Put the signed state in it as
  opaque bytes plus a token table (key id, expiry, tag). See
  [`stateless-signed-state.md`](stateless-signed-state.md).
- **Keep** the columnar and Arrow IPC bodies as union members. They work and they have
  tests. Give each its own union tag. Stop the "byte 1, version 1 or 2" overload.
- **Make the native layout body typed FlatBuffers tables**, not opaque bytes, because the
  native renderer reads it directly (native first, D4).
- **Drop MessagePack** for now. FlatBuffers tables cover typed data. Arrow covers tabular
  data. A third format adds a decoder on each platform for no current use case.
- **Drop the eight-way `StateFormatPayload` union.** The server picks one encoding for state
  and the client treats it as opaque.

See questions Q1, Q2 and Q3 in [`questions.md`](questions.md).
