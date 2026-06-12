# Feature 03: App Bootstrap — one-line wiring with protocol presets

## 1. Motivation

Every program (and every test) repeats the same five-object ritual:

```rust
let signals = Rc::new(SignalsRuntime::new());
let ctx = Context::new(Rc::clone(&signals));
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())
    .build();
runtime.attach(&signals);
let receiver = runtime.receiver();
```

Nothing in it varies except the protocol. Wrap it once; keep every piece
reachable from the wrapper.

## 2. Design: one `App` struct, protocol-named constructors

One struct with named constructors rather than distinct `ColumnarApp` /
`ArrowApp` / `JsonApp` types: the protocol is a VALUE choice behind the
`ProtocolMethods` seam, not a type-level property of the app — distinct
types would force every downstream signature to be generic (or duplicated)
over the app flavor for zero benefit. (If a use case ever needs the
protocol type statically, `App::with_protocol` keeps that door open.)

```rust
pub struct App {
    signals: Rc<SignalsRuntime>,
    ctx: Context,
    runtime: Runtime,
    receiver: SharedInstructionReceiver,
}

impl App {
    /// THE DEFAULT — the arrow-family wire (protocol byte 1), VERSION 1:
    /// compact columnar, zero-copy on the JS side. We always default to
    /// arrow unless otherwise stated.
    pub fn new() -> App;                      // = arrow family, v1 columnar
    /// Explicit alias of the default (wire VERSION 1).
    pub fn columnar() -> App;
    /// Arrow family VERSION 2: real Apache Arrow IPC RecordBatches, via a
    /// NEW `ArrowIpcV2` ProtocolMethods impl wrapping the EXISTING
    /// `foundation_arrow::ArrowIpcEncoder` (already implements
    /// `ProtocolEncoder<Vec<DomOp>>`). Behind an `arrow` cargo feature
    /// (optional foundation_arrow dep); JS side reads it with the embedded
    /// apache-arrow.js (`APACHE_ARROW_JS`).
    pub fn arrow_ipc() -> App;
    /// Human-readable JSON DomOps (debugging).
    pub fn json() -> App;
    /// MockProtocol — tests; pairs with `sent_batches()` access.
    pub fn mock() -> (App, SentBatches);
    /// SERVER preset (review addition): on native, host_apply is a no-op
    /// stub — these capture every flushed batch as envelope-framed Vec<u8>
    /// in a shared queue (FrameSink<E>), ready for WS binary frames / SSE
    /// toward a client mount-stream. server() = arrow v1; server_with takes
    /// ANY Layer-1 encoder (JsonEncoder, foundation_arrow::ArrowIpcEncoder —
    /// no cargo feature needed server-side).
    pub fn server() -> (App, CollectedFrames);
    pub fn server_with<E: ProtocolEncoder<Vec<DomOp>> + 'static>(e: E) -> (App, CollectedFrames);
    /// Escape hatch: any ProtocolMethods impl.
    pub fn with_protocol(p: impl ProtocolMethods<Vec<DomOp>> + 'static) -> App;

    /// The pair every reactive html!/component call needs.
    pub fn context(&self) -> (Context, SharedInstructionReceiver);

    pub fn ctx(&self) -> &Context;
    pub fn receiver(&self) -> SharedInstructionReceiver;
    pub fn signals(&self) -> &Rc<SignalsRuntime>;
    pub fn stabilize(&self);                 // signals().stabilize()
    pub fn scope(&self) -> Context;          // ctx().child() — component scopes
}
```

Usage:

```rust
let app = App::new();        // arrow family by default
let (ctx, receiver) = app.context();
let tree = html! { ctx, receiver, <main>…</main> };
app.stabilize();
```

Notes (settled):
- **Arrow is the default, restated.** The wire taxonomy (spec-39): protocol
  byte 1 = the ARROW FAMILY; envelope VERSION demuxes v1 (our compact
  zero-copy columnar — `ColumnarV1`, the F19 rename of the old `Arrow*`
  names) from v2 (real Apache Arrow IPC, `foundation_arrow`). `App::new()`
  IS arrow (family, v1); `arrow_ipc()` selects v2. Nothing defaults to
  json/mock — those are opt-in debugging/testing presets.
- `context()` returns CLONES (both are cheap Rc-backed handles) — the tuple
  the user asked for, directly usable by `html!` and component functions.
- "PlainHtmlApp" from the plan: there is no html-string protocol on the
  DomOp loop — the html-as-text story is `Html::to_markup()` (feature 00)
  on the server side, plus the existing `MorphNode` op for html patches. So
  the preset set is new/columnar/arrow_ipc/json/mock (+`with_protocol`); a
  markup "app" wrapper is not needed — servers call `to_markup` on plain
  `Html` values with no runtime at all.
- `App` owns all five pieces → drop order handled in one place; dropping the
  `App` disposes the root context (tears down effects) with the runtime
  still alive, then the rest in field order.
- Memory defaults to `MemoryAllocations::new()`; an `App::builder()` is NOT
  added until a real need appears (don't speculate API).

## 3. Server rendering (review addition)

First-paint HTML needs NO App (`html!{…}.to_markup()` — feature 00). Live
server-driven UI = `App::server()` + ship the frames; one App per
connection/session; drop on disconnect. README §10 is the user-facing
walkthrough. `Context` gains handle-counted `Clone` (clone = another handle
to the SAME scope; disposal at the last handle — `Rc::strong_count` can't
decide it because parents hold child inners) so `app.context()` can hand
out an owned pair.

## 4. Testing

Construction per preset; `context()` pair drives a reactive template
end-to-end (mock preset asserts the op stream); `stabilize` flushes;
dropping `App` disposes effects (signal set after drop runs nothing);
`scope()` child teardown independent of root.
