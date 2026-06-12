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
    /// Wire-v1 compact columnar (the default loop protocol).
    pub fn columnar() -> App;
    /// Human-readable JSON DomOps (debugging).
    pub fn json() -> App;
    /// MockProtocol — tests; pairs with `sent_batches()` access.
    pub fn mock() -> (App, SentBatches);
    /// Escape hatch: any ProtocolMethods impl (incl. future Arrow IPC v2
    /// once a wasm-loop Arrow protocol lands; today v2 lives in
    /// foundation_arrow as a server/content-type wire form).
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
let app = App::columnar();
let (ctx, receiver) = app.context();
let tree = html! { ctx, receiver, <main>…</main> };
app.stabilize();
```

Notes (settled):
- `context()` returns CLONES (both are cheap Rc-backed handles) — the tuple
  the user asked for, directly usable by `html!` and component functions.
- "PlainHtmlApp" from the plan: there is no html-string protocol on the
  DomOp loop — the html-as-text story is `Html::to_markup()` (feature 00)
  on the server side, plus the existing `MorphNode` op for html patches. So
  the preset set is columnar/json/mock (+`with_protocol`); a markup "app"
  wrapper is not needed — servers call `to_markup` on plain `Html` values
  with no runtime at all.
- `App` owns all five pieces → drop order handled in one place; dropping the
  `App` disposes the root context (tears down effects) with the runtime
  still alive, then the rest in field order.
- Memory defaults to `MemoryAllocations::new()`; an `App::builder()` is NOT
  added until a real need appears (don't speculate API).

## 3. Testing

Construction per preset; `context()` pair drives a reactive template
end-to-end (mock preset asserts the op stream); `stabilize` flushes;
dropping `App` disposes effects (signal set after drop runs nothing);
`scope()` child teardown independent of root.
