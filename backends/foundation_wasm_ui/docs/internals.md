# Internals — runtime, JS runtime, the wire

How the pieces fit once you go below `App` and `html!`.

## Mental model

```
Rust                                              JS (foundation-wasm-ui.js)
────                                              ──────────────────────────
ctx.signal / set()  ── stabilize() ──▶ effects
                                        │ queue DomOps
                                        ▼
                          Runtime + InstructionReceiver
                                        │ flush → encode (envelope)
                                        ▼ one host_apply (or a server frame)
                                                   ColumnarParser → DomOpApplicator
                                                   → NodeRegistry → real DOM
events  ◀── invoke_signal_callback ◀────────────── EventDispatcher (primal:on*)
```

Rust never touches the DOM. It emits typed `DomOp` instructions; the JS runtime
applies them. No virtual DOM, no diffing, no wasm-bindgen.

## `Runtime`, receiver, event bridge

`Runtime` owns the `InstructionReceiver` and wires it into the reactive loop;
`App` builds one for you. The lower layer:

```rust
use foundation_wasm_ui::{ColumnarV1, Runtime, SharedInstructionReceiver, install_event_bridge};
use foundation_wasm::MemoryAllocations;
use foundation_signals::Runtime as SignalsRuntime;
use std::rc::Rc;

let signals = Rc::new(SignalsRuntime::new());
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())   // OR .global_arena() for the live JS loop
    .build();
runtime.attach(&signals);               // flush after every stabilize
let receiver: SharedInstructionReceiver = runtime.receiver();
install_event_bridge(Rc::clone(&signals));   // the two-way-binding return leg

// Highest-throughput: columnar-native accumulation (no Vec<DomOp>):
let runtime = Runtime::builder().columnar().global_arena().build();
```

`Runtime::attach` registers a `NotificationManager` that calls `flush()` after
every `stabilize()`, so the full loop is: `set() → stabilize() → effects queue
DomOps → flush → encode → one host_apply → JS applies + ACKs`.

`SharedInstructionReceiver` (cloneable, `Rc<RefCell<…>>`) is the handle effects
capture: `queue(op)`, `flush()`, `ack(memory_id)`, `pending_count()`.

### Constraints

- Single-threaded by design (the `Rc`/`RefCell` handles aren't `Send`).
  Cross-thread signal access goes through `foundation_signals`' `SignalHub`.
- `.global_arena()` vs `.memory(..)`, and `.columnar()` vs `.protocol(..)`, are
  mutually exclusive — mixing panics at build.
- `install_event_bridge` is process-global (single-threaded contract).

## The JS runtime

`runtimes/foundation-wasm-ui.js` is a single dependency-free ES module — the other
half of every contract. Major exports:

- **`ColumnarParser`** — parses wire v1; zero-copy `TypedArray` views when aligned.
- **`DomOpApplicator` + `NodeRegistry`** — applies all 19 `DomOp`s. Reserved
  ambient node ids: **`0=head`, `1=body`, `2=html`, `3=theme style`**; Rust id
  allocation starts at 16. (`App::theme` appends to `0`; `App::mount` to `1`.)
- **`EventDispatcher`** — wires `primal:on*`: `primal:setter` → signal delivery via
  `invoke_signal_callback`, `callback-N` → WASM dispatch, dot-paths → JS scope fns.
- **`MorphDom`** — DOM morphing for server-rendered HTML patches.
- **`Transport`** factory — `fetch`, `sse`, `ws`, `chunked`, `worker`.
- **`Patcher` / `decodeEnvelopeFrame`** — envelope → routed result: protocol 1
  → DomOp applicator (columnar v1 / Arrow IPC v2), protocol 2 → JSON (DomOp batch
  *or* signal patches), text → `routeHtml` (materialize / island morph).
- **`registerWebComponents()`** / **`registerArrowIpc(arrow?)`** — one-call setup.

### Web components

Three custom elements ship server-driven islands with zero app JS:

```html
<primal-island>…server-rendered content…</primal-island>
<mount-data api="/api/profile" method="POST" data='{"id":7}' target="#profile"></mount-data>
<mount-stream api="/feed" transport="sse" target="#timeline"></mount-stream>
```

Attributes: `api` (required), `data` (JSON), `transport`
(`fetch|sse|ws|chunked|worker`), `protocol` (`html|json|arrow` override), `target`
(CSS selector / `parent` / omit for self-replacement), `method`. Protocol
negotiation precedence: attribute > content-type/event-name > channel default.

## The wire (encode side)

Each protocol pairs a Layer-1 `ProtocolEncoder<Vec<DomOp>>` with a Layer-2
`ProtocolHandler` transport; `ProtocolMethods` (Layer 3) composes "encode these ops
and ship them". `encode_with_envelope(&encoder, ops)` produces the framed bytes
directly (the pure server path). **Alignment contract:** encoders pad so the
columnar header lands 8-byte aligned; misaligned input still parses (copy
fallback), only corrupt input fails with typed decode errors.

## Testing the loop

`MockProtocol` makes the whole loop assertable without a browser:

```rust
let (app, sent) = App::mock();        // sent: Rc<RefCell<Vec<Vec<DomOp>>>>
let (ctx, receiver) = app.context();
let _ui = html! { ctx, receiver, <div><span>{0i64}</span></div> };
app.stabilize();
assert!(matches!(sent.borrow()[0][0], DomOp::CreateElement { .. }));
```

For real-browser assertions, see **[server & protocols](./server-and-protocols.md#live-browser-testing)**.

See also: **[shipping](./shipping.md)** · the JS suite:
`node --test backends/foundation_wasm_ui/integration/test/` · **[incidents](./incidents/)**.
