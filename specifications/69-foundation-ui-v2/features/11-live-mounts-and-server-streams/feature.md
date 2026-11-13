---
feature: "live-mounts-and-server-streams"
description: "The live mount contract on the web and the server: a live channel that sends Frames in sequence order with a replay backlog and resume, the mount elements on the frame protocol, and frames from local WASM logic."
status: "pending"
priority: "high"
model: "sonnet"
depends_on: ["03-frame-writer-and-tiers", "09-web-frame-applicator"]
estimated_effort: "large"
created: 2026-11-13
last_updated: 2026-11-13
author: "Main Agent"
crates: ["foundation_wasm_ui", "foundation_wasm", "foundation_browser"]
decisions: ["D4", "D15", "D25", "D29", "D30", "D39", "D40"]
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0
---

# Feature 11: Live mounts and server streams

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); Document Object Model (DOM); JavaScript
(JS); WebAssembly (WASM); Server-Sent Events (SSE); hypertext transfer protocol (HTTP);
identifier (id); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

## 1. Goal

A live mount is a region that marks itself live, opens a channel, and applies the frames
that arrive, in order. This feature gives the live mode of the server a channel that sends
`Frame` buffers with sequence ids, keeps a replay backlog and serves a resume. The existing
`<mount-stream>` and `<mount-data>` elements carry frames with no new transport. Local WASM
logic sends the same frames through `host_apply`. The current `Broadcaster` and the current
wires stay as they are.

## 2. Story

- **S1**: "A server App changes a signal, and every connected page shows the change."
- **S2**: "A page joins late and gets the frames that it missed."
- **S3**: "A page loses its connection and resumes after the last frame that it applied."
- **S4**: "A data mount shows only the latest value."
- **S5**: "Local WASM logic changes the page with the same frames as the server."
- **S6**: "A web page and a native app listen to one channel, and each gets its own forms."
- **S7**: "A client asks to resume from a frame that the backlog no longer has."
- **S8**: "A connection is slow or closed during a push."
- **S9**: "A region leaves the page, and its channel closes."

| # | Use case | Actor | Trigger | Result |
|---|---|---|---|---|
| U1 | Push | Server App | `stabilize()` with pending ops | One frame per mount surface, sent to each connection of that surface |
| U2 | Join | Client | `LiveChannel::register(conn, caps, 0)` | The whole backlog, then live frames |
| U3 | Resume | Client | `register(conn, caps, n)` | The frames after `n`, then live frames |
| U4 | Latest only | `<mount-data>` on the frame protocol | Several frames before a flush | Only the newest applies |
| U5 | Local frames | WASM App | `App::frames()` and `stabilize()` | A bare frame through `host_apply`; the applicator applies it |
| U6 | Close | Region unmount | `disconnectedCallback` | The transport disconnects (existing behaviour) |

## 3. Bounded context

**Inside this feature:** `LiveChannel`, `LiveTx`, `FrameBroadcastSink`, `BacklogConfig`,
`LiveError` in `foundation_wasm_ui::server`; `FrameV1` in `foundation_wasm_ui::protocol`
and `App::frames`; the frame seam in `foundation-wasm.js`; the resume and latest-only
behaviour of the mount elements in `foundation-ui-frames.js`.

**Outside:**

| Thing | Owner |
|---|---|
| `Broadcaster`, `BroadcastTx`, `BroadcastSink`, `FrameTransport` | `foundation_wasm_ui::server` (spec 39). They do not change (D15, D40) |
| `App`, the signal graph, `stabilize` | `foundation_wasm_ui`, `foundation_signals` |
| `web_form` | Feature 02 |
| `gate`, `write_ops`, `StreamSequencer`, `ClientCaps` | Feature 03 |
| `FrameApplicator` (ordering, `onGap`, `setCursor`), the `LiveMount` element creation, the frame routing seams | Feature 09 |
| The SSE and WebSocket servers and the header negotiation | The host and `foundation_browser`'s test server (D25, D30) |
| The native live mount record | Features 14 and 15 |

