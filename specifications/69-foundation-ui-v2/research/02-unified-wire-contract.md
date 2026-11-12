# 02. One wire contract: FlatBuffers as the top format

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); WebAssembly (WASM); JavaScript (JS);
TypeScript (TS); Hypertext Transfer Protocol (HTTP); Server-Sent Events (SSE); inter-process
communication (IPC); FlatBuffers binary schema (BFBS); identifier (id); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. The idea, laid out

Source: `foundation_ui_rearchitecture.md` §1 ("Unified Network Contract") and §2.

**One FlatBuffers schema is the contract for every frame, in both directions, on every
platform.** The owner's review of PR #157 makes this exact: FlatBuffers is the top format,
and everything else, columnar included, is a type in that schema. The schema is
code-generated for every language we support.

The document's schema has four parts:

1. **Outbound envelope** `WireEnvelope { version, sequence_id, payload: PayloadData }`.
2. **Body union** `PayloadData = Columnar | Arrow | MessagePack | MobileLayout`. A union gives
   each body a typed tag. A reader that does not know a tag reads it as `NONE` and can skip
   it, so an old client survives a new server.
3. **Inbound envelope** `UserInteractionRequest { version, sequence_id, node_id, event_name,
   active_state, input_value }`.
4. **Typed operations** for native layout (`CreateElementOp`, `SetTextOp`, `SetAttributeOp`,
   `AppendChildOp`) in a `DomOperation` union.

Why FlatBuffers:

| Property | What it gives |
|---|---|
| Zero-copy reads | A reader follows offsets in the received buffer. No parse step, no allocation per field. |
| Schema evolution | Add fields at the end, deprecate fields, never reuse an id. Old and new readers both work. |
| Unions | Typed choice of body with a safe unknown case. |
| One schema, many languages | `flatc` generates Rust, TS, Swift, Kotlin, and more from one `.fbs` file. |
| Sequence ids | Ordering and gap detection, in both directions. |

_North star fit: this file is the standard at the top. It carries UI, data and actions on
every surface; [13](./13-platform-capabilities.md) shows the layers below it (D35)._

## 2. Current state

| Part | Where |
|---|---|
| 6-byte header `[protocol:1][version:1][length:4 LE]` | `backends/foundation_ui_traits/src/envelope.rs:16-27` |
| Protocol byte table: 0 custom binary, 1 Arrow or columnar, 2 JSON; byte 1 holds two formats chosen by the version byte | `backends/foundation_ui_traits/src/encoder.rs:29-36`, `columnar_encoder.rs:8-11` |
| 14-byte `WasmEnvelope` at the WASM-to-JS boundary | `backends/foundation_wasm/src/protocol.rs:96` |
| Hand-written encoders per body, hand-written JS parser | `columnar_encoder.rs`, `foundation-wasm-ui.js:94-187` |
| No inbound typed request; spec 39 decision 025 sketches JSON | `specifications/completed/39-foundation-wasm-ui/decisions/025-server-client-state-sync.md:44-52` |
| No crate uses FlatBuffers directly (Arrow IPC uses it inside) | — |

Spec 36 decision 10 (`specifications/completed/36-agentic-api/decisions/10-serialization-format.md`)
is about how `foundation_ai` **stores conversation data** (messages, tool calls, memory
snapshots, events): always as JSON and as Arrow. It once kept FlatBuffers as a fallback if Arrow
did not build on WASM, and dropped that fallback (lines 116-118). It does not cover the UI wire,
and stored `foundation_ai` data is out of scope for this design (D26).

## 3. The new way

### 3.1 The schema is the product

- `.fbs` files in the `foundation_ui_wire` crate are the single source of truth (owner answer
  to Q2, 2026-11-11).
- `flatc`, pinned to one version, generates Rust, TS, Swift and Kotlin. A pure-Rust generator
  (`planus`) is not enough, because it generates only Rust.
- Generated code is checked in. CI regenerates and fails on any difference, so a normal build
  does not need `flatc`.
- Every body is a schema type. Two schema types hold opaque `[ubyte]` blocks: Arrow IPC, for
  data that goes to Arrow tools ([03](./03-payload-tiers.md)), and MessagePack (D9). No
  generated reader reads those blocks.

### 3.2 Outbound frame

```flatbuffers
namespace ewe.ui;

table Frame {
  schema_version: ushort;      // bumps only on a breaking change
  stream_id: ulong;            // one per mounted root or live stream
  sequence_id: ulong;          // strictly increasing per stream_id
  ack_action: ulong;           // sequence_id of the ActionRequest this answers, 0 if push
  body: Body;
}

union Body {
  Ops: OpBatch,                // tier 1: typed operations (03)
  Columnar: ColumnarBatch,     // tier 2: aligned columns (03)
  Table: DataTable,            // tier 3: typed bulk rows (03)
  ArrowIpc: ArrowIpcBlob,      // escape hatch for Arrow consumers (03)
  MessagePack: MessagePackBlob, // opaque bytes, kept (D9)
  StateUpdate: SignedState,    // new signed state for a component (07, 08)
  Capabilities: ServerCaps,    // answer to the client hello (11)
  Error: ActionError
}

table MessagePackBlob { bytes: [ubyte]; }   // no generated reader
```

MessagePack stays (D9). It is a schema type that carries one opaque byte block, like the
Arrow escape hatch. The difference is that no generated code reads its content. A client that
needs it decodes it itself. New features use the typed bodies.

