# Idea 2 and 3: FlatBuffers as the top format: one schema, every body a generated type

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
- Spec 36 decision 10 (`specifications/completed/36-agentic-api/decisions/10-serialization-format.md`)
  is about a different thing: how `foundation_ai` stores its conversation data (messages, tool
  calls, memory snapshots, events). It says that data is always written as JSON and as Arrow.
  It once kept FlatBuffers as a fallback in case Arrow did not build on WebAssembly; the
  fallback was dropped on 2026-06-15 (lines 116-118) because Arrow builds there. It says
  nothing about the UI wire. It is the only written position on FlatBuffers in the repository.
- MessagePack (`rmp-serde`, `rmpv`) is used only by `foundation_keychain`
  (`backends/foundation_keychain/Cargo.toml:27-28`). The UI stack does not use it.

## 3. Gap

Owner direction (PR #157 review): **FlatBuffers is the top format. Everything else, columnar
included, is a type in the FlatBuffers schema, and the schema is code-generated for every
language we support.** Today it is the other way round: each body is a hand-written byte
layout with its own encoder in Rust and its own parser in JavaScript.

1. **No schema.** The columnar layout (`columnar_encoder.rs`), the JSON body and the Arrow
   `DomOp` encoder are hand-written. Each new language needs a new hand-written parser. The
   JavaScript `ColumnarParser` is one; Swift and Kotlin would need two more.
2. **No sequence id.** The 6-byte header has no frame order number. A client cannot detect a
   lost or reordered frame across replicas.
3. **No typed body tag.** The protocol byte plus the version byte select the body. Byte 1
   means two different formats. A FlatBuffers union gives a typed tag, and old clients can
   skip members they do not know.
4. **No inbound schema.** Actions have no typed request. The signed state token
   ([`stateless-signed-state.md`](stateless-signed-state.md)) must ride in one.
5. **No native layout body.** All bodies carry the HTML-shaped `DomOp`. See
   [`native-mobile-rendering.md`](native-mobile-rendering.md).
6. **Schema evolution.** We only have a version byte. FlatBuffers gives field-level evolution
   (add fields at the end, deprecate fields, never reuse ids).

## 4. Open risks

1. **Alignment inside the envelope.** The columnar body needs an 8-byte aligned start for
   zero-copy typed arrays (`columnar_encoder.rs:26-32`). Arrow IPC needs 8-byte alignment too.
   A FlatBuffers `[ubyte]` vector is 4-byte aligned by default. We must confirm that the
   chosen FlatBuffers generator supports `force_align: 8` on vectors, or put the body after
   the FlatBuffers header as a trailing aligned block. If not, the JS parser falls back to a
   copy (`foundation-wasm-ui.js:129-150`). The fallback is correct, but the zero-copy gain is
   lost.
2. **Generator choice.** The schema must generate code for every language we ship: Rust,
   TypeScript (web), Swift (iOS) and Kotlin (Android). `flatc`, the reference compiler,
   generates all four from one `.fbs` file. A pure Rust generator such as `planus` generates
   only Rust, so it cannot be the single source. So the generator is `flatc`, pinned to one
   version, run by a build step that writes the generated code for each language. We must
   check `no_std` support of the generated Rust code, because `foundation_ui_traits` is
   `no_std` with zero dependencies (`foundation_ui_traits/src/lib.rs:1-5`). The schema and
   the generated Rust types get their own crate, so `foundation_ui_traits` stays
   dependency-free.
3. **Double framing.** HTTP, SSE and WebSocket already frame messages. The FlatBuffers
   envelope adds a second frame. This is acceptable, but frame size limits must be
   configuration with documented defaults (rule 8). No hard-coded caps.
4. **Streaming.** A FlatBuffers table is read from one complete buffer. That is correct for
   one frame. Frames must stay small and bounded. A large data push must be a stream of
   frames, not one large frame (rule 4).
5. **Arrow IPC.** Arrow is its own format: FlatBuffers message headers plus raw column
   buffers. It is not a FlatBuffers table, so it cannot be "a type in our schema" without
   wrapping its bytes. Its value is tool interoperability (DuckDB, Polars, DataFusion). For
   UI frames, a columnar FlatBuffers table gives the same zero-copy columns with generated
   readers. See Q20 in [`questions.md`](questions.md).
6. **The state format union is too wide.** Eight formats for `active_state` means eight
   decoders on the server, and a `.bfbs` schema shipped by the client. The client never reads
   the state (owner decision D5: the server signs it). The client only returns it. So the
   format is a server concern. On the wire, the state can be opaque bytes plus a token.

## 5. Conflict with earlier decisions

- Spec 36 decision 10 is about stored `foundation_ai` conversation data (JSON plus Arrow), not
  the UI wire. It dropped FlatBuffers only as a fallback for Arrow on WebAssembly. Per owner
  decision D3 and the PR #157 review, FlatBuffers is now the top format for the UI wire. If
  the owner wants the same rule for the stored conversation data too, decision 10 must be
  rewritten, not only annotated. See Q19 in [`questions.md`](questions.md).
- The 6-byte `Envelope`, the protocol byte table and the hand-written columnar layout are
  replaced, not extended.

## 6. Recommendation: **adopt FlatBuffers as the single schema**

- **One schema** (`.fbs` files) is the source of truth for every frame and every body. `flatc`
  generates Rust, TypeScript, Swift and Kotlin from it (rule 5: use generated types).
- **Outbound frame:** a FlatBuffers `Frame` table with `version`, `sequence_id`, a stream or
  component id, and a typed `body` union.
- **Inbound action:** a FlatBuffers `ActionRequest` table. The signed state rides in it as
  opaque bytes plus a token table (key id, expiry, tag). See
  [`stateless-signed-state.md`](stateless-signed-state.md).
- **Columnar becomes a schema type.** The columnar `DomOp` batch is a FlatBuffers table of
  typed vectors (operation codes, node ids, string offsets, string bytes), with
  `force_align: 8` on the numeric vectors. FlatBuffers vectors are contiguous little-endian
  arrays, so the zero-copy typed-array view stays: the generated TypeScript reader returns a
  `Uint32Array` view over the buffer for a `[uint32]` vector. The hand-written
  `ColumnarBatch` encoder and the JavaScript `ColumnarParser` are retired once the generated
  form matches them in a benchmark. See [`columnar-zero-copy.md`](columnar-zero-copy.md).
- **Native layout body:** typed FlatBuffers tables that the Swift and Kotlin renderers read
  through their generated code (native first, D4).
- **Arrow IPC:** not a UI body by default. Keep it, as a `[ubyte]` field in its own union
  member, only where data goes to Arrow tools. See Q20.
- **Drop MessagePack.** It has no schema, so it cannot be generated for every language.
- **Drop the eight-way `StateFormatPayload` union.** The server picks one encoding for state
  (a generated table per component), and the client treats it as opaque.

See questions Q1, Q2, Q3, Q19 and Q20 in [`questions.md`](questions.md).