| Entity | Meaning | Owner (the one writer) |
|---|---|---|
| Live channel | Connections, a backlog and a sequencer for one `stream_id` | `LiveChannel` |
| Backlog | The sent frames, by sequence id, one encoding per mount surface | `LiveChannel` |
| Mount cursor | The last applied sequence id of a mount's stream | `FrameApplicator` (feature 09) |

| Verb | Kind | Trigger | Effect |
|---|---|---|---|
| `push_ops` | story | App | Gate per surface, encode, store, send to each connection |
| `register` | story | A new connection | Replay from the backlog, then add the connection |
| `close` | entity | Teardown | Drops the connections |
| resume (web) | story | `onGap` or a reconnect | The mount reconnects with `resume_from` |

**Words.** "Channel" is one ordered stream of frames with one `stream_id`. "Backlog" is the
list of frames a late or resuming client can get. "Resume" is a register with a
`resume_from` greater than 0.

## 4. Rules

### 4.1 Law worksheets

```
Entity: Live channel
Conserved value:         every connection of a surface gets the same frame bytes for one sequence id
Bound:                   backlog length <= BacklogConfig::max_frames when that limit is set
Inverse pairs:           register / (a failed send removes the connection)
Idempotent verbs:        close
Ordering pairs:          a frame is stored in the backlog before it is sent
Monotone values:         sequence ids strictly increase, with no gap
Freeze points:           stream_id is fixed at construction; a stored frame never changes
Links to other entities: Frame.stream_id and Frame.sequence_id equal the channel's values
```

```
Entity: Backlog
Conserved value:         the frames are in sequence order
Bound:                   see the channel
Inverse pairs:           none
Idempotent verbs:        none
Ordering pairs:          none
Monotone values:         the oldest retained sequence id never decreases
Freeze points:           none
Links to other entities: none
```

### 4.2 Invariants

| # | Invariant |
|---|---|
| I1 | A connection receives the frames of its channel in strictly increasing sequence order, with no gap after its first frame. |
| I2 | `register(conn, caps, n)` sends exactly the stored frames with sequence id greater than `n`, in order, before any live frame. |
| I3 | A connection with `mount: Native` never receives a `Web` form from a channel; a connection with `mount: Web` never receives a `Mobile` form. |
| I4 | A push stores the frame before it sends it, and a register replays under the same lock, so a connection never misses a frame between its replay and its first live frame. |
| I5 | A connection whose `send_binary` returns false is removed and gets no more frames. The other connections are not affected. |
| I6 | When `max_frames` is set, `register` with `n` lower than `oldest - 1` (frames between were dropped) fails with `LiveError::ResumeTooOld` and sends nothing. |
| I7 | A `<mount-data>` on the frame protocol applies only the newest queued frame of its stream at a flush. A `<mount-stream>` applies every frame. |
| I8 | A frame from local WASM logic and a frame from the server with the same ops give the same DOM. |
| I9 | The existing `Broadcaster` tests and the existing mount tests pass with no change. |

### 4.3 Must have

- `BacklogConfig { max_frames: Option<usize> }`. The default is `None`: keep every frame,
  which is what `Broadcaster` does today (`server/mod.rs:64-67`, `:151-155`). A page that
  is built only from the stream needs the whole history to join late.
- A GitHub issue (severity medium) for the unbounded backlog of `Broadcaster` and of
  `LiveChannel` with the default config: the likely fix is a snapshot frame that replaces
  the dropped history. Do not build the snapshot in this feature (rule 9).

### 4.4 Must do

- Encode a pushed batch one time for the web surface and one time for the native surface,
  and only for a surface that has a connection or when the backlog must hold it (always
  store both encodings; a later joiner of either surface needs them).
- Use the existing transports. A binary frame goes through `FrameTransport::send_binary`;
  the page detects it by its identifier (feature 09 seam 2 and 5).

### 4.5 Must not

- Do not change `Broadcaster`, `BroadcastTx`, `BroadcastSink` or `FrameTransport`.
- Do not prescribe a thread, a Durable Object or a replica layout (D30).
- Do not let a stream frame change a component's signed state by a side path. A push that
  changes state sends an `AttachState` op in the batch.