The `Ops` body holds typed operations. One of them, `HtmlMorph`, carries an HTML fragment for
the web target ([01](./01-compile-time-targeting.md), [03](./03-payload-tiers.md)). It is not a
body type. The server sends it only to a client that declares `mount: Web` in `ClientHello`
(D8, D11, [11](./11-app-store-and-command-bus.md)).

**One message, one form per change.** The schema has one set of forms that every platform can
do, plus platform-specific forms only for what that set cannot express (D36, owner review
2026-11-12):

- **Cross-platform** members work on every platform: `CreateNode`, `SetText`, `SetProp`,
  `AppendChild`, `Remove` and `ReplaceSubtree`.
- **Web** members exist only for what the cross-platform set cannot express: `HtmlMorph` and
  `ScriptNode`.
- **Mobile** members exist only for what the cross-platform set cannot express: `NativeControl`
  and `NativeCommand`.

A change has one form. If the cross-platform set can express it, every platform uses that form.
It does not also get a web form or a mobile form. If it cannot, it gets a form for one platform
only, with no cross-platform twin:

```flatbuffers
table OpRow { op: Op; }
```

A cross-platform member applies on every platform. A platform-specific member applies on its
own platform. A runtime that meets a member for another platform reports
`UnsupportedPlatformOp` and continues the batch. One batch can hold cross-platform members and
platform-specific members together; each client applies its own. Section
[13](./13-platform-capabilities.md) §3.2 gives the full rule.

### 3.3 Inbound request

```flatbuffers
table ActionRequest {
  schema_version: ushort;
  stream_id: ulong;
  sequence_id: ulong;          // client counter, for ordering and duplicate detection
  action: string;              // "server:name"; the reference sets the origin (10, 11, D31, D37)
  node_id: uint;
  input: ActionInput;          // typed, unsigned user input (10)
  state: SignedState;          // returned as received, never edited (07, 08)
}

table ClientHello {            // first message on a connection (11)
  schema_version: ushort;
  vocabulary_version: uint;
  action_registry_hash: [ubyte];
  platform: Platform;          // Web, Ios, Android, Desktop
  mount: MountSurface;         // Native or Web (D11)
  resume_from: ulong;          // last sequence_id seen, for reconnect
}

enum MountSurface : ubyte { Native = 0, Web = 1 }
```

`platform` says what the machine is. `mount` says what renders the frames. A browser declares
`Web`. A native app can declare `Native` or `Web` (a WebView). `mount: Web` can receive
`HtmlMorph`; `mount: Native` cannot (D11, [06](./06-native-rendering.md),
[11](./11-app-store-and-command-bus.md)).

`action` carries the full reference, for example `server:increment_counter`. The server rejects
a reference whose origin is not `server:` with a typed error. A `native:` or `js:` reference
never crosses the transport ([10](./10-action-model.md) §3.8).

### 3.4 Ordering and resume

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server (any replica)
    C->>S: ClientHello(resume_from = 17)
    S->>C: Frame(seq 18), Frame(seq 19)
    C->>S: ActionRequest(seq 5, action server:increment_counter)
    S->>C: Frame(seq 20, ack_action 5, Ops)
    Note over C: gap at seq 21? ask to resume from 20
```

_The diagram shows how sequence ids let a client detect a gap and resume on any replica._

### 3.5 Improvements over the document

| Document | New way | Why |
|---|---|---|
| `MessagePack` body | Kept as an opaque schema type (`MessagePackBlob`) | The owner keeps it (D9). It has no generated reader; a client that needs it decodes it itself. |
| `Columnar` and `Arrow` as opaque bytes | Typed schema tables ([03](./03-payload-tiers.md)) | Generated readers; no hand-written parsers. |
| `event_name: string` | `action: string` plus `node_id` | The name is the tag the compiled registry uses. No derived numeric id (D31, [11](./11-app-store-and-command-bus.md)). |
| One union for every target | Cross-platform forms plus platform-specific forms | A change has one form; a platform-specific form exists only where the cross-platform set cannot express it (D36, [13](./13-platform-capabilities.md) §3.2). |
| `sequence_id: uint` only | `stream_id` plus 64-bit `sequence_id` plus `ack_action` | Per-stream ordering, resume after reconnect, request and response pairing. |
| No handshake | `ClientHello` / `ServerCaps` | Version skew is handled before the first frame. |
| Eight-way state union | One `SignedState` table ([07](./07-element-riding-state.md)) | The client never reads state, so its format is a server choice. |
| HTML morph as a separate path | `HtmlMorph`, a web-only member of the `Op` union ([01](./01-compile-time-targeting.md)) | The morph works on the web; the schema carries it and the platform gate keeps it off native (D8). |

## 4. What this changes for us

- The 6-byte `Envelope`, the protocol byte table and the hand-written body encoders are
  replaced, not extended. This includes the hand-written JavaScript (JS) parser: the web
  runtime keeps its apply loop and its `<mount-stream>` and `<mount-data>` paths, and reads
  frames through the generated TypeScript reader ([05](./05-web-zero-copy-applicator.md)). No
  web feature is dropped.
- The new `foundation_ui_wire` crate holds the schema and the generated Rust.
  `foundation_ui_traits` stays dependency-free and `no_std`.
- Frame size caps and resume backlog length are configuration with documented defaults
  (`CLAUDE.md` rule 8).

## 5. Open questions

Q3 in [`questions.md`](./questions.md) is answered: enforce 8-byte alignment (D24). Q4 is
answered: the transport is a boundary concern, and SSE, HTTP and WebSocket already work (D25).
Q5 is removed (D26). Q2 is answered: `foundation_ui_wire`.