- Do not spin on a slow connection. A full transport queue returns false, and the
  connection is dropped (the client resumes).

## 5. Acceptance scenarios

`RecordingTransport` is a test `FrameTransport` that keeps every frame.

```gherkin
Feature: Live mounts and server streams

  Scenario: A push reaches every connection in order
    Given a LiveChannel for stream 7 with two web connections
    When the App pushes [SetText(42,"1")] and then [SetText(42,"2")]
    Then each connection received two frames with sequence 1 and 2, stream 7 and ack_action 0

  Scenario: A late joiner gets the backlog first
    Given a LiveChannel that pushed 3 frames
    When a web connection registers with resume_from 0
    Then it received the frames with sequence 1, 2 and 3, in order
    And a later push gives it sequence 4

  Scenario: A resume gets only the missed frames
    Given a LiveChannel that pushed 5 frames
    When a web connection registers with resume_from 3
    Then it received exactly the frames with sequence 4 and 5

  Scenario: Each surface gets its own forms
    Given a LiveChannel with one web connection and one native connection
    When the App pushes [SetText(42,"x"), HtmlMorph(node 40,"<b>x</b>"), NativeCommand(HAPTIC)]
    Then the web connection's frame holds SetText and HtmlMorph
    And the native connection's frame holds SetText and NativeCommand
    And both frames have sequence 1

  Scenario: A dead connection is dropped and the others continue
    Given a LiveChannel with a connection that returns false and a recording connection
    When the App pushes two batches
    Then the recording connection received two frames
    And conn_count() is 1

  Scenario: A resume that is too old is refused
    Given a LiveChannel with BacklogConfig max_frames 2 that pushed 5 frames
    When a connection registers with resume_from 1
    Then register fails with LiveError::ResumeTooOld { oldest: 4 }
    And the connection received nothing

  Scenario: A client with an older vocabulary is refused
    Given a LiveChannel
    When a connection registers with ClientCaps whose vocabulary_version is 0
    Then register fails with LiveError::UpdateRequired

  Scenario: The App sink turns DomOps into frames
    Given an App built with FrameBroadcastSink over a LiveChannel with a recording web connection
    When the App mounts <span>{count}</span> and sets count to 5 and stabilizes
    Then the recording connection received frames
    And applying them to an empty mock document gives a span with the text "5"

  Scenario: A mount-stream on the frame protocol applies every frame
    Given <mount-stream api="/live" protocol="frame" transport="sse"> and a fake transport
    When the transport delivers frames with sequence 1, 2 and 3 that set node 42 to "a", "b", "c"
    Then node 42 showed "a", then "b", then "c"

  Scenario: A mount-data on the frame protocol keeps the latest value
    Given a <mount-data> created by LiveMount(kind Data, channel 9) on the frame protocol
    When frames with sequence 1, 2 and 3 for stream 9 are queued before one flush
    Then only the frame with sequence 3 applied
    And the cursor of stream 9 is 3

  Scenario: A gap makes the mount reconnect with resume_from
    Given a <mount-stream api="/live" protocol="frame"> whose stream cursor is 19
    When a frame with sequence 21 arrives and the gap timer fires
    Then the mount disconnected its transport
    And it connected again to "/live?stream_id=7&resume_from=19"

  Scenario: Local WASM logic sends bare frames
    Given a WASM App built with App::frames()
    When the App stabilizes one SetText
    Then host_apply received bytes whose bytes 4 to 8 are "EWF1"
    And the JS dispatcher passed them to the frame handler, not to a protocol-byte handler

  Scenario: An old envelope from WASM still works
    Given the frame seam is installed
    When host_apply receives a 14-byte-envelope columnar message
    Then the columnar handler applied it
```

## 6. Technical breakdown

### 6.1 Measured facts

| Fact | Source |
|---|---|
| `Broadcaster` keeps `conns` and an unbounded `backlog`; `register` replays the whole backlog under the lock, then adds the connection; `push` writes to each connection with `retain_mut`, then stores | `backends/foundation_wasm_ui/src/server/mod.rs:64-67`, `:97-102`, `:151-155` |
| `FrameTransport` has `send_binary` and `send_text`; each returns false when the connection is gone. The SSE test transport queues to a bounded pipe with `try_send` | `server/mod.rs:38-45`; `backends/foundation_browser/src/test/stream.rs:103-115` |
| `BroadcastSink` implements `ProtocolHandler` and `ProtocolMethods<Vec<DomOp>>`; `encode_and_write` encodes and calls `tx.send` | `server/mod.rs:160-232` |
| `App::with_protocol(protocol)` takes any `ProtocolMethods<Vec<DomOp>>` | `backends/foundation_wasm_ui/src/app.rs:116` |
| `ColumnarV1::write_batch` writes a 14-byte `WasmEnvelope` plus the payload into one arena slot and returns `(SendResult, ptr, len)`; `ship` calls `host_apply` | `backends/foundation_wasm_ui/src/protocol/columnar.rs`, `protocol/mod.rs:47-49` |
| JS `host_apply(memId, ptr, len)` calls `dispatcher.dispatch(memory.get(memId).bytes)` and then disposes the slot; `ProtocolDispatcher.dispatch` parses the `WasmEnvelope` and picks a handler by protocol byte | `backends/foundation_wasm/runtime/foundation-wasm.js:1967-1973`, `:1363-1383` |
| `<mount-stream>` reads `api`, `transport`, `protocol`, `target`, `data`; default transport `sse`; `disconnectedCallback` disconnects | `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js:2334-2393` |
| An SSE `arrow` event is base64 of an envelope-framed message and goes to `decodeEnvelopeFrame` | `foundation-wasm-ui.js:1958-1967` |

### 6.2 Files

```
backends/foundation_wasm_ui/src/server/live.rs            LiveChannel, LiveTx, BacklogConfig, LiveError, FrameBroadcastSink
backends/foundation_wasm_ui/src/server/mod.rs             `mod live; pub use live::*;` only
backends/foundation_wasm_ui/src/protocol/frame.rs         FrameV1
backends/foundation_wasm_ui/src/protocol/mod.rs           `mod frame; pub use frame::FrameV1;`
backends/foundation_wasm_ui/src/app.rs                    App::frames
backends/foundation_wasm_ui/tests/live_channel_tests.rs
backends/foundation_wasm_ui/tests/frame_protocol_tests.rs
backends/foundation_wasm/runtime/foundation-wasm.js       one seam (section 6.6)
backends/foundation_wasm_ui/runtimes/foundation-ui-frames.js   mount resume and latest-only (section 6.5)
backends/foundation_wasm_ui/integration/test/frames-live.test.js
backends/foundation_browser/src/test/stream.rs            a `/live` frame route for the browser test
```

`foundation_wasm_ui` gains `foundation_ui_wire` as a normal dependency (it was a
dev-dependency in feature 09). `server/live.rs` is native-only, like `server/mod.rs`.

### 6.3 Server API (`server/live.rs`)

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BacklogConfig { /* max_frames: Option<usize> */ }
impl BacklogConfig {
    /// Keep at most `n` frames. `n` must be greater than 0.
    #[must_use] pub fn max_frames(self, n: usize) -> Self;
    /// Keep every frame (the default).
    #[must_use] pub fn unbounded(self) -> Self;
    #[must_use] pub fn limit(&self) -> Option<usize>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveError {
    /// The client's vocabulary version is not the current one.
    UpdateRequired,
    /// Frames between `resume_from` and `oldest` were dropped from the backlog.
    ResumeTooOld { oldest: u64 },
    /// The first frame of the replay could not be written to the connection.
    ConnectionClosed,
    Write(WriteError),
}

/// A live channel: connections, a backlog and a sequencer for one stream. Cheap to clone.
#[derive(Clone)]
pub struct LiveChannel { /* inner: Arc<Mutex<LiveInner>> */ }
impl LiveChannel {
    #[must_use] pub fn new(stream_id: u64, backlog: BacklogConfig, writer: WriterConfig) -> Self;
    #[must_use] pub fn stream_id(&self) -> u64;
    /// Replays the stored frames after `resume_from`, then adds the connection.
    pub fn register(&self, conn: impl FrameTransport + Send + 'static, caps: &ClientCaps,
        resume_from: u64) -> Result<(), LiveError>;
    /// A `Send` handle for pushing (held by `FrameBroadcastSink`).
    #[must_use] pub fn sender(&self) -> LiveTx;
    pub fn close(&self);
    #[must_use] pub fn conn_count(&self) -> usize;
    #[must_use] pub fn last_sequence(&self) -> u64;
    #[must_use] pub fn backlog_len(&self) -> usize;
}

#[derive(Clone)]
pub struct LiveTx { /* inner: Arc<Mutex<LiveInner>> */ }
impl LiveTx {
    /// Encodes `ops` for each mount surface, stores the frames, sends them.
    /// Returns the sequence id of the frame.
    pub fn push_ops(&self, ops: Vec<OpT>) -> Result<u64, LiveError>;
}

/// An App protocol sink that sends each batch as a frame through a `LiveTx`.
pub struct FrameBroadcastSink { /* tx: LiveTx, next_id: Cell<u32> */ }
impl FrameBroadcastSink { #[must_use] pub fn new(tx: LiveTx) -> Self; }
impl ProtocolHandler for FrameBroadcastSink { /* protocol_byte 3, version 1; the other methods are no-ops, as in BroadcastSink */ }
impl ProtocolMethods<Vec<DomOp>> for FrameBroadcastSink { /* section 6.4 */ }
```

`LiveInner { conns: Vec<(MountSurface, Box<dyn FrameTransport + Send>)>,
backlog: VecDeque<StoredFrame>, sequencer: StreamSequencer, config, writer }` and
`StoredFrame { sequence: u64, web: Vec<u8>, native: Vec<u8> }`.

Version 1 serves two capability sets: `ClientCaps::web_current()` and
`ClientCaps::native_current(Platform::Unknown)`. `register` maps `caps.mount` to one of
them and fails with `UpdateRequired` when `caps.vocabulary_version != VOCABULARY_VERSION`.
`caps.known_actions` is not used here: a live frame from an App carries no `BindAction`
that the App did not render for this build.

### 6.4 `push_ops` and `register`

```mermaid
sequenceDiagram
    participant A as App (its own thread)
    participant S as FrameBroadcastSink
    participant L as LiveChannel (lock)
    participant C1 as Web connection
    participant C2 as Native connection
    A->>S: encode_and_write(DomOps)
    S->>S: web_form for each DomOp
    S->>L: push_ops(ops)
    L->>L: seq = last + 1; gate + write_ops for web and for native
    L->>L: store StoredFrame; advance the sequencer; drop the oldest if over the limit
    L->>C1: send_binary(web bytes)
    L->>C2: send_binary(native bytes)
    L-->>S: seq
```

_The diagram shows one push. The lock covers the encode, the store and the sends, so a
register cannot run between them._

**`push_ops`**, under the lock:

1. `header = FrameHeader { stream_id, sequence_id: sequencer.last() + 1, ack_action: 0 }`.
2. `write_ops(ops.clone(), &web_caps, header, &writer)` and the same for native. An error
   returns `LiveError::Write`; nothing is stored and the sequencer does not move.
3. `sequencer.next()`; push the `StoredFrame`; if a limit is set and the backlog is longer,
   pop from the front.
4. `conns.retain_mut(|(mount, conn)| conn.send_binary(bytes for mount))`.
5. Return the sequence id.

**`register`**, under the lock:

1. The vocabulary check (`UpdateRequired`).
2. `oldest` = the sequence of the front of the backlog, or `last + 1` when it is empty. If
   a limit is set and `resume_from + 1 < oldest`: `ResumeTooOld { oldest }`.
3. For each stored frame with `sequence > resume_from`, in order: `send_binary`. A false
   return gives `ConnectionClosed`, and the connection is not added.
4. Add the connection.

**`FrameBroadcastSink::encode_and_write`**: map each `DomOp` with `web_form`; call
`tx.push_ops`; on an error, log it with `tracing::error!` and report zero bytes in the
`SendResult` (the trait returns no error; this matches how `BroadcastSink` cannot fail).
Return `(SendResult { memory_id: MemoryId::new(id, 0), op_count, encoded_bytes }, null, 0)`
as `BroadcastSink` does.

### 6.5 The web side (`foundation-ui-frames.js`)

1. **Latest only.** `FrameApplicator` gains `setLatestOnly(streamId, bool)`. In `flush`,
   for a stream marked latest-only, apply only the last queued live frame of that stream;
   the cursor still moves to that frame's sequence. Out-of-order rules do not apply to a
   latest-only stream: a frame with a sequence above the cursor is queued at once, and one
   at or below it is ignored.
2. **Mount binding.** `installFrameRuntime` wraps the mount elements once: when a
   `mount-stream` or `mount-data` with `protocol="frame"` connects, read
   `data-channel-id` (a decimal `u64`; absent means the first frame's `stream_id` decides)
   and `data-resume-from`; call `frames.setCursor(streamId, resumeFrom)` when
   `resumeFrom > 0`; for `mount-data` with a `data-channel-id`, call
   `setLatestOnly(streamId, true)`. Keep a `Map` from stream id to the mount element. Do
   this by a `connectedCallback` hook: `installFrameRuntime` sets
   `wasmUi.Patcher.runtime.onMountConnect = (element) => {…}`, and section 6.7 adds the one
   call to it.
3. **Resume.** The applicator's `onGap({ streamId, resumeFrom })` (default set by
   `installFrameRuntime`): find the mount element of `streamId`; call
   `element._transport?.disconnect?.()`; set `data-resume-from`; build the URL
   `api + (api.includes("?") ? "&" : "?") + "stream_id=" + streamId + "&resume_from=" + resumeFrom`;
   reconnect with the same transport kind and the same result callback. With no mount
   element for the stream, call the user's `onGap` only.
4. **`LiveMount` transport `Local`.** The element gets `transport="local"`. A local mount
   opens no network channel: its frames arrive from WASM through `host_apply`
   (section 6.6) and are routed by `stream_id`. `mountSetup` is not called for it; the
   wrapper of item 2 returns early for `transport="local"`.

The resume URL parameters are a convention of this runtime. The host reads them and calls
`LiveChannel::register(conn, caps, resume_from)`. The frame protocol itself does not
depend on them (D25).

### 6.6 Local WASM frames

**Rust (`protocol/frame.rs`)**

```rust
/// Layer 3 protocol that ships each DomOp batch as one bare `Frame` (no WasmEnvelope).
pub struct FrameV1 { /* sequencer: RefCell<StreamSequencer>, config: WriterConfig */ }
impl FrameV1 {
    /// `stream_id` names the local channel. Use 0 for the page's default stream.
    #[must_use] pub fn new(stream_id: u64) -> Self;
}
pub const PROTOCOL_FRAME: u8 = 3;
impl ProtocolHandler for FrameV1 { /* protocol_byte PROTOCOL_FRAME, version 1; send_to_js = ship */ }
impl ProtocolMethods<Vec<DomOp>> for FrameV1 {
    // encode_and_write: web_form each op; write_ops(ops, &ClientCaps::web_current(),
    //   sequencer.header(0), &config); allocate one arena slot of the frame length; copy the
    //   frame bytes into it with no header; return (SendResult, ptr, len).
    // handle_received: Ok(Vec::new()) (send-only, as BroadcastSink).
}
impl App { /// An App that ships frames to the page (D29).
    #[must_use] pub fn frames() -> Self { Self::with_protocol(FrameV1::new(0)) } }
```

A `write_ops` error here is a bug in the op list (for example a frame above the limit).
`encode_and_write` cannot return it, so it logs with `tracing::error!`, ships nothing, and
returns a `SendResult` with `op_count: 0`. The arena calls keep the `expect` style of
`ColumnarV1::write_batch`.

The slot address may not be 8-aligned. Then the applicator copies the columns and counts
them (feature 09 I7). This is correct, and it is visible in `stats`.

**JS (`foundation-wasm.js`)**: one seam in `ProtocolDispatcher`:

```js
// in the constructor:
this.frameHandler = null;
// at the start of dispatch(buffer):
const bytes = buffer instanceof Uint8Array ? buffer : new Uint8Array(buffer);
if (this.frameHandler && bytes.byteLength >= 8 &&
    bytes[4] === 0x45 && bytes[5] === 0x57 && bytes[6] === 0x46 && bytes[7] === 0x31) {
  return this.frameHandler(bytes.slice()); // copy: host_apply frees the slot after dispatch
}
```

`installFrameRuntime(wasmUi, { wasmRuntime })`: when `wasmRuntime` is given, set
`wasmRuntime.dispatcher.frameHandler = (bytes) => frames.receive(bytes)`.

A `WasmEnvelope` whose bytes 4 to 7 happen to be `"EWF1"` would be taken for a frame. Those
bytes are part of the 8-byte memory id; the value needs an arena index of 826 754 885,
which an arena does not reach. Record this in a comment at the seam.

### 6.7 One more seam in `foundation-wasm-ui.js`

In `MountDataComponent.connectedCallback` and `MountStreamComponent.connectedCallback`,
as the first statement inside the `try`:
`if (Patcher.runtime.onMountConnect?.(this) === "handled") return;`. Add
`onMountConnect: null` to `Patcher.runtime`. The hook returns `"handled"` only for
`transport="local"`.

### 6.8 Fault tables

**`push_ops`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| Encode (step 2) | Returns | `LiveError::Write` | nothing; no sequence id was used |
| A send returns false (step 4) | Removes that connection | `Ok(seq)` | The frame is in the backlog; the other connections got it. The removed client resumes |

**`register`**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| Vocabulary check | Returns | `UpdateRequired` | nothing |
| Backlog check | Returns | `ResumeTooOld` | nothing |
| A replay send returns false | Stops the replay | `ConnectionClosed` | The connection got a prefix of the replay; it is not in the channel. The client reconnects with its own cursor |

**Resume on the web**

| Step that fails | What the system does | What the caller sees | What is left behind |
|---|---|---|---|
| The reconnect fails | The existing transport retry of the mount applies (`reconnectDelay`) | The existing console error of the mount | The cursor stays; the region shows its last applied frame |
| The server answers `ResumeTooOld` (the host maps it to an `Error` frame with code `UpdateRequired`) | The applicator calls `onError` | `ewe:error` on `document` | The region is stale until the page reloads. The action runtime of feature 10 reloads on `UpdateRequired` |

## 7. Tests

| Law or scenario | Test | Oracle |
|---|---|---|
| I1; Scenario: push in order | `tests/live_channel_tests.rs::push_reaches_every_connection_in_order`; `::sequences_have_no_gap` (50 pushes, 3 connections that join at pushes 0, 10 and 20 with resume 0) | example, property |
| I2; Scenarios: late joiner, resume | `live_channel_tests.rs::late_joiner_gets_backlog_first`, `::resume_gets_only_missed_frames` | example |
| I3; Scenario: each surface | `live_channel_tests.rs::each_surface_gets_its_own_forms` | example |
| I4 | `live_channel_tests.rs::no_frame_is_missed_between_replay_and_live`: one thread pushes 1000 frames while another registers 20 connections at random points (seeded); each connection's frames are consecutive from its first | stateful model |
| I5; Scenario: dead connection | `live_channel_tests.rs::dead_connection_is_dropped` | fault |
| I6; Scenario: resume too old | `live_channel_tests.rs::resume_too_old_is_refused`, `::bounded_backlog_drops_oldest` | expected outcome |
| Scenario: older vocabulary | `live_channel_tests.rs::older_vocabulary_is_refused` | expected outcome |
| Scenario: App sink | `live_channel_tests.rs::app_sink_turns_dom_ops_into_frames` (decode with `read_frame` and `columnar_rows`; compare with `web_form` of the DomOps that a `MockProtocol` App records for the same steps) | metamorphic |
| Fault: encode error leaves the sequencer | `live_channel_tests.rs::write_error_uses_no_sequence` (a `WriterConfig` with `max_frame_bytes` 64) | fault |
| I7; Scenarios: mount-stream, mount-data | `integration/test/frames-live.test.js` "mount-stream applies every frame", "mount-data keeps the latest value" | example |
| Scenario: gap reconnect | `frames-live.test.js` "a gap reconnects with resume_from" (fake transport that records `connect` URLs) | example |
| I8; Scenarios: local WASM frames, old envelope | `tests/frame_protocol_tests.rs::frame_v1_ships_bare_frames` (native: the slot bytes pass `read_frame`); `frames-live.test.js` "dispatcher routes EWF1 to the frame handler", "an old envelope still dispatches by protocol byte"; `frames-live.test.js` "local and server frames give the same DOM" (the same fixture through `frameHandler` and through `Patcher.route`) | example, metamorphic |
| Local mount opens no network channel | `frames-live.test.js` "a local live mount does not connect" | example |
| I9 | `cargo test -p foundation_wasm_ui` and `mise run ui-web:test` pass with no edit to an existing test | expected outcome |
| End to end | `examples/ui_counter/tests/counter_browser_tests.rs::live_region_updates_and_resumes`: the example gains a `/live` SSE route over a `LiveChannel` and a `<mount-stream protocol="frame">` clock region; the test sees two updates, drops the connection through the test server, and sees the next update after the reconnect | example |

## 8. Tasks

- [ ] 1. Open the GitHub issue for the unbounded backlog (severity medium).
- [ ] 2. `BacklogConfig`, `LiveError`, `LiveChannel::new`, `register`, `close` with tests.
- [ ] 3. `LiveTx::push_ops` with tests, including the concurrent test of I4.
- [ ] 4. `FrameBroadcastSink` with its test.
- [ ] 5. `FrameV1`, `App::frames`, `PROTOCOL_FRAME` with tests.
- [ ] 6. The seam in `foundation-wasm.js`; the `wasmRuntime` option of `installFrameRuntime`.
- [ ] 7. `setLatestOnly`, the mount binding, the resume, the local transport; the seam of section 6.7.
- [ ] 8. `frames-live.test.js`.
- [ ] 9. The `/live` route in the test server and in `examples/ui_counter`; the browser test.
- [ ] 10. Bump crate versions.
- [ ] 11. Update `progress.md` and `learnings.md`.

## 9. Verify

```bash
cargo fmt --all -- --check
cargo clippy -p foundation_wasm_ui --profile uat --no-deps -- -D warnings
cargo test   -p foundation_wasm_ui --profile uat
cargo check  -p foundation_wasm_ui --profile uat --target wasm32-unknown-unknown
cargo test   -p foundation_wasm --profile uat
cargo test   -p foundation_browser --profile uat
mise run ui-web:test
node --check backends/foundation_wasm/runtime/foundation-wasm.js
cargo test   -p ui_counter --profile uat
mise run test:browser
git diff --stat backends/foundation_wasm_ui/src/server/mod.rs   # two added lines only
```

## 10. Done when

- [ ] All tasks are complete and every command of section 9 passes with no warning.
- [ ] Each invariant and each scenario has a passing test (section 7).
- [ ] `Broadcaster`, `BroadcastTx`, `BroadcastSink` and `FrameTransport` are unchanged.
- [ ] No lock is held while user code runs, except the `send_binary` calls that the
      existing `Broadcaster` also makes under its lock.
- [ ] The GitHub issue of task 1 exists and is linked in `learnings.md`.
- [ ] The check list of the `story-driven-specification` skill passes.

---

_Created: 2026-11-13_
